use crate::{monitor::{monitor, MonitorInfo}, system::monitoring};
use crate::config;
use crate::ai;
use crate::api::emitter as app_emitter;
use crate::overlay::overlay::get_latest_mosaic_payload;

#[tauri::command]
pub fn get_monitors() -> Vec<MonitorInfo> {
    monitor::list_monitors().unwrap()
}

#[tauri::command]
pub async fn set_working_monitor(monitor: MonitorInfo) -> Result<(), String> {
    // 就绪保护：Python 环境与人脸模型均需就绪
    let py_ready = ai::python_env::is_python_ready();
    let face_ready = crate::ai::faces::is_face_model_ready();
    if !py_ready {
        app_emitter::emit_toast("正在完成初始化，请稍候…");
        return Err("python_not_ready".to_string());
    }
    if !face_ready {
        app_emitter::emit_toast("人脸模型未就绪，请重启应用后重试");
        return Err("face_model_not_ready".to_string());
    }
    monitoring::set_working_monitor(monitor).await;
    Ok(())
}

#[tauri::command]
pub fn is_ready() -> bool {
    crate::ai::python_env::is_python_ready() && crate::ai::faces::is_face_model_ready()
}

#[tauri::command]
pub fn stop_monitoring() {
    // 停止监控
    monitoring::stop_monitoring();
}

#[tauri::command]
pub fn get_mosaic_style() -> String {
    config::get_config().unwrap().monitoring.unwrap().mosaic_style
}

#[tauri::command]
pub fn get_latest_mosaic() -> Option<serde_json::Value> {
    get_latest_mosaic_payload()
}