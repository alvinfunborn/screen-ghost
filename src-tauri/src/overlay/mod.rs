use log::{error, info, warn};
use tauri::{AppHandle, Manager};
use windows::Win32::{Foundation::HWND, Graphics::Dwm::{DwmSetWindowAttribute, DWMWINDOWATTRIBUTE}, UI::WindowsAndMessaging::{GetWindowLongW, SetWindowLongW, GWL_EXSTYLE, WS_EX_LAYERED, WS_EX_TRANSPARENT}};

use crate::monitor::MonitorInfo;

pub mod overlay;

pub fn create_overlay_window(
    app_handle: &AppHandle,
    monitor: &MonitorInfo,
) {
    // 如果已存在，先关闭
    if let Some(existing_window) = app_handle.get_webview_window("overlay") {
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
        "[create_overlay_window] create overlay window {}: position({}, {}), size{}x{}",
        "overlay", position_x, position_y, width, height
    );
    let window = tauri::WebviewWindowBuilder::new(
        app_handle,
        "overlay",
        tauri::WebviewUrl::App(format!("overlay.html?window_label={}", "overlay").into()),
    )
    .title("overlay")
    .transparent(true)
    .decorations(false)
    // must disable shadow, otherwise the window will be offset
    .shadow(false)
    .resizable(true)
    .inner_size(width, height)
    .focused(false)
    .build();

    if let Err(e) = window {
        panic!(
            "[create_overlay_window] create overlay window failed: {}",
            e
        );
    }
    
    let window = window.unwrap();
    if let Err(e) = window.set_position(tauri::PhysicalPosition::new(position_x, position_y)) {
        error!("[create_overlay_window] set position failed: {}", e);
    }
    // 确保窗口位置正确
    if let Ok(hwnd) = window.hwnd() {
        let hwnd_raw = hwnd.0;
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
            set_window_transparent_style(&window, hwnd_raw as i64);
        }
    }
}

fn set_window_transparent_style(window: &tauri::WebviewWindow, hwnd_raw: i64) {
    // 设置无任务栏图标并确保在最顶层
    if let Err(e) = window.set_skip_taskbar(true) {
        error!("[set_overlay_style] set skip taskbar failed: {}", e);
    }
    if let Err(e) = window.set_always_on_top(true) {
        error!("[set_overlay_style] set always on top failed: {}", e);
    }

    // 设置扩展窗口样式
    unsafe {
        let style = GetWindowLongW(HWND(hwnd_raw as *mut _), GWL_EXSTYLE);
        // 确保WS_EX_TRANSPARENT样式被正确设置
        SetWindowLongW(
            HWND(hwnd_raw as *mut _),
            GWL_EXSTYLE,
            style | (WS_EX_TRANSPARENT.0 | WS_EX_LAYERED.0) as i32,
        );
    }
}
