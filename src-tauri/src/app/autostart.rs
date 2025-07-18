use log::info;
use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

const AUTO_START: bool = true;

pub fn set_auto_start(
    app_handle: &AppHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    let auto_start = AUTO_START;
    let autostart_manager = app_handle.autolaunch();
    info!("[set_auto_start] auto start: {}", auto_start);
    if auto_start {
        let _ = autostart_manager.enable();
    } else {
        let _ = autostart_manager.disable();
    }
    Ok(())
}