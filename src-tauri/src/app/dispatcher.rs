use std::sync::Mutex;
use log::error;
use once_cell::sync::Lazy;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

use crate::{monitor::MonitorInfo, mosaic::Mosaic};


static CURRENT_MONITOR: Lazy<Mutex<Option<MonitorInfo>>> = Lazy::new(|| Mutex::new(None));
// 当前马赛克
static CURRENT_MOSAIC: Lazy<Mutex<Vec<Mosaic>>> = Lazy::new(|| Mutex::new(vec![]));
// 截屏间隔, 单位ms
const SCREEN_SHOT_INTERVAL: u64 = 30;

pub fn run() {
    std::thread::spawn(move || loop {
        unsafe {
            // 1. 每个线程要初始化COM
            let result = CoInitializeEx(None, COINIT_MULTITHREADED);
            if result.is_err() {
                error!("CoInitializeEx failed: {result:?}");
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(SCREEN_SHOT_INTERVAL));
        mosaic();
    });
}

fn mosaic() {
    let monitor = CURRENT_MONITOR.lock().unwrap();
    if monitor.is_none() {
        return;
    }
    let monitor = monitor.as_ref().unwrap();
    let image = monitor.screen_shot().unwrap();


}
