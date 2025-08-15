use tauri::Emitter;
use std::collections::HashMap;
use crate::{app::AppState, monitor::Image, utils::rect::Rect};

pub fn emit_image(image: &Image) {
    let app = AppState::get_global().unwrap();
    let handle = app.handle;
    handle.emit("image", image).unwrap();
}

pub fn emit_toast(message: &str) {
    let app = AppState::get_global().unwrap();
    let handle = app.handle;
    let _ = handle.emit("toast", message.to_string());
}

pub fn emit_toast_close() {
    emit_toast("close");
}

pub fn emit_frame_info(frame_info: Vec<Rect>) {
    let app = AppState::get_global().unwrap();
    let handle = app.handle;
    handle.emit("frame_info", frame_info).unwrap();
}