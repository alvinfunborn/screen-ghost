mod monitor_state;

pub use monitor_state::MonitorState;

use log::{error, debug};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
use std::sync::Mutex;

use crate::{ai::face_recognition, api::emitter, config, monitor::MonitorInfo, overlay};

static THREAD: Mutex<Option<std::thread::JoinHandle<()>>> = Mutex::new(None);

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
    let interval = config::get_config().unwrap().monitoring.unwrap().interval;
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
            std::thread::sleep(std::time::Duration::from_millis(interval));
        }));
    }
}

fn cal() {
    let monitor = MonitorState::get_working();
    debug!("[cal] get working monitor: {monitor:?}");
    if monitor.is_err() {
        error!("[cal] get working monitor failed: {monitor:?}");
        return;
    }
    let monitor = monitor.unwrap();
    
    let image = match monitor.screen_shot() {
        Ok(img) => img,
        Err(e) => {
            error!("[cal] screen shot failed: {}", e);
            return;  // 优雅退出而不是 panic
        }
    };

    match face_recognition::detect_targets_or_all_faces(&image) {
        Ok(rects) => {
            if rects.is_empty() {
                debug!("[cal] no faces detected");
            }
            emitter::emit_frame_info(rects.clone());

            // 叠加马赛克仍然基于检测框
            crate::overlay::overlay::apply_mosaic(rects, monitor.scale_factor);
        }
        Err(e) => {
            error!("[cal] face processing failed: {}", e);
        }
    }
}