use std::sync::Mutex;
use once_cell::sync::Lazy;
use tauri::WebviewWindow;

static OVERLAY_STATE: Lazy<Mutex<Option<OverlayState>>> = Lazy::new(|| Mutex::new(None));

#[derive(Debug)]
pub struct OverlayState {
    window: WebviewWindow,
}

impl OverlayState {

    pub fn get_window() -> Option<WebviewWindow> {
        OVERLAY_STATE.lock().unwrap().as_ref().map(|state| state.window.clone())
    }

    pub fn set_window(window: WebviewWindow) {
        *OVERLAY_STATE.lock().unwrap() = Some(OverlayState { window });
    }
}