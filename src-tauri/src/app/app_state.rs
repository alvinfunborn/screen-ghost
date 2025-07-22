use std::sync::Mutex;

use once_cell::sync::Lazy;
use tauri::{AppHandle, WebviewWindow};


static APP: Lazy<Mutex<Option<AppState>>> = Lazy::new(|| Mutex::new(None));

#[derive(Clone)]
pub struct AppState {
    pub handle: AppHandle,
    pub main_window: WebviewWindow,
}

impl AppState {
    /// 设置全局App实例
    pub fn set_global(app: AppState) -> Result<(), Box<dyn std::error::Error>> {
        let mut app_guard = APP.lock().map_err(|e| format!("Failed to lock app mutex: {}", e))?;
        *app_guard = Some(app);
        Ok(())
    }

    /// 获取全局App实例
    pub fn get_global() -> Result<AppState, Box<dyn std::error::Error>> {
        let app_guard = APP.lock().map_err(|e| format!("Failed to lock app mutex: {}", e))?;
        app_guard.clone().ok_or_else(|| "App not initialized".into())
    }

    /// 获取全局AppHandle
    pub fn get_handle() -> Result<AppHandle, Box<dyn std::error::Error>> {
        Self::get_global().map(|app| app.handle)
    }

    /// 获取全局MainWindow
    pub fn get_main_window() -> Result<WebviewWindow, Box<dyn std::error::Error>> {
        Self::get_global().map(|app| app.main_window)
    }

    /// 检查App是否已初始化
    pub fn is_initialized() -> bool {
        APP.lock().map(|guard| guard.is_some()).unwrap_or(false)
    }
}


// 使用示例：
// 
// 在其他模块中获取AppHandle：
// ```rust
// use crate::app::App;
// 
// fn some_function() -> Result<(), Box<dyn std::error::Error>> {
//     let handle = App::get_handle()?;
//     handle.emit("some-event", "data")?;
//     Ok(())
// }
// ```
// 
// 获取MainWindow：
// ```rust
// fn control_window() -> Result<(), Box<dyn std::error::Error>> {
//     let window = App::get_main_window()?;
//     window.hide()?;
//     Ok(())
// }
// ```
// 
// 检查是否已初始化：
// ```rust
// if App::is_initialized() {
//     // 可以安全地获取App实例
//     let app = App::get_global()?;
// }
// ```
