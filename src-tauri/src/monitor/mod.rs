pub mod monitor;
pub use monitor::MonitorInfo;

use log::{error, info};
use tauri::WebviewWindow;

// 获取所有显示器信息，按照x坐标排序
pub fn list_monitors(window: &WebviewWindow) -> Result<Vec<MonitorInfo>, String> {
    let monitors = window.available_monitors();
    if let Err(e) = monitors {
        panic!("[list_monitors] get available monitors failed: {}", e);
    }
    let mut monitors = monitors
        .unwrap()
        .into_iter()
        .enumerate()
        .map(|(index, monitor)| {
            let position = monitor.position();
            let size = monitor.size();
            MonitorInfo {
                id: index,
                x: position.x,
                y: position.y,
                width: size.width as i32,
                height: size.height as i32,
                scale_factor: monitor.scale_factor(),
            }
        })
        .collect::<Vec<_>>();

    // 首先按y坐标排序，然后按x坐标排序
    monitors.sort_by(|a, b| {
        // 优先按y坐标排序
        match a.y.cmp(&b.y) {
            std::cmp::Ordering::Equal => a.x.cmp(&b.x), // y相同时按x排序
            other => other,
        }
    });

    for monitor in &monitors {
        info!(
            "[list_monitors] monitor: {}, position: ({}, {}), size: {}x{}, scale_factor: {}",
            monitor.id, monitor.x, monitor.y, monitor.width, monitor.height, monitor.scale_factor
        );
    }

    Ok(monitors)
}