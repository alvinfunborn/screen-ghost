mod monitor_state;

pub use monitor_state::MonitorState;

use log::{error, info};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
use std::sync::{OnceLock, Mutex};

use crate::{api::emitter, monitor::MonitorInfo, overlay};

static THREAD: Mutex<Option<std::thread::JoinHandle<()>>> = Mutex::new(None);

// 截屏间隔, 单位ms
const SCREEN_SHOT_INTERVAL: u64 = 5000;

pub async fn set_working_monitor(monitor: MonitorInfo) {
    overlay::create_overlay_window(&monitor).await;
    MonitorState::set_working(Some(monitor)).unwrap();
    run();
}

pub fn stop_monitoring() {
    overlay::close_overlay_window();
    MonitorState::set_working(None).unwrap();
    if let Some(window) = crate::overlay::OverlayState::get_window() {
        window.close().unwrap();
    }
    // 停止线程
    if let Ok(mut guard) = THREAD.lock() {
        if let Some(thread) = guard.take() {
            thread.join().unwrap();
        }
    }
}

pub fn run() {
    if let Ok(mut guard) = THREAD.lock() {
        *guard = Some(std::thread::spawn(move || loop {
            unsafe {
                // 1. 每个线程要初始化COM
                let result = CoInitializeEx(None, COINIT_MULTITHREADED);
                if result.is_err() {
                    error!("CoInitializeEx failed: {result:?}");
                }
            }
            cal();
            std::thread::sleep(std::time::Duration::from_millis(SCREEN_SHOT_INTERVAL));
        }));
    }
}

fn cal() {
    let monitor = MonitorState::get_working();
    info!("[cal] get working monitor: {monitor:?}");
    if monitor.is_err() {
        error!("[cal] get working monitor failed: {monitor:?}");
        return;
    }
    let monitor = monitor.unwrap();
    
    // 截图前隐藏mosaic
    crate::overlay::overlay::hide_mosaic();
    let image = match monitor.screen_shot() {
        Ok(img) => img,
        Err(e) => {
            error!("[cal] screen shot failed: {}", e);
            return;  // 优雅退出而不是 panic
        }
    };
    // 截图后恢复mosaic
    crate::overlay::overlay::show_mosaic();
    emitter::emit_image(&image);
    // 计算mosaic
    let faces = match crate::ai::face_detect::face_detect_high_performance(&image) {
        Ok(faces) => faces,
        Err(e) => {
            error!("[cal] face detection failed: {}", e);
            return;
        }
    };
    info!("[cal] faces: {faces:?}");
    crate::overlay::overlay::apply_mosaic(faces, monitor.scale_factor);
}