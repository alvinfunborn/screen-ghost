use std::sync::Mutex;

use once_cell::sync::Lazy;
use crate::monitor::MonitorInfo;

static MONITOR_STATE: Lazy<Mutex<Option<MonitorState>>> = Lazy::new(|| Mutex::new(None));

#[derive(Clone)]
pub struct MonitorState {
    pub working_monitor: MonitorInfo,
}

impl MonitorState {

    /// 设置全局实例
    pub fn set_working(monitor: MonitorInfo) -> Result<(), Box<dyn std::error::Error>> {
        let mut guard = MONITOR_STATE.lock().map_err(|e| format!("Failed to lock app mutex: {}", e))?;
        *guard = Some(MonitorState { working_monitor: monitor });
        Ok(())
    }

    /// 获取全局实例
    pub fn get_working() -> Result<MonitorInfo, Box<dyn std::error::Error>> {
        let guard = MONITOR_STATE.lock().map_err(|e| format!("Failed to lock app mutex: {}", e))?;
        guard.clone().ok_or_else(|| "current monitor not set".into()).map(|state| state.working_monitor)
    }

    /// 检查是否已初始化
    pub fn is_working_set() -> bool {
        MONITOR_STATE.lock().map(|guard| guard.is_some()).unwrap_or(false)
    }
}