use log::{info, warn};
use serde::{Deserialize, Serialize};
use windows::Win32::Graphics::Direct3D11::{D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D, D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING};
use windows::Win32::Graphics::Gdi::{BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, RGBQUAD, SRCCOPY};
use windows::Win32::Graphics::Gdi::{GetDC, ReleaseDC};
use windows::Win32::UI::WindowsAndMessaging::GetDesktopWindow;
use windows::core::Interface;
use windows::Win32::Graphics::Dxgi::{IDXGIOutputDuplication, DXGI_OUTDUPL_FRAME_INFO};
use windows::Win32::Graphics::Dxgi::{IDXGIFactory1, CreateDXGIFactory1, IDXGIAdapter1, IDXGIOutput, IDXGIOutput1};

use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
use windows::Win32::UI::HiDpi::{SetProcessDpiAwareness, PROCESS_PER_MONITOR_DPI_AWARE};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorInfo {
    pub id: usize,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub scale_factor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    pub width: i32,
    pub height: i32,
    pub data: Vec<u8>, // BGRA
}

impl MonitorInfo {
    pub fn screen_shot(&self) -> Result<Image, String> {
        // 设置DPI感知
        self.set_dpi_awareness();
        
        // 首先尝试 DirectX 方法
        match self.screen_shot_directx() {
            Ok(image) => {
                // 检查是否获取到有效内容（不是全零）
                if self.has_valid_content(&image) {
                    info!("[screen_shot] DirectX method succeeded");
                    return Ok(image);
                } else {
                    warn!("[screen_shot] DirectX method returned blank content, using GDI fallback");
                }
            }
            Err(e) => {
                warn!("[screen_shot] DirectX method failed: {}, using GDI fallback", e);
            }
        }

        // 如果 DirectX 失败或返回空白内容，使用 GDI 方法
        self.screen_shot_gdi()
    }

    fn set_dpi_awareness(&self) {
        unsafe {
            // 设置DPI感知为每显示器感知
            if let Err(e) = SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE) {
                warn!("[set_dpi_awareness] Failed to set DPI awareness: {}", e);
            }
        }
    }

    fn has_valid_content(&self, image: &Image) -> bool {
        // 检查前100个像素是否都是零
        let sample_size = std::cmp::min(100, image.data.len() / 4);
        for i in 0..sample_size {
            let offset = i * 4;
            if offset + 3 < image.data.len() {
                if image.data[offset] != 0 || image.data[offset + 1] != 0 || 
                   image.data[offset + 2] != 0 || image.data[offset + 3] != 0 {
                    return true;
                }
            }
        }
        false
    }

    fn screen_shot_gdi(&self) -> Result<Image, String> {
        unsafe {
            let start_time = std::time::Instant::now();
            
            // 获取桌面窗口的DC
            let desktop = GetDesktopWindow();
            let dc = GetDC(Some(desktop));
            if dc.is_invalid() {
                return Err("Failed to get desktop DC".to_string());
            }

            // 创建兼容的DC和位图
            let mem_dc = CreateCompatibleDC(Some(dc));
            if mem_dc.is_invalid() {
                ReleaseDC(Some(desktop), dc);
                return Err("Failed to create compatible DC".to_string());
            }

            let bitmap = CreateCompatibleBitmap(dc, self.width, self.height);
            if bitmap.is_invalid() {
                DeleteDC(mem_dc);
                ReleaseDC(Some(desktop), dc);
                return Err("Failed to create compatible bitmap".to_string());
            }

            // 选择位图到内存DC
            let old_bitmap = SelectObject(mem_dc, bitmap.into());
            if old_bitmap.is_invalid() {
                DeleteObject(bitmap.into());
                DeleteDC(mem_dc);
                ReleaseDC(Some(desktop), dc);
                return Err("Failed to select bitmap".to_string());
            }

            // 复制屏幕内容到位图
            let result = BitBlt(
                mem_dc,
                0,
                0,
                self.width,
                self.height,
                Some(dc),
                self.x,
                self.y,
                SRCCOPY,
            );

            if result.is_err() {
                SelectObject(mem_dc, old_bitmap);
                DeleteObject(bitmap.into());
                DeleteDC(mem_dc);
                ReleaseDC(Some(desktop), dc);
                return Err("BitBlt failed".to_string());
            }

            // 获取位图信息
            let mut bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: self.width,
                    biHeight: -self.height, // 负值表示自上而下
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [RGBQUAD::default()],
            };

            // 分配缓冲区
            let buffer_size = (self.width * self.height * 4) as usize;
            let mut buffer = vec![0u8; buffer_size];

            // 获取位图数据
            let lines = GetDIBits(
                mem_dc,
                bitmap,
                0,
                self.height as u32,
                Some(buffer.as_mut_ptr() as *mut _),
                &mut bmi,
                DIB_RGB_COLORS,
            );

            if lines == 0 {
                SelectObject(mem_dc, old_bitmap);
                DeleteObject(bitmap.into());
                DeleteDC(mem_dc);
                ReleaseDC(Some(desktop), dc);
                return Err("GetDIBits failed".to_string());
            }

            // 清理资源
            SelectObject(mem_dc, old_bitmap);
            DeleteObject(bitmap.into());
            DeleteDC(mem_dc);
            ReleaseDC(Some(desktop), dc);

            let elapsed = start_time.elapsed();
            info!("[screen_shot_gdi] GDI screenshot completed in {:?}: {}x{}", elapsed, self.width, self.height);

            Ok(Image {
                width: self.width,
                height: self.height,
                data: buffer,
            })
        }
    }

    fn screen_shot_directx(&self) -> Result<Image, String> {
        // 尝试多种DirectX方法
        info!("[screen_shot_directx] Trying standard method");
        match self.screen_shot_directx_standard() {
            Ok(image) => {
                if self.has_valid_content(&image) {
                    info!("[screen_shot_directx] Standard method succeeded");
                    return Ok(image);
                } else {
                    warn!("[screen_shot_directx] Standard method returned blank content");
                }
            }
            Err(e) => {
                warn!("[screen_shot_directx] Standard method failed: {}", e);
            }
        }
        
        info!("[screen_shot_directx] Trying alternative method");
        match self.screen_shot_directx_alternative() {
            Ok(image) => {
                if self.has_valid_content(&image) {
                    info!("[screen_shot_directx] Alternative method succeeded");
                    return Ok(image);
                } else {
                    warn!("[screen_shot_directx] Alternative method returned blank content");
                }
            }
            Err(e) => {
                warn!("[screen_shot_directx] Alternative method failed: {}", e);
            }
        }
        
        Err("All DirectX methods failed".to_string())
    }

    fn screen_shot_directx_standard(&self) -> Result<Image, String> {
        unsafe {
            info!("[screen_shot_directx_standard] Starting standard DirectX method...");
            
            // 检查DPI感知
            if self.scale_factor != 1.0 {
                warn!("[screen_shot_directx_standard] High DPI monitor detected (scale_factor={})", self.scale_factor);
            }
            
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
                    
                    // 使用更宽松的匹配条件，允许10像素的误差
                    let width_match = (self.width - ow).abs() <= 10;
                    let height_match = (self.height - oh).abs() <= 10;
                    
                    if self.x == ox && self.y == oy && width_match && height_match {
                        info!("[screen_shot_directx_standard] Found matching output: Adapter={}, Output={}", i, j);
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
            
            if !found {
                return Err("No matching adapter/output found".to_string());
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
            
            // 尝试多次获取duplication，有时第一次会失败
            let mut duplication: Option<IDXGIOutputDuplication> = None;
            let mut retry_count = 0;
            const MAX_RETRIES: i32 = 3;
            
            while duplication.is_none() && retry_count < MAX_RETRIES {
                match output1.DuplicateOutput(&device) {
                    Ok(dup) => {
                        duplication = Some(dup);
                        info!("[screen_shot_directx_standard] Output duplication created on attempt {}", retry_count + 1);
                    }
                    Err(e) => {
                        retry_count += 1;
                        if retry_count >= MAX_RETRIES {
                            return Err(format!("DuplicateOutput failed after {} attempts: {e}", MAX_RETRIES));
                        }
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                }
            }
            
            let duplication = duplication.unwrap();
            
            // 6. 获取下一帧
            let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut resource = None;
            let hr = duplication.AcquireNextFrame(100, &mut frame_info, &mut resource);
            if hr.is_err() {
                return Err("AcquireNextFrame failed".to_string());
            }
            let resource = resource.unwrap();
            
            // 检查是否有累积帧
            if frame_info.AccumulatedFrames == 0 {
                warn!("[screen_shot_directx_standard] No accumulated frames");
            }
            
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
            
            // 检查是否有非零像素
            let mut has_non_zero = false;
            for i in 0..std::cmp::min(100, buf.len()) {
                if buf[i] != 0 {
                    has_non_zero = true;
                    break;
                }
            }
            
            if !has_non_zero {
                warn!("[screen_shot_directx_standard] All sampled pixels are zero");
            }
            
            context.Unmap(&cpu_tex, 0);
            duplication.ReleaseFrame().ok();
            
            info!("[screen_shot_directx_standard] DirectX screenshot completed: {}x{}", desc.Width, desc.Height);
            
            Ok(Image {
                width: desc.Width as i32,
                height: desc.Height as i32,
                data: buf,
            })
        }
    }

    fn screen_shot_directx_alternative(&self) -> Result<Image, String> {
        unsafe {
            info!("[screen_shot_directx_alternative] Starting alternative method...");
            
            // 初始化COM
            let co_init_result = CoInitializeEx(None, COINIT_MULTITHREADED);
            if co_init_result.is_err() {
                warn!("[screen_shot_directx_alternative] CoInitializeEx failed");
            }
            
            // 创建DXGI工厂
            let factory: IDXGIFactory1 = match CreateDXGIFactory1() {
                Ok(f) => f,
                Err(e) => return Err(format!("CreateDXGIFactory1 failed: {e}")),
            };
            
            // 找到目标显示器
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
                    
                    // 使用更宽松的匹配条件
                    let width_match = (self.width - ow).abs() <= 10;
                    let height_match = (self.height - oh).abs() <= 10;
                    
                    if self.x == ox && self.y == oy && width_match && height_match {
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
            
            if !found {
                return Err("No matching adapter/output found".to_string());
            }
            
            let adapter = adapter.unwrap();
            let adapter = adapter.cast::<windows::Win32::Graphics::Dxgi::IDXGIAdapter>().unwrap();
            let output = output.unwrap();
            
            // 创建D3D11设备，尝试不同的标志
            let mut device: Option<ID3D11Device> = None;
            let mut context: Option<ID3D11DeviceContext> = None;
            let hr = D3D11CreateDevice(
                Some(&adapter),
                windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_UNKNOWN,
                windows::Win32::Foundation::HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
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
            
            // 获取Output1和Duplication
            let output1: IDXGIOutput1 = output.cast().map_err(|e| format!("Output1 cast failed: {e}"))?;
            
            // 尝试多次获取duplication
            let mut duplication: Option<IDXGIOutputDuplication> = None;
            let mut retry_count = 0;
            const MAX_RETRIES: i32 = 5;
            
            while duplication.is_none() && retry_count < MAX_RETRIES {
                match output1.DuplicateOutput(&device) {
                    Ok(dup) => {
                        duplication = Some(dup);
                        info!("[screen_shot_directx_alternative] Output duplication created on attempt {}", retry_count + 1);
                    }
                    Err(e) => {
                        retry_count += 1;
                        if retry_count >= MAX_RETRIES {
                            return Err(format!("DuplicateOutput failed after {} attempts: {e}", MAX_RETRIES));
                        }
                        std::thread::sleep(std::time::Duration::from_millis(200));
                    }
                }
            }
            
            let duplication = duplication.unwrap();
            
            // 等待并获取帧，尝试多次
            let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut resource = None;
            let mut frame_attempts = 0;
            const MAX_FRAME_ATTEMPTS: i32 = 10;
            
            while frame_attempts < MAX_FRAME_ATTEMPTS {
                let hr = duplication.AcquireNextFrame(1000, &mut frame_info, &mut resource);
                if hr.is_ok() && resource.is_some() {
                    // 如果有累积帧，继续处理
                    if frame_info.AccumulatedFrames > 0 {
                        info!("[screen_shot_directx_alternative] Frame acquired with {} accumulated frames", frame_info.AccumulatedFrames);
                        break;
                    }
                }
                
                frame_attempts += 1;
                if frame_attempts >= MAX_FRAME_ATTEMPTS {
                    return Err("Failed to acquire frame with accumulated frames".to_string());
                }
                
                // 释放当前帧并重试
                if resource.is_some() {
                    duplication.ReleaseFrame().ok();
                    resource = None;
                }
                
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            
            let resource = resource.unwrap();
            
            // 拷贝到CPU可读的Texture2D
            let tex: ID3D11Texture2D = resource.cast().map_err(|e| format!("Resource cast failed: {e}"))?;
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            tex.GetDesc(&mut desc);
            
            let mut cpu_desc = desc.clone();
            cpu_desc.Usage = D3D11_USAGE_STAGING;
            cpu_desc.BindFlags = 0;
            cpu_desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
            cpu_desc.MiscFlags = 0;
            
            let mut cpu_tex: Option<ID3D11Texture2D> = None;
            device.CreateTexture2D(&cpu_desc, None, Some(&mut cpu_tex))
                .map_err(|e| format!("CreateTexture2D failed: {e}"))?;
            let cpu_tex = cpu_tex.unwrap();
            context.CopyResource(&cpu_tex, &tex);
            
            // 读取像素数据
            let mut mapped = windows::Win32::Graphics::Direct3D11::D3D11_MAPPED_SUBRESOURCE::default();
            context.Map(&cpu_tex, 0, windows::Win32::Graphics::Direct3D11::D3D11_MAP_READ, 0, Some(&mut mapped))
                .map_err(|e| format!("Map failed: {e}"))?;
            
            let pitch = mapped.RowPitch as usize;
            let mut buf = vec![0u8; (desc.Width * desc.Height * 4) as usize];
            
            // 逐行复制数据
            for y in 0..desc.Height as usize {
                let src = mapped.pData as *const u8;
                let dst = buf.as_mut_ptr().add(y * desc.Width as usize * 4);
                std::ptr::copy_nonoverlapping(src.add(y * pitch), dst, desc.Width as usize * 4);
            }
            
            context.Unmap(&cpu_tex, 0);
            duplication.ReleaseFrame().ok();
            
            info!("[screen_shot_directx_alternative] Alternative method completed: {}x{}", desc.Width, desc.Height);
            
            Ok(Image {
                width: desc.Width as i32,
                height: desc.Height as i32,
                data: buf,
            })
        }
    }
}
