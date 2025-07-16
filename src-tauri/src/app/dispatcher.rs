use std::sync::Mutex;
use once_cell::sync::Lazy;

use crate::mosaic::Mosaic;

static CURRENT_MONITOR: Lazy<Mutex<MonitorInfo>> = Lazy::new(|| Mutex::new(MonitorInfo::default()));
// 当前马赛克
static CURRENT_MOSAIC: Lazy<Mutex<Vec<Mosaic>>> = Lazy::new(|| Mutex::new(vec![]));
// 截屏间隔, 单位ms
const SCREEN_SHOT_INTERVAL: u64 = 30;

pub fn run() {
    std::thread::spawn(move || loop {
        // 1. 初始化COM
        let result = CoInitializeEx(None, COINIT_MULTITHREADED);
        if result.is_err() {
            return Err(format!("CoInitializeEx failed: {result:?}"));
        }
        std::thread::sleep(Duration::from_millis(SCREEN_SHOT_INTERVAL));
        mosaic();
    });
}

fn mosaic() {
    let image = CURRENT_MONITOR.lock().unwrap().screen_shot().unwrap();

}
