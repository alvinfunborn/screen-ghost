use tauri::{Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

use crate::api::command;

pub fn create_app_builder() -> tauri::Builder<tauri::Wry> {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let _ = app
                .get_webview_window("main")
                .expect("no main window")
                .set_focus();
        }))
        .invoke_handler(tauri::generate_handler![
            command::get_monitors,
            command::set_working_monitor,
            command::stop_monitoring,
            command::get_mosaic_style,
            command::get_latest_mosaic,
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { .. } = event {
                // 仅当主窗口关闭时退出整个应用；其他窗口（如 overlay）允许正常关闭
                if window.label() == "main" {
                    let _ = std::panic::catch_unwind(|| {
                        crate::system::monitoring::stop_monitoring();
                    });
                    let _ = window.app_handle().exit(0);
                }
            }
        })
}