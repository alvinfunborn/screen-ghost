use std::panic;
use log::error;
use tauri::{Emitter, Manager};

pub fn setup_panic_handler(app_handle: tauri::AppHandle) {
    panic::set_hook(Box::new(move |panic_info| {
        let location = panic_info
            .location()
            .unwrap_or_else(|| panic::Location::caller());
        let message = match panic_info.payload().downcast_ref::<&str>() {
            Some(s) => *s,
            None => match panic_info.payload().downcast_ref::<String>() {
                Some(s) => &s[..],
                None => "Box<Any>",
            },
        };

        let error_info = format!(
            "program panic:\nlocation: {}:{}\nerror: {}",
            location.file(),
            location.line(),
            message
        );

        error!("{}", error_info);

        // 发送错误到前端
        let window = app_handle.get_webview_window("main").unwrap();
        window.emit("rust-panic", error_info).unwrap_or_else(|e| {
            error!(
                "[setup_panic_handler] send panic info to frontend failed: {}",
                e
            );
        });
    }));
}