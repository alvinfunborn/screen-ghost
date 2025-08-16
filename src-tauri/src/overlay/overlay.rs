use crate::mosaic::Mosaic;
use crate::utils::rect::Rect;
use tauri::Emitter;
use log::{error, info};
// 样式在窗口创建时一次性下发，apply_mosaic 不再读取样式

pub fn apply_mosaic(rects: Vec<Rect>, scale_factor: f64) {
    // 在发送给 overlay 前进行缩放：保持中心不变
    // 公式：w' = w*s, h' = h*s, x' = x - (w' - w)/2, y' = y - (h' - h)/2
    let s = scale_factor as f32;
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
    
    info!("[apply_mosaic] Applying {:?} mosaics with scale_factor: {}", mosaics, scale_factor);
    
    // 获取overlay窗口并发送马赛克数据和scale_factor（不再携带样式）
    if let Some(window) = crate::overlay::OverlayState::get_window() {
        let payload = serde_json::json!({
            "mosaics": mosaics,
            "scale_factor": scale_factor
        });
        
        if let Err(e) = window.emit("apply-mosaic", &payload) {
            error!("[apply_mosaic] Failed to emit apply-mosaic event: {}", e);
        }
    } else {
        error!("[apply_mosaic] Overlay window not found");
    }
}