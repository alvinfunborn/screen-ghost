use log::{debug, info};
use serde::{Deserialize, Serialize};

use super::monitor::{MonitorInfo};
use std::sync::{Arc, Mutex, OnceLock};
use std::collections::HashMap;
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
pub struct Image {
	pub width: i32,
	pub height: i32,
	pub data: Vec<u8>, // BGRA
}

// 对外统一的截图入口。后续可将 MonitorInfo 上的方法完全移走并在此实现具体逻辑。
pub fn capture_monitor_image(monitor: &MonitorInfo) -> Result<Image, String> {
	// 目前桥接到 MonitorInfo::screen_shot()
	let img = monitor.screen_shot()?;
	debug!("[capture_monitor_image] got buffer {}x{} ({} bytes)", img.width, img.height, img.data.len());
	Ok(img.into())
}
// 全局 DirectX 资源管理器
static DIRECTX_MANAGER: OnceLock<Arc<Mutex<DirectXResourceManager>>> = OnceLock::new();

struct DirectXResourceManager {
    device: Option<ID3D11Device>,
    context: Option<ID3D11DeviceContext>,
    staging_texture: Option<ID3D11Texture2D>,
    output_buffer: Vec<u8>,
    is_initialized: bool,
    last_width: i32,
    last_height: i32,
}

impl DirectXResourceManager {
    fn new() -> Self {
        Self {
            device: None,
            context: None,
            staging_texture: None,
            output_buffer: Vec::new(),
            is_initialized: false,
            last_width: 0,
            last_height: 0,
        }
    }
    
    fn get_instance() -> Arc<Mutex<DirectXResourceManager>> {
        DIRECTX_MANAGER.get_or_init(|| {
            Arc::new(Mutex::new(DirectXResourceManager::new()))
        }).clone()
    }
    
    fn initialize(&mut self) -> Result<(), String> {
        if self.is_initialized {
            return Ok(());
        }
        
        unsafe {
            // 创建 D3D11 设备和上下文
            let mut device: Option<ID3D11Device> = None;
            let mut context: Option<ID3D11DeviceContext> = None;
            
            let hr = D3D11CreateDevice(
                None,
                windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE,
                windows::Win32::Foundation::HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            );
            
            if hr.is_err() || device.is_none() || context.is_none() {
                return Err("Failed to create D3D11 device".to_string());
            }
            
            self.device = device;
            self.context = context;
            self.is_initialized = true;
            
            info!("[DirectXResourceManager] Initialized successfully");
        }
        
        Ok(())
    }
    
    fn ensure_staging_texture(&mut self, width: i32, height: i32) -> Result<(), String> {
        // 如果尺寸没变，直接返回
        if self.last_width == width && self.last_height == height && self.staging_texture.is_some() {
            return Ok(());
        }
        
        unsafe {
            if let (Some(device), Some(_context)) = (&self.device, &self.context) {
                // 创建新的 staging texture
                let mut desc = D3D11_TEXTURE2D_DESC::default();
                desc.Width = width as u32;
                desc.Height = height as u32;
                desc.MipLevels = 1;
                desc.ArraySize = 1;
                desc.Format = windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
                desc.SampleDesc.Count = 1;
                desc.SampleDesc.Quality = 0;
                desc.Usage = D3D11_USAGE_STAGING;
                desc.BindFlags = 0;
                desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
                desc.MiscFlags = 0;
                
                let mut staging_texture: Option<ID3D11Texture2D> = None;
                device.CreateTexture2D(&desc, None, Some(&mut staging_texture))
                    .map_err(|e| format!("Failed to create staging texture: {}", e))?;
                
                self.staging_texture = staging_texture;
                self.last_width = width;
                self.last_height = height;
                
                // 预分配输出缓冲区
                let buffer_size = (width * height * 4) as usize;
                if self.output_buffer.len() < buffer_size {
                    self.output_buffer.resize(buffer_size, 0);
                }
                
                info!("[DirectXResourceManager] Created staging texture {}x{}", width, height);
            }
        }
        
        Ok(())
    }
    
    fn get_device(&self) -> Option<&ID3D11Device> {
        self.device.as_ref()
    }
    
    fn get_context(&self) -> Option<&ID3D11DeviceContext> {
        self.context.as_ref()
    }
    
    fn get_staging_texture(&self) -> Option<&ID3D11Texture2D> {
        self.staging_texture.as_ref()
    }
    
    fn get_output_buffer(&mut self) -> &mut Vec<u8> {
        &mut self.output_buffer
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
enum CaptureMethod { Optimized, Standard, Alternative }

#[derive(Clone, Debug)]
struct CaptureStats {
    consec_optimized: u32,
    consec_standard: u32,
    consec_alternative: u32,
    preferred: CaptureMethod,
}

impl Default for CaptureStats {
    fn default() -> Self {
        Self {
            consec_optimized: 0,
            consec_standard: 0,
            consec_alternative: 0,
            preferred: CaptureMethod::Optimized,
        }
    }
}

static CAPTURE_STATE: OnceLock<Mutex<HashMap<usize, CaptureStats>>> = OnceLock::new();
const SUCCESS_THRESHOLD: u32 = 3;

fn state_map() -> &'static Mutex<HashMap<usize, CaptureStats>> {
    CAPTURE_STATE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn choose_start_method(monitor_id: usize) -> CaptureMethod {
    let map = state_map().lock().ok();
    if let Some(m) = map.and_then(|m| m.get(&monitor_id).cloned()) {
        // 按性能优先选择达到阈值的方法
        if m.consec_optimized >= SUCCESS_THRESHOLD { return CaptureMethod::Optimized; }
        if m.consec_standard >= SUCCESS_THRESHOLD { return CaptureMethod::Standard; }
        if m.consec_alternative >= SUCCESS_THRESHOLD { return CaptureMethod::Alternative; }
        // 否则使用上次首选，默认 Optimized
        return m.preferred;
    }
    CaptureMethod::Optimized
}

fn record_result(monitor_id: usize, method: CaptureMethod, success: bool) {
    let mut map = match state_map().lock() { Ok(g) => g, Err(_) => return };
    let entry = map.entry(monitor_id).or_insert_with(|| CaptureStats { preferred: CaptureMethod::Optimized, ..Default::default() });
    // 更新连续计数
    match method {
        CaptureMethod::Optimized => {
            entry.consec_optimized = if success { entry.consec_optimized.saturating_add(1) } else { 0 };
        }
        CaptureMethod::Standard => {
            entry.consec_standard = if success { entry.consec_standard.saturating_add(1) } else { 0 };
        }
        CaptureMethod::Alternative => {
            entry.consec_alternative = if success { entry.consec_alternative.saturating_add(1) } else { 0 };
        }
    }
    // 依据阈值提升首选项（按性能从高到低）
    entry.preferred = if entry.consec_optimized >= SUCCESS_THRESHOLD {
        CaptureMethod::Optimized
    } else if entry.consec_standard >= SUCCESS_THRESHOLD {
        CaptureMethod::Standard
    } else if entry.consec_alternative >= SUCCESS_THRESHOLD {
        CaptureMethod::Alternative
    } else {
        // 若无方法达到阈值，保持原有首选
        entry.preferred
    };

    debug!(
        "[capture_state] monitor={} meth={:?} ok={} consec: opt={} std={} alt={} prefer={:?}",
        monitor_id,
        method,
        success,
        entry.consec_optimized,
        entry.consec_standard,
        entry.consec_alternative,
        entry.preferred
    );
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
                    debug!("[screen_shot] DirectX method succeeded");
                    return Ok(image);
                } else {
                    debug!("[screen_shot] DirectX method returned blank content, using GDI fallback");
                }
            }
            Err(e) => {
                debug!("[screen_shot] DirectX method failed: {}, using GDI fallback", e);
            }
        }

        // 如果 DirectX 失败或返回空白内容，使用 GDI 方法
        self.screen_shot_gdi()
    }

    fn set_dpi_awareness(&self) {
        unsafe {
            // 设置DPI感知为每显示器感知
            if let Err(e) = SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE) {
                debug!("[set_dpi_awareness] Failed to set DPI awareness: {}", e);
            }
        }
    }

    fn has_valid_content(&self, image: &Image) -> bool {
        // 采样若干点判断是否为“近乎纯色”或“全零”帧
        let width = image.width.max(1) as usize;
        let height = image.height.max(1) as usize;
        let data = &image.data;
        if data.len() < width * height * 4 { return false; }

        let grid_x = 8usize;
        let grid_y = 8usize;
        let mut non_zero = 0usize;
        let mut first_color: Option<[u8;4]> = None;
        let mut different_colors = 0usize;

        for gy in 0..grid_y {
            let y = gy * (height - 1) / (grid_y - 1).max(1);
            for gx in 0..grid_x {
                let x = gx * (width - 1) / (grid_x - 1).max(1);
                let idx = (y * width + x) * 4;
                if idx + 3 >= data.len() { continue; }
                let b = data[idx];
                let g = data[idx+1];
                let r = data[idx+2];
                let a = data[idx+3];
                if b != 0 || g != 0 || r != 0 || a != 0 { non_zero += 1; }
                match first_color {
                    None => first_color = Some([b,g,r,a]),
                    Some(fc) => { if fc != [b,g,r,a] { different_colors += 1; } }
                }
            }
        }

        // 判定规则：存在非零且存在至少一个不同颜色，认为有效
        // 否则视为“全黑/全白/纯色”无效帧
        non_zero > 0 && different_colors > 0
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
                let released = ReleaseDC(Some(desktop), dc);
                if released == 0 {
                    debug!("[screen_shot_gdi] ReleaseDC failed when mem_dc invalid");
                }
                return Err("Failed to create compatible DC".to_string());
            }

            let bitmap = CreateCompatibleBitmap(dc, self.width, self.height);
            if bitmap.is_invalid() {
                let ok = DeleteDC(mem_dc).as_bool();
                if !ok { debug!("[screen_shot_gdi] DeleteDC failed after CreateCompatibleBitmap error"); }
                let released = ReleaseDC(Some(desktop), dc);
                if released == 0 { debug!("[screen_shot_gdi] ReleaseDC failed after CreateCompatibleBitmap error"); }
                return Err("Failed to create compatible bitmap".to_string());
            }

            // 选择位图到内存DC
            let old_bitmap = SelectObject(mem_dc, bitmap.into());
            if old_bitmap.is_invalid() {
                let ok1 = DeleteObject(bitmap.into()).as_bool();
                if !ok1 { debug!("[screen_shot_gdi] DeleteObject failed after SelectObject error"); }
                let ok2 = DeleteDC(mem_dc).as_bool();
                if !ok2 { debug!("[screen_shot_gdi] DeleteDC failed after SelectObject error"); }
                let released = ReleaseDC(Some(desktop), dc);
                if released == 0 { debug!("[screen_shot_gdi] ReleaseDC failed after SelectObject error"); }
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
                let _ = SelectObject(mem_dc, old_bitmap);
                let ok1 = DeleteObject(bitmap.into()).as_bool();
                if !ok1 { debug!("[screen_shot_gdi] DeleteObject failed after BitBlt error"); }
                let ok2 = DeleteDC(mem_dc).as_bool();
                if !ok2 { debug!("[screen_shot_gdi] DeleteDC failed after BitBlt error"); }
                let released = ReleaseDC(Some(desktop), dc);
                if released == 0 { debug!("[screen_shot_gdi] ReleaseDC failed after BitBlt error"); }
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
                let _ = SelectObject(mem_dc, old_bitmap);
                let ok1 = DeleteObject(bitmap.into()).as_bool();
                if !ok1 { debug!("[screen_shot_gdi] DeleteObject failed after GetDIBits error"); }
                let ok2 = DeleteDC(mem_dc).as_bool();
                if !ok2 { debug!("[screen_shot_gdi] DeleteDC failed after GetDIBits error"); }
                let released = ReleaseDC(Some(desktop), dc);
                if released == 0 { debug!("[screen_shot_gdi] ReleaseDC failed after GetDIBits error"); }
                return Err("GetDIBits failed".to_string());
            }

            // 清理资源
            let _ = SelectObject(mem_dc, old_bitmap);
            let ok1 = DeleteObject(bitmap.into()).as_bool();
            if !ok1 { debug!("[screen_shot_gdi] DeleteObject failed during cleanup"); }
            let ok2 = DeleteDC(mem_dc).as_bool();
            if !ok2 { debug!("[screen_shot_gdi] DeleteDC failed during cleanup"); }
            let released = ReleaseDC(Some(desktop), dc);
            if released == 0 { debug!("[screen_shot_gdi] ReleaseDC failed during cleanup"); }

            let elapsed = start_time.elapsed();
            debug!("[screen_shot_gdi] GDI screenshot completed in {:?}: {}x{}", elapsed, self.width, self.height);

            Ok(Image {
                width: self.width,
                height: self.height,
                data: buffer,
            })
        }
    }

    fn screen_shot_directx(&self) -> Result<Image, String> {
        // 状态机：优先选择达到阈值的高性能方法；失败则向下回退
        let start = choose_start_method(self.id);
        let mut order: Vec<CaptureMethod> = match start {
            CaptureMethod::Optimized => vec![CaptureMethod::Optimized, CaptureMethod::Standard, CaptureMethod::Alternative],
            CaptureMethod::Standard => vec![CaptureMethod::Standard, CaptureMethod::Alternative],
            CaptureMethod::Alternative => vec![CaptureMethod::Alternative],
        };
        debug!("[screen_shot_directx] State start method: {:?}", start);

        for method in order.drain(..) {
            let res = match method {
                CaptureMethod::Optimized => {
                    debug!("[screen_shot_directx] Trying optimized method");
                    self.screen_shot_directx_optimized()
                }
                CaptureMethod::Standard => {
                    debug!("[screen_shot_directx] Trying standard method");
                    self.screen_shot_directx_standard()
                }
                CaptureMethod::Alternative => {
                    debug!("[screen_shot_directx] Trying alternative method");
                    self.screen_shot_directx_alternative()
                }
            };

            match res {
                Ok(image) => {
                    let ok = self.has_valid_content(&image);
                    if ok {
                        record_result(self.id, method, true);
                        debug!("[screen_shot_directx] {:?} method succeeded", method);
                        return Ok(image);
                    } else {
                        record_result(self.id, method, false);
                        debug!("[screen_shot_directx] {:?} method returned blank content", method);
                        continue;
                    }
                }
                Err(e) => {
                    record_result(self.id, method, false);
                    debug!("[screen_shot_directx] {:?} method failed: {}", method, e);
                    continue;
                }
            }
        }

        Err("All DirectX methods failed or returned blank".to_string())
    }

    // 新增：优化的 DirectX 截图函数，使用资源管理器
    fn screen_shot_directx_optimized(&self) -> Result<Image, String> {
        unsafe {
            let start_time = std::time::Instant::now();
            
            // 获取资源管理器实例
            let manager = DirectXResourceManager::get_instance();
            
            // 先初始化并创建（或复用）资源，然后克隆所需句柄，避免借用冲突
            let (device, context, staging_texture) = {
                let mut mgr = manager.lock().map_err(|e| format!("Failed to lock resource manager: {}", e))?;
                // 确保资源管理器已初始化
                mgr.initialize()?;
                // 确保 staging texture 已创建
                mgr.ensure_staging_texture(self.width, self.height)?;
                // 克隆 COM 句柄供后续使用
                let device = mgr.get_device().cloned().ok_or("Device not available")?;
                let context = mgr.get_context().cloned().ok_or("Context not available")?;
                let staging_texture = mgr.get_staging_texture().cloned().ok_or("Staging texture not available")?;
                (device, context, staging_texture)
            };
            
            // 创建DXGI工厂
            let factory: IDXGIFactory1 = match CreateDXGIFactory1() {
                Ok(f) => f,
                Err(e) => return Err(format!("CreateDXGIFactory1 failed: {e}")),
            };
            
            // 枚举适配器和输出，找到目标显示器
            let mut _adapter: Option<IDXGIAdapter1> = None;
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
                        debug!("[screen_shot_directx_optimized] Found matching output: Adapter={}, Output={}", i, j);
                        _adapter = Some(a.clone());
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
            
            // 适配器句柄此处不再需要显式使用
            let output = match output { Some(o) => o, None => return Err("No output found".to_string()) };
            
            // 获取Output1和Duplication
            let output1: IDXGIOutput1 = output.cast().map_err(|e| format!("Output1 cast failed: {e}"))?;
            
            // 尝试多次获取duplication，有时第一次会失败
            let mut duplication: Option<IDXGIOutputDuplication> = None;
            let mut retry_count = 0;
            const MAX_RETRIES: i32 = 5;
            
            while duplication.is_none() && retry_count < MAX_RETRIES {
                // DuplicateOutput 需要 IUnknown；ID3D11Device 可直接作为 Param<IUnknown>
                match output1.DuplicateOutput(&device) {
                    Ok(dup) => {
                        duplication = Some(dup);
                        debug!("[screen_shot_directx_optimized] Output duplication created on attempt {}", retry_count + 1);
                    }
                    Err(e) => {
                        retry_count += 1;
                        if retry_count >= MAX_RETRIES {
                            return Err(format!("DuplicateOutput failed after {} attempts: {e}", MAX_RETRIES));
                        }
                        std::thread::sleep(std::time::Duration::from_millis(150));
                    }
                }
            }
            
            let duplication = duplication.unwrap();
            
            // 获取下一帧
            let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut resource = None;
            // 一些外接坞/多GPU链路下，第一帧常为空白；适当增加等待时间
            let hr = duplication.AcquireNextFrame(250, &mut frame_info, &mut resource);
            if hr.is_err() {
                return Err("AcquireNextFrame failed".to_string());
            }
            let resource = resource.unwrap();
            
            // 检查是否有累积帧
            if frame_info.AccumulatedFrames == 0 {
                debug!("[screen_shot_directx_optimized] No accumulated frames");
            }
            
            // 拷贝到复用的 staging texture
            let tex: ID3D11Texture2D = resource.cast().map_err(|e| format!("Resource cast failed: {e}"))?;
            context.CopyResource(&staging_texture, &tex);
            
            // 读取像素数据到复用的缓冲区
            let mut mapped = windows::Win32::Graphics::Direct3D11::D3D11_MAPPED_SUBRESOURCE::default();
            context.Map(&staging_texture, 0, windows::Win32::Graphics::Direct3D11::D3D11_MAP_READ, 0, Some(&mut mapped))
                .map_err(|e| format!("Map failed: {e}"))?;
            
            let pitch = mapped.RowPitch as usize;
            let width = self.width as usize;
            let height = self.height as usize;
            
            // 获取复用缓冲区并确保大小足够
            let image_data = {
                let mut mgr = manager.lock().map_err(|e| format!("Failed to lock resource manager: {}", e))?;
                let output_buffer = mgr.get_output_buffer();
                if output_buffer.len() < width * height * 4 {
                    output_buffer.resize(width * height * 4, 0);
                }
            // 逐行复制数据到复用缓冲区
            // 逐行内存复制（仅在调用处使用 unsafe）
            for y in 0..height {
                let src = (mapped.pData as *const u8).wrapping_add(y * pitch);
                // 目标切片范围已在上方 resize 保证
                let start = y * width * 4;
                let end = start + width * 4;
                let dst_slice = &mut output_buffer[start..end];
                std::ptr::copy_nonoverlapping(src, dst_slice.as_mut_ptr(), width * 4);
            }
                // 返回一个拷贝用于构造 Image，避免持有锁
                output_buffer[..width * height * 4].to_vec()
            };
            
            context.Unmap(&staging_texture, 0);
            duplication.ReleaseFrame().ok();
            
            let elapsed = start_time.elapsed();
            debug!("[screen_shot_directx_optimized] Optimized DirectX screenshot completed in {:?}: {}x{}", elapsed, width, height);
            
            Ok(Image {
                width: width as i32,
                height: height as i32,
                data: image_data,
            })
        }
    }

    fn screen_shot_directx_standard(&self) -> Result<Image, String> {
        unsafe {
            debug!("[screen_shot_directx_standard] Starting standard DirectX method...");
            
            // 检查DPI感知
            if self.scale_factor != 1.0 {
                debug!("[screen_shot_directx_standard] High DPI monitor detected (scale_factor={})", self.scale_factor);
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
                        debug!("[screen_shot_directx_standard] Found matching output: Adapter={}, Output={}", i, j);
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
                        debug!("[screen_shot_directx_standard] Output duplication created on attempt {}", retry_count + 1);
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
            let hr = duplication.AcquireNextFrame(300, &mut frame_info, &mut resource);
            if hr.is_err() {
                return Err("AcquireNextFrame failed".to_string());
            }
            let resource = resource.unwrap();
            
            // 检查是否有累积帧
            if frame_info.AccumulatedFrames == 0 {
                debug!("[screen_shot_directx_standard] No accumulated frames");
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
                debug!("[screen_shot_directx_standard] All sampled pixels are zero");
            }
            
            context.Unmap(&cpu_tex, 0);
            duplication.ReleaseFrame().ok();
            
            debug!("[screen_shot_directx_standard] DirectX screenshot completed: {}x{}", desc.Width, desc.Height);
            
            Ok(Image {
                width: desc.Width as i32,
                height: desc.Height as i32,
                data: buf,
            })
        }
    }

    fn screen_shot_directx_alternative(&self) -> Result<Image, String> {
        unsafe {
            debug!("[screen_shot_directx_alternative] Starting alternative method...");
            
            // 初始化COM
            let co_init_result = CoInitializeEx(None, COINIT_MULTITHREADED);
            if co_init_result.is_err() {
                debug!("[screen_shot_directx_alternative] CoInitializeEx failed");
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
                        debug!("[screen_shot_directx_alternative] Output duplication created on attempt {}", retry_count + 1);
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
                        debug!("[screen_shot_directx_alternative] Frame acquired with {} accumulated frames", frame_info.AccumulatedFrames);
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
            
            debug!("[screen_shot_directx_alternative] Alternative method completed: {}x{}", desc.Width, desc.Height);
            
            Ok(Image {
                width: desc.Width as i32,
                height: desc.Height as i32,
                data: buf,
            })
        }
    }
}
