mod monitor_state;

pub use monitor_state::MonitorState;

use log::{error, debug, info};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex as StdMutex;

use crate::{ai::{faces}, api::emitter, config, monitor::{MonitorInfo, screen_shot}, overlay};
use crate::utils::rect::Rect;

static THREAD: Mutex<Option<std::thread::JoinHandle<()>>> = Mutex::new(None);

// 预取下一帧：单帧缓冲 + 去重控制
static NEXT_FRAME: OnceLock<Mutex<Option<screen_shot::Image>>> = OnceLock::new();
static PREFETCHING: AtomicBool = AtomicBool::new(false);
static CAPTURE_LOCK: OnceLock<StdMutex<()>> = OnceLock::new();

fn next_frame_buf() -> &'static Mutex<Option<screen_shot::Image>> {
    NEXT_FRAME.get_or_init(|| Mutex::new(None))
}

fn spawn_prefetch() {
    // 避免并发重复预取
    if PREFETCHING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

    std::thread::spawn(|| {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }

        let monitor = MonitorState::get_working();
        if let Ok(monitor) = monitor {
            // 截图时仅持有 CAPTURE_LOCK；写入帧缓存时再短暂获取 NEXT_FRAME 锁，
            // 锁顺序固定：先 CAPTURE_LOCK 后 NEXT_FRAME，避免与主循环相反顺序造成死锁。
            let _g = CAPTURE_LOCK.get_or_init(|| StdMutex::new(())).lock();
            if let Ok(img) = screen_shot::capture_monitor_image(&monitor) {
                drop(_g);
                if let Ok(mut guard) = next_frame_buf().lock() {
                    if log::max_level() == log::LevelFilter::Debug {
                        emitter::emit_image(&img);
                    }
                    *guard = Some(img);
                }
            }
        }

        PREFETCHING.store(false, Ordering::SeqCst);
    });
}

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
    let cfg_interval = config::get_config().unwrap().monitoring.unwrap().interval;
    // 防止 0ms 忙等占用CPU与事件通道：钳制到至少 ~120fps
    let interval = if cfg_interval < 8 { 8 } else { cfg_interval.min(1000) };
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

    // 截图耗时统计开始
    let screenshot_start = std::time::Instant::now();

    // 优先使用上一轮预取的帧；若无，则在不持有 NEXT_FRAME 锁的情况下进行截图，
    // 以避免与预取线程形成相反的锁顺序（CAPTURE_LOCK -> NEXT_FRAME）而死锁。
    let mut from_prefetch: Option<screen_shot::Image> = None;
    if let Ok(mut guard) = next_frame_buf().lock() {
        from_prefetch = guard.take();
    }
    let image_result: Result<screen_shot::Image, String> = if let Some(img) = from_prefetch {
        debug!("[cal] use prefetched frame");
        Ok(img)
    } else {
        let _g = CAPTURE_LOCK.get_or_init(|| StdMutex::new(())).lock();
        screen_shot::capture_monitor_image(&monitor)
    };

    // 输出截图用时（info级别）
    let screenshot_elapsed_ms = screenshot_start.elapsed().as_millis();
    info!("[perf] prefetched screenshot {} ms", screenshot_elapsed_ms);

    match image_result {
        Ok(image) => {
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

            // 在进行检测的同时，异步预取下一帧
            spawn_prefetch();

            // 若人脸模型未就绪，则跳过本轮检测，但保证输出两行日志
            if !crate::ai::faces::is_face_model_ready() {
                debug!("[cal] face model not ready, skip detection");
                info!("[perf] face_detection 0 ms");
                return;
            }

            // 读取监控配置中的 capture_scale，对截图进行可选下采样
            let capture_scale = config::get_config()
                .and_then(|c| c.monitoring)
                .and_then(|m| m.capture_scale)
                .unwrap_or(1.0);

            let mut resize_ratio = 1.0f32;
            let detection_image = if capture_scale > 0.0 && capture_scale < 0.9999 {
                resize_ratio = capture_scale.max(0.1);
                downscale_image_bgra(&image, resize_ratio)
            } else {
                image.clone()
            };

            // 人脸检测耗时统计开始
            let face_start = std::time::Instant::now();
            match faces::detect_targets_or_all_faces(&detection_image) {
                Ok(rects) => {
                    // 输出人脸检测用时（info级别）
                    let face_elapsed_ms = face_start.elapsed().as_millis();
                    info!("[perf] face_detection {} ms", face_elapsed_ms);

                    if rects.is_empty() {
                        debug!("[cal] no faces detected");
                    }

                    // 将检测框从缩小坐标系映射回原始分辨率
                    let mapped_rects: Vec<Rect> = if (resize_ratio - 1.0).abs() < f32::EPSILON {
                        rects
                    } else {
                        let inv = 1.0f32 / resize_ratio;
                        rects
                            .into_iter()
                            .map(|r| Rect::new(
                                ((r.x as f32) * inv).round() as i32,
                                ((r.y as f32) * inv).round() as i32,
                                ((r.width as f32) * inv).round() as i32,
                                ((r.height as f32) * inv).round() as i32,
                            ))
                            .collect()
                    };

                    // 对前端 app 布局发送映射回原分辨率的检测框
                    emitter::emit_frame_info(mapped_rects.clone());

                    // 叠加马赛克：mosaic_scale 控制马赛克矩形自身放大比例；dpi_scale 用于前端坐标换算
                    let mosaic_scale = config::get_config()
                        .and_then(|c| c.monitoring)
                        .map(|m| m.mosaic_scale)
                        .unwrap_or(1.0f32);
                    crate::overlay::overlay::apply_mosaic(mapped_rects, mosaic_scale, monitor.scale_factor);
                }
                Err(e) => {
                    // 输出人脸检测用时（即便失败也记录耗时）
                    let face_elapsed_ms = face_start.elapsed().as_millis();
                    info!("[perf] face_detection {} ms", face_elapsed_ms);
                    error!("[cal] face processing failed: {}", e);
                }
            }
        }
        Err(e) => {
            error!("[cal] screen shot failed: {}", e);
            // 即便截图失败，也保证两行日志输出
            info!("[perf] face_detection 0 ms");
            return;  // 优雅退出而不是 panic
        }
    }
}

// 最近邻快速缩放 BGRA 图像
fn downscale_image_bgra(src: &screen_shot::Image, scale: f32) -> screen_shot::Image {
    let src_w = src.width.max(1) as usize;
    let src_h = src.height.max(1) as usize;
    let dst_w = ((src.width as f32) * scale).round().max(1.0) as usize;
    let dst_h = ((src.height as f32) * scale).round().max(1.0) as usize;
    if dst_w == src_w && dst_h == src_h {
        return src.clone();
    }

    let mut dst = vec![0u8; dst_w * dst_h * 4];
    let x_ratio = (src_w as f32) / (dst_w as f32);
    let y_ratio = (src_h as f32) / (dst_h as f32);

    for dy in 0..dst_h {
        let sy = (dy as f32 * y_ratio).floor() as usize;
        let sy = sy.min(src_h - 1);
        for dx in 0..dst_w {
            let sx = (dx as f32 * x_ratio).floor() as usize;
            let sx = sx.min(src_w - 1);
            let sidx = (sy * src_w + sx) * 4;
            let didx = (dy * dst_w + dx) * 4;
            dst[didx..didx+4].copy_from_slice(&src.data[sidx..sidx+4]);
        }
    }

    screen_shot::Image { width: dst_w as i32, height: dst_h as i32, data: dst }
}