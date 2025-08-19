use tauri::Emitter;
use std::sync::{OnceLock, Mutex, Condvar};
use crate::{app::AppState, monitor::Image, utils::rect::Rect};

struct ImageEmitQueue {
	buf: Mutex<Option<Image>>, // 仅保留最新一帧
	cv: Condvar,
}

static IMAGE_QUEUE: OnceLock<ImageEmitQueue> = OnceLock::new();
static IMAGE_EMIT_THREAD: OnceLock<()> = OnceLock::new();

fn image_queue() -> &'static ImageEmitQueue {
	IMAGE_QUEUE.get_or_init(|| ImageEmitQueue {
		buf: Mutex::new(None),
		cv: Condvar::new(),
	})
}

fn spawn_image_emit_thread_once() {
	IMAGE_EMIT_THREAD.get_or_init(|| {
		std::thread::spawn(|| {
			let q = image_queue();
			loop {
				// 等待有最新一帧
				let mut guard = q.buf.lock().unwrap();
				while guard.is_none() {
					guard = q.cv.wait(guard).unwrap();
				}
				let img = guard.take().unwrap();
				drop(guard);

				// 串行发送，确保不并行 emit；默认关闭，仅在 DEBUG_IMAGE_STREAM=1 时开启
				let enable = std::env::var("DEBUG_IMAGE_STREAM").ok().as_deref() == Some("1");
				if enable {
					if let Ok(app) = AppState::get_global() {
						let handle = app.handle;
						let _ = handle.emit("image", img);
					}
				}
			}
		});
	});
}

pub fn emit_image(image: &Image) {
	// 仅在显式开启 DEBUG_IMAGE_STREAM=1 时才启用图像事件流
	let enable = std::env::var("DEBUG_IMAGE_STREAM").ok().as_deref() == Some("1");
	if !enable { return; }
	// 后台串行线程发送：仅覆盖为最新帧
	spawn_image_emit_thread_once();
	let q = image_queue();
	if let Ok(mut guard) = q.buf.lock() {
		*guard = Some(image.clone());
		q.cv.notify_one();
	}
}

pub fn emit_toast(message: &str) {
    let app = AppState::get_global().unwrap();
    let handle = app.handle;
    let _ = handle.emit("toast", message.to_string());
}

pub fn emit_toast_close() {
    emit_toast("close");
}

pub fn emit_frame_info(frame_info: Vec<Rect>) {
    let app = AppState::get_global().unwrap();
    let handle = app.handle;
    handle.emit("frame_info", frame_info).unwrap();
}