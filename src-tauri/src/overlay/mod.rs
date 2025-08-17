pub mod overlay;
pub mod overlay_state;

pub use overlay_state::OverlayState;

use log::{error, info, warn};
use tauri::Manager;
use windows::Win32::{
    Foundation::HWND,
    Graphics::Dwm::{DwmSetWindowAttribute, DWMWINDOWATTRIBUTE},
    UI::WindowsAndMessaging::{
        GetWindowLongW, SetWindowLongW, GWL_EXSTYLE, WS_EX_TRANSPARENT, WS_EX_LAYERED,
        SetWindowPos, HWND_TOPMOST, HWND_NOTOPMOST, SWP_NOMOVE, SWP_NOSIZE, SWP_NOACTIVATE, SWP_SHOWWINDOW,
        SetWindowDisplayAffinity, WINDOW_DISPLAY_AFFINITY, WDA_EXCLUDEFROMCAPTURE,
    },
};

use crate::{app::AppState, monitor::MonitorInfo};
// 不再在创建时下发样式，前端会在初始化时 invoke 获取

pub async fn create_overlay_window(
    monitor: &MonitorInfo,
) {
    info!("[create_overlay_window] Starting overlay window creation...");
    info!("[create_overlay_window] Monitor info: x={}, y={}, width={}, height={}, scale_factor={}", 
          monitor.x, monitor.y, monitor.width, monitor.height, monitor.scale_factor);
    
    // 如果已存在，先关闭
    if let Some(existing_window) = AppState::get_global().unwrap().handle.get_webview_window("overlay") {
        warn!("[create_overlay_window] close existing window: {}", "overlay");
        if let Err(e) = existing_window.close() {
            error!(
                "[create_overlay_window] close existing window failed: {}",
                e
            );
        }
    }

    let width = monitor.width as f64 / monitor.scale_factor;
    let height = monitor.height as f64 / monitor.scale_factor;
    let position_x = monitor.x;
    let position_y = monitor.y;
    info!(
        "[create_overlay_window] Calculated dimensions: position({}, {}), size({}x{})",
        position_x, position_y, width, height
    );
    
    info!("[create_overlay_window] Building window...");
    
    // 添加更多日志来诊断build过程
    info!("[create_overlay_window] About to create WebviewWindowBuilder...");
    
    info!("[create_overlay_window] WebviewWindowBuilder created, calling build()...");
    
    let app_state = AppState::get_global().unwrap();
    let handle = app_state.handle.clone();
    
    let window = tauri::WebviewWindowBuilder::new(
        &handle,
        "overlay",
        tauri::WebviewUrl::App("overlay.html".into()),
    )
    .title("overlay")
    .transparent(true)
    .decorations(false)
    .shadow(false)
    .resizable(false)
    .inner_size(width, height)
    .focused(false)
    .skip_taskbar(true)
    .always_on_top(true)
    .build();

    if let Err(e) = &window {
        error!("[create_overlay_window] create overlay window failed: {}", e);
        panic!(
            "[create_overlay_window] create overlay window failed: {}",
            e
        );
    }
    
    let window = window.unwrap();
    info!("[create_overlay_window] Window created successfully");

    if log::max_level() == log::LevelFilter::Debug {
        let _ = window.open_devtools();
    }
    
    OverlayState::set_window(window.clone());
    info!("[create_overlay_window] Window stored in OverlayState");

    // 样式获取改由前端初始化时通过 invoke('get_mosaic_style') 完成
    
    info!("[create_overlay_window] Setting window position to ({}, {})", position_x, position_y);
    if let Err(e) = window.set_position(tauri::PhysicalPosition::new(position_x, position_y)) {
        error!("[create_overlay_window] set position failed: {}", e);
    } else {
        info!("[create_overlay_window] Window position set successfully");
    }
    // 确保窗口位置正确
    info!("[create_overlay_window] Getting window handle...");
    match window.hwnd() {
        Ok(hwnd) => {
            let hwnd_raw = hwnd.0;
            info!("[create_overlay_window] Window handle obtained: {:?}", hwnd_raw);
            
            const DWMWA_WINDOW_CORNER_PREFERENCE: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(33);
            const DWMWCP_DONOTROUND: u32 = 1;
            let preference: u32 = DWMWCP_DONOTROUND;
            unsafe {
                // 去掉 Windows 11 圆角
                let _ = DwmSetWindowAttribute(
                    HWND(hwnd_raw as *mut _),
                    DWMWA_WINDOW_CORNER_PREFERENCE,
                    &preference as *const _ as _,
                    std::mem::size_of_val(&preference) as u32,
                );
                info!("[create_overlay_window] Setting transparent style and topmost...");
                set_window_transparent_style(&window, hwnd_raw as i64);
                // 通过“先取消再设置顶置 + 显示”确保位于任务栏之上
                let _ = SetWindowPos(
                    HWND(hwnd_raw as *mut _),
                    Some(HWND(HWND_NOTOPMOST.0)),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
                let _ = SetWindowPos(
                    HWND(hwnd_raw as *mut _),
                    Some(HWND(HWND_TOPMOST.0)),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
            }
        }
        Err(e) => {
            error!("[create_overlay_window] Failed to get window handle: {:?}", e);
        }
    }
    
    info!("[create_overlay_window] Overlay window creation completed");
}

fn set_window_transparent_style(window: &tauri::WebviewWindow, hwnd_raw: i64) {
    info!("[set_overlay_style] Setting window transparent style...");
    
    // 设置无任务栏图标并确保在最顶层
    if let Err(e) = window.set_skip_taskbar(true) {
        error!("[set_overlay_style] set skip taskbar failed: {}", e);
    } else {
        info!("[set_overlay_style] Skip taskbar set successfully");
    }
    
    // 置顶（配合后续的 SetWindowPos 再做一次保证）
    if let Err(e) = window.set_always_on_top(true) {
        error!("[set_overlay_style] set always on top failed: {}", e);
    } else {
        info!("[set_overlay_style] Always on top set successfully");
    }

    // 设置扩展窗口样式：对窗口设置穿透与分层
    unsafe {
        let hwnd = HWND(hwnd_raw as *mut _);
        apply_click_through_to_hwnd(hwnd);
        info!("[set_overlay_style] Applied click-through to overlay HWND");
    }
    
    info!("[set_overlay_style] Transparent style setup completed");
}

#[inline]
unsafe fn apply_click_through_to_hwnd(hwnd: HWND) {
    let style = GetWindowLongW(hwnd, GWL_EXSTYLE);
    // 参考 screen-buoy：使用 WS_EX_TRANSPARENT 与 WS_EX_LAYERED
    let new_style = style | (WS_EX_TRANSPARENT.0 | WS_EX_LAYERED.0) as i32;
    if style != new_style {
        let prev = SetWindowLongW(hwnd, GWL_EXSTYLE, new_style);
        if prev == 0 {
            error!(
                "[set_overlay_style] SetWindowLongW failed for child/parent hwnd={:?}",
                hwnd
            );
        } else {
            info!(
                "[set_overlay_style] HWND {:?} exstyle updated: 0x{:x} -> 0x{:x}",
                hwnd, style, new_style
            );
        }
    } else {
        info!("[set_overlay_style] HWND {:?} already click-through", hwnd);
    }

    // 将窗口从屏幕捕获中排除，避免截图时捕获到 overlay，从而无需隐藏/显示马赛克
    match SetWindowDisplayAffinity(hwnd, WINDOW_DISPLAY_AFFINITY(WDA_EXCLUDEFROMCAPTURE.0)) {
        Ok(()) => info!("[set_overlay_style] SetWindowDisplayAffinity: WDA_EXCLUDEFROMCAPTURE applied"),
        Err(e) => warn!("[set_overlay_style] SetWindowDisplayAffinity failed or unsupported: {}", e),
    }
}

pub fn close_overlay_window() {
    if let Some(window) = OverlayState::get_window() {
        window.close().unwrap();
    }
}
