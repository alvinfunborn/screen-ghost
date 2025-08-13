use crate::{monitor::{self, MonitorInfo}, system::monitoring};

#[tauri::command]
pub fn get_monitors() -> Vec<MonitorInfo> {
    monitor::list_monitors().unwrap()
}

#[tauri::command]
pub async fn set_working_monitor(monitor: MonitorInfo) {
    monitoring::set_working_monitor(monitor).await;
}

#[tauri::command]
pub fn stop_monitoring() {
    // 停止监控
    monitoring::stop_monitoring();
}