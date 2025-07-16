use log::{error, info};
use serde::{Deserialize, Serialize};
use windows::Win32::Graphics::Direct3D11::{D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D, D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING};
use windows::Win32::Graphics::Gdi::{BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HDC, RGBQUAD, SRCCOPY};
use windows::Win32::Graphics::Gdi::{GetDC, ReleaseDC};
use windows::Win32::UI::WindowsAndMessaging::GetDesktopWindow;
use windows::core::{Interface, Result as WinResult};
use windows::Win32::Graphics::Dxgi::{IDXGIOutputDuplication, DXGI_OUTDUPL_FRAME_INFO};
use windows::Win32::Graphics::Dxgi::{IDXGIFactory1, CreateDXGIFactory1, IDXGIAdapter1, IDXGIOutput, IDXGIOutput1};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::{DXGI_OUTPUT_DESC};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorInfo {
    pub id: usize,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub scale_factor: f64,
}

pub struct Image {
    pub width: i32,
    pub height: i32,
    pub data: Vec<u8>, // BGRA
}

impl MonitorInfo {
    pub fn screen_shot(&self) -> Result<Image, String> {
        unsafe {
            // 2. 创建DXGI工厂
            let factory: IDXGIFactory1 = match CreateDXGIFactory1() {
                Ok(f) => f,
                Err(e) => return Err(format!("CreateDXGIFactory1 failed: {e}")),
            };
            // 3. 枚举适配器和输出，找到目标显示器
            let mut adapter: Option<IDXGIAdapter1> = None;
            let mut output: Option<IDXGIOutput> = None;
            let mut i = 0;
            let mut found = false;
            while let Ok(a) = factory.EnumAdapters1(i) {
                let mut j = 0;
                while let Ok(o) = a.EnumOutputs(j) {
                    let desc = o.GetDesc().unwrap();
                    let ox = desc.DesktopCoordinates.left;
                    let oy = desc.DesktopCoordinates.top;
                    let ow = desc.DesktopCoordinates.right - desc.DesktopCoordinates.left + 1;
                    let oh = desc.DesktopCoordinates.bottom - desc.DesktopCoordinates.top;
                    if self.x == ox && self.y == oy && self.width == ow && self.height == oh {
                        adapter = Some(a.clone());
                        output = Some(o);
                        found = true;
                        break;
                    }
                    j += 1;
                }
                if found { break; }
                i += 1;
            }
            let adapter = match adapter { Some(a) => a, None => return Err("No adapter found".to_string()) };
            let adapter = adapter.cast::<windows::Win32::Graphics::Dxgi::IDXGIAdapter>().unwrap();
            let output = match output { Some(o) => o, None => return Err("No output found".to_string()) };
            // 4. 创建D3D11设备
            let mut device: Option<ID3D11Device> = None;
            let mut context: Option<ID3D11DeviceContext> = None;
            let hr = D3D11CreateDevice(
                Some(&adapter),
                windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_UNKNOWN,
                windows::Win32::Foundation::HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None, // 或 Some(&[])
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            );
            if hr.is_err() || device.is_none() || context.is_none() {
                return Err("D3D11CreateDevice failed".to_string());
            }
            let device = device.unwrap();
            let context = context.unwrap();
            // 5. 获取Output1和Duplication
            let output1: IDXGIOutput1 = output.cast().map_err(|e| format!("Output1 cast failed: {e}"))?;
            let duplication: IDXGIOutputDuplication = output1.DuplicateOutput(&device).map_err(|e| format!("DuplicateOutput failed: {e}"))?;
            // 6. 获取下一帧
            let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut resource = None;
            let hr = duplication.AcquireNextFrame(100, &mut frame_info, &mut resource);
            if hr.is_err() {
                return Err("AcquireNextFrame failed".to_string());
            }
            let resource = resource.unwrap();
            // 7. 拷贝到CPU可读的Texture2D
            let tex: ID3D11Texture2D = resource.cast().map_err(|e| format!("Resource cast failed: {e}"))?;
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            tex.GetDesc(&mut desc);
            let mut cpu_desc = desc.clone();
            cpu_desc.Usage = D3D11_USAGE_STAGING;
            cpu_desc.BindFlags = 0;
            cpu_desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
            cpu_desc.MiscFlags = 0;
            let mut cpu_tex: Option<ID3D11Texture2D> = None;
            device.CreateTexture2D(&cpu_desc, None, Some(&mut cpu_tex)).map_err(|e| format!("CreateTexture2D failed: {e}"))?;
            let cpu_tex = cpu_tex.unwrap();
            context.CopyResource(&cpu_tex, &tex);
            // 8. 读取像素数据
            let mut mapped = windows::Win32::Graphics::Direct3D11::D3D11_MAPPED_SUBRESOURCE::default();
            context.Map(&cpu_tex, 0, windows::Win32::Graphics::Direct3D11::D3D11_MAP_READ, 0, Some(&mut mapped)).map_err(|e| format!("Map failed: {e}"))?;
            let pitch = mapped.RowPitch as usize;
            let mut buf = vec![0u8; (desc.Width * desc.Height * 4) as usize];
            for y in 0..desc.Height as usize {
                let src = mapped.pData as *const u8;
                let dst = buf.as_mut_ptr().add(y * desc.Width as usize * 4);
                std::ptr::copy_nonoverlapping(src.add(y * pitch), dst, desc.Width as usize * 4);
            }
            context.Unmap(&cpu_tex, 0);
            duplication.ReleaseFrame().ok();
            Ok(Image {
                width: desc.Width as i32,
                height: desc.Height as i32,
                data: buf,
            })
        }
    }
}
