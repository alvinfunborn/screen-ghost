use crate::mosaic::Mosaic;
use crate::utils::rect::Rect;
use tauri::Emitter;
use log::{info, error};

pub fn apply_mosaic(rects: Vec<Rect>, scale_factor: f64) {
    // 将Rect转换为Mosaic
    let mosaics: Vec<Mosaic> = rects.into_iter()
        .map(|rect| Mosaic {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        })
        .collect();
    
    info!("[apply_mosaic] Applying {} mosaics with scale_factor: {}", mosaics.len(), scale_factor);
    
    // 获取overlay窗口并发送马赛克数据和scale_factor
    if let Some(window) = crate::overlay::OverlayState::get_window() {
        let payload = serde_json::json!({
            "mosaics": mosaics,
            "scale_factor": scale_factor
        });
        
        if let Err(e) = window.emit("apply-mosaic", &payload) {
            error!("[apply_mosaic] Failed to emit apply-mosaic event: {}", e);
        } else {
            info!("[apply_mosaic] Successfully sent {} mosaics to overlay with scale_factor: {}", mosaics.len(), scale_factor);
        }
    } else {
        error!("[apply_mosaic] Overlay window not found");
    }
}