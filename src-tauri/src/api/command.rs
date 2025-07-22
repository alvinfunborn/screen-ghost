use crate::{monitor::{self, MonitorInfo}, system::monitoring};

#[tauri::command]
pub fn get_monitors() -> Vec<MonitorInfo> {
    monitor::list_monitors().unwrap()
}

#[tauri::command]
pub fn set_working_monitor(monitor: MonitorInfo) {
    monitoring::set_working_monitor(monitor);
}

