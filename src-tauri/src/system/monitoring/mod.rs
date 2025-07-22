mod monitor_state;

pub use monitor_state::MonitorState;

use log::{error, info};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

use crate::{api::emitter, monitor::MonitorInfo};

// 截屏间隔, 单位ms
const SCREEN_SHOT_INTERVAL: u64 = 5000;

pub fn set_working_monitor(monitor: MonitorInfo) {
    MonitorState::set_working(monitor).unwrap();
    run();
}

pub fn run() {
    std::thread::spawn(move || loop {
        unsafe {
            // 1. 每个线程要初始化COM
            let result = CoInitializeEx(None, COINIT_MULTITHREADED);
            if result.is_err() {
                error!("CoInitializeEx failed: {result:?}");
            }
        }
        cal();
        std::thread::sleep(std::time::Duration::from_millis(SCREEN_SHOT_INTERVAL));
    });
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
    
}