mod monitor_state;

pub use monitor_state::MonitorState;

use log::{error, debug};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
use std::sync::Mutex;

use crate::{ai::{faces}, api::emitter, config, monitor::{MonitorInfo, screen_shot}, overlay};

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
        *guard = Some(std::thread::spawn(move || {
            unsafe {
                // 1. 每个线程要初始化COM
                let result = CoInitializeEx(None, COINIT_MULTITHREADED);
                if result.is_err() {
                    error!("CoInitializeEx failed: {result:?}");
                }
            }
            loop {
                if !MonitorState::is_working_set() {
                    break;
                }
                cal();
                std::thread::sleep(std::time::Duration::from_millis(interval));
            }
        }));
    }
}

fn cal() {
    let monitor = MonitorState::get_working();
    debug!("[cal] get working monitor: {monitor:?}");
    if monitor.is_err() {
        // 当未设置工作显示器时，静默退出，等待监控线程自然停止
        return;
    }
    let monitor = monitor.unwrap();
    
    let image = match screen_shot::capture_monitor_image(&monitor) {
        Ok(img) => img,
        Err(e) => {
            error!("[cal] screen shot failed: {}", e);
            return;  // 优雅退出而不是 panic
        }
    };

    // 诊断：若数据大小刚好等于 width*height*4 但画面仍是空白，输出一次警告
    if image.data.len() == (image.width as usize * image.height as usize * 4) {
        // 简要采样首尾像素，辅助判断是否纯色
        if !image.data.is_empty() {
            let head = &image.data[0..4.min(image.data.len())];
            let tail = &image.data[image.data.len()-4..image.data.len()];
            debug!("[cal] screenshot buffer size matches {}x{}x4, head={:?}, tail={:?}", image.width, image.height, head, tail);
        }
    }

    debug!("[cal] screen shot success, image size: {}x{},{}", image.width, image.height, image.data.len());
    if log::max_level() == log::LevelFilter::Debug {
        emitter::emit_image(&image);
    }
    match faces::detect_targets_or_all_faces(&image) {
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