use crate::mosaic::Mosaic;
use crate::utils::rect::Rect;
use log::{debug};
use std::sync::{OnceLock, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};
use serde_json::Value;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::app::AppState;
use tauri::Emitter;
use crate::overlay::overlay_state::OverlayState;
// 样式在窗口创建时一次性下发，apply_mosaic 不再读取样式

static LATEST_MOSAIC: OnceLock<Mutex<Option<Value>>> = OnceLock::new();
static SEQ: AtomicU64 = AtomicU64::new(0);

// 最近一次需要主动推送给前端的 payload（仅保留最新），按 ~60fps 节流
static MOSAIC_EMIT_BUF: OnceLock<Mutex<Option<Value>>> = OnceLock::new();
static MOSAIC_EMIT_THREAD: OnceLock<()> = OnceLock::new();

fn set_latest(payload: &Value) {
    let lock = LATEST_MOSAIC.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = lock.lock() {
        *guard = Some(payload.clone());
    }
}

fn set_latest_for_emit(payload: &Value) {
    let lock = MOSAIC_EMIT_BUF.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = lock.lock() {
        *guard = Some(payload.clone());
    }
}

fn spawn_emit_thread_once() {
    MOSAIC_EMIT_THREAD.get_or_init(|| {
        std::thread::spawn(|| {
            loop {
                // 16ms 节拍（~60fps）
                std::thread::sleep(Duration::from_millis(16));

                let payload_opt = {
                    let lock = MOSAIC_EMIT_BUF.get_or_init(|| Mutex::new(None));
                    if let Ok(mut guard) = lock.lock() {
                        guard.take()
                    } else {
                        None
                    }
                };

                if let Some(mut payload) = payload_opt {
                    // 在投递前记录发送时间戳（毫秒）
                    let emit_ms = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
                    if let serde_json::Value::Object(ref mut map) = payload {
                        map.insert("emit_ts".to_string(), serde_json::json!(emit_ms));
                    }
                    // 优先单播到 overlay 窗口，避免广播开销；若不存在则退回到全局广播
                    if let Some(window) = OverlayState::get_window() {
                        let _ = window.emit("mosaic-update", payload.clone());
                    } else if let Ok(app) = AppState::get_global() {
                        let handle = app.handle;
                        let _ = handle.emit("mosaic-update", payload);
                    }
                }
            }
        });
    });
}

pub fn get_latest_mosaic_payload() -> Option<Value> {
    let lock = LATEST_MOSAIC.get_or_init(|| Mutex::new(None));
    lock.lock().ok().and_then(|g| g.clone())
}

pub fn apply_mosaic(rects: Vec<Rect>, mosaic_scale: f32, dpi_scale: f64) {
    // 在发送给 overlay 前进行缩放：保持中心不变
    // 公式：w' = w*s, h' = h*s, x' = x - (w' - w)/2, y' = y - (h' - h)/2
    let s = mosaic_scale;
    let mosaics: Vec<Mosaic> = rects
        .into_iter()
        .map(|rect| {
            let new_w_f = (rect.width as f32) * s;
            let new_h_f = (rect.height as f32) * s;
            let dx = ((new_w_f - rect.width as f32) / 2.0).round() as i32;
            let dy = ((new_h_f - rect.height as f32) / 2.0).round() as i32;
            let w = new_w_f.round() as i32;
            let h = new_h_f.round() as i32;
            let x = rect.x - dx;
            let y = rect.y - dy;
            Mosaic { x, y, width: w, height: h }
        })
        .collect();
    
    debug!("[apply_mosaic] Applying {} mosaics (mosaic_scale={}, dpi_scale={})", mosaics.len(), mosaic_scale, dpi_scale);
    
    // 生成 payload，并更新最新缓存（供前端轮询获取最新状态）
    let seq = SEQ.fetch_add(1, Ordering::SeqCst) + 1;
    // 附带服务端生成时间戳（毫秒），用于端到端延迟测量
    let now_ms: i64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let payload = serde_json::json!({
        "mosaics": mosaics,
        // 传给前端用于 DPI 适配（overlay.html 按此除以坐标）
        "scale_factor": dpi_scale,
        "seq": seq,
        "ts": now_ms
    });
    set_latest(&payload);
    // 主动按 60fps 推送最新一帧到前端（只发最新，不合并）
    set_latest_for_emit(&payload);
    spawn_emit_thread_once();
}