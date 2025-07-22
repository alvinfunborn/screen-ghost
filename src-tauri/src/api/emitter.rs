use tauri::Emitter;
use crate::{app::AppState, monitor::{Image, MonitorInfo}};

pub fn emit_image(image: &Image) {
    let app = AppState::get_global().unwrap();
    let handle = app.handle;
    handle.emit("image", image).unwrap();
}