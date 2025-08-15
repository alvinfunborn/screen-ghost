use crate::config;
use crate::monitor::Image;
use crate::utils::rect::Rect;
use crate::ai::python_env;
use log::{debug, info, warn};
use once_cell::sync::OnceCell;
use pyo3::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

static TARGET_EMBEDDINGS: OnceCell<RwLock<HashMap<String, Arc<Vec<f32>>>>> = OnceCell::new();

fn get_store() -> &'static RwLock<HashMap<String, Arc<Vec<f32>>>> {
    TARGET_EMBEDDINGS.get_or_init(|| RwLock::new(HashMap::new()))
}

pub fn initialize_face_recognition() -> Result<(), String> {
    // 确保 Python 环境
    if !python_env::is_python_ready() {
        return Err("Python environment is not ready".to_string());
    }

    Python::with_gil(|py| {
        let python_files_path = python_env::get_python_files_path()
            .map_err(|e| format!("Failed to get python files path: {}", e))?;

        let path_setup = format!(
            r#"
import sys
import os
sys.path.insert(0, r'{}')
"#,
            python_files_path.to_string_lossy()
        );

        py.run(&path_setup, None, None)
            .map_err(|e| format!("Failed to setup Python path: {}", e))?;

        // 先尝试正常导入，如失败则从文件路径回退加载
        let fallback_import = format!(
            r#"
import sys, os, importlib.util
module_name = 'face_recognition'
try:
    import face_recognition as mod
except Exception:
    file_path = os.path.join(r'{p}', 'face_recognition.py')
    if not os.path.exists(file_path):
        raise ModuleNotFoundError(f"face_recognition.py not found at {{file_path}}")
    spec = importlib.util.spec_from_file_location(module_name, file_path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    sys.modules[module_name] = mod
"#,
            p = python_files_path.to_string_lossy()
        );
        py.run(&fallback_import, None, None)
            .map_err(|e| format!("Failed to load face_recognition module: {}", e))?;

        let recog = py
            .import("face_recognition")
            .map_err(|e| format!("Failed to import face_recognition: {}", e))?;

        let ok: bool = recog
            .call_method1("init_model", ("cpu",))
            .map_err(|e| format!("Failed to call init_model: {}", e))?
            .extract()
            .map_err(|e| format!("Failed to extract init_model result: {}", e))?;

        if !ok {
            return Err("init_model returned false".to_string());
        }
        Ok(())
    })
}

pub fn preload_targets_from_faces_dir(app_handle: &tauri::AppHandle) -> Result<(), String> {
    let faces_dirs = resolve_faces_dirs(app_handle);
    if faces_dirs.is_empty() {
        warn!("[preload_targets] no faces directory found");
        return Ok(());
    }

    let mut total_loaded = 0usize;
    for dir in faces_dirs {
        if !dir.exists() { continue; }
        for entry in fs::read_dir(&dir).map_err(|e| format!("read_dir failed: {}", e))? {
            let entry = entry.map_err(|e| format!("dir entry err: {}", e))?;
            if !entry.file_type().map_err(|e| e.to_string())?.is_dir() { continue; }
            let person_id = entry.file_name().to_string_lossy().to_string();
            let person_dir = entry.path();
            let images = collect_images(&person_dir);
            if images.is_empty() { continue; }
            if let Some(embedding) = compute_person_embedding(&images)? {
                get_store().write().unwrap().insert(person_id.clone(), Arc::new(embedding));
                total_loaded += 1;
            }
        }
    }
    info!("[preload_targets] loaded {} persons", total_loaded);
    Ok(())
}

pub fn recognize_best(image: &Image) -> Result<Option<(Rect, String, f32)>, String> {
    let threshold = config::get_config().unwrap().face.unwrap().recognition.threshold;
    let rects = crate::ai::face_detect::face_detect(image)?;
    if rects.is_empty() { return Ok(None); }
    let store = get_store().read().unwrap();
    if store.is_empty() { return Ok(None); }

    let mut best: Option<(Rect, String, f32)> = None;
    for rect in &rects {
        if let Some(emb) = compute_embedding_from_image_rect(image, rect)? {
            for (person, target) in store.iter() {
                let score = cosine_similarity(&emb, target);
                if best.as_ref().map(|(_,_,s)| *s).unwrap_or(f32::MIN) < score {
                    best = Some((rect.clone(), person.clone(), score));
                }
            }
        }
    }
    if let Some((r, p, s)) = best {
        if s >= threshold { return Ok(Some((r, p, s))); }
    }
    Ok(None)
}

// 当没有任何目标（faces/为空或未加载）时，回退为“检测所有人脸”；
// 否则，仅返回识别命中的单个人脸框。
pub fn detect_targets_or_all_faces(image: &Image) -> Result<Vec<Rect>, String> {
    let store = get_store().read().unwrap();
    if store.is_empty() {
        debug!("[detect_targets_or_all_faces] no targets, fallback to detect all faces");
        // 无目标，回退为检测所有人脸
        let rects = crate::ai::face_detect::face_detect(image)?;
        return Ok(rects);
    }
    drop(store);

    debug!("[detect_targets_or_all_faces] targets found, return only the best one");
    // 有目标，仅返回识别命中的那一个人脸框
    match recognize_best(image)? {
        Some((rect, _person, _score)) => Ok(vec![rect]),
        None => Ok(Vec::new()),
    }
}

fn compute_person_embedding(images: &[(Vec<u8>, i32, i32)]) -> Result<Option<Vec<f32>>, String> {
    let mut embs: Vec<Vec<f32>> = Vec::new();
    for (bytes, _w, _h) in images.iter() {
        if let Some(emb) = call_python_compute_embedding(bytes)? { embs.push(emb); }
    }
    if embs.is_empty() { return Ok(None); }
    let dim = embs[0].len();
    let mut mean = vec![0f32; dim];
    for e in &embs { for i in 0..dim { mean[i] += e[i]; } }
    for i in 0..dim { mean[i] /= embs.len() as f32; }
    l2_normalize_inplace(&mut mean);
    Ok(Some(mean))
}

fn compute_embedding_from_image_rect(image: &Image, rect: &Rect) -> Result<Option<Vec<f32>>, String> {
    // 从 BGRA 图像裁剪 rect 并编码为 JPG，交给 Python
    let (x, y, w, h) = (rect.x, rect.y, rect.width, rect.height);
    let bytes = &image.data;
    let width = image.width as usize;
    let height = image.height as usize;
    if x < 0 || y < 0 || w <= 0 || h <= 0 { return Ok(None); }
    let (x0, y0) = (x as usize, y as usize);
    let (rw, rh) = (w as usize, h as usize);
    if x0+rw > width || y0+rh > height { return Ok(None); }

    // 转 BGR 并裁剪
    let mut bgr = Vec::with_capacity(width * height * 3);
    for row in 0..height {
        let start = row * width * 4;
        for col in 0..width {
            let idx = start + col*4;
            let b = bytes[idx];
            let g = bytes[idx+1];
            let r = bytes[idx+2];
            bgr.extend_from_slice(&[b,g,r]);
        }
    }
    // 裁剪 ROI
    let mut roi = Vec::with_capacity(rw * rh * 3);
    for row in 0..rh {
        let src_row = (y0 + row) * width * 3;
        let src_start = src_row + x0 * 3;
        let src_end = src_start + rw * 3;
        roi.extend_from_slice(&bgr[src_start..src_end]);
    }

    // 简单用 OpenCV 侧编码（在 Python 做），这里直接把整幅图交给 Python，让其内部检测对齐会更稳
    // 因为已有人脸框，这里直接用整幅原图 bytes 让 Python 自行检测对齐，避免我们在 Rust 手写编码
    call_python_compute_embedding(&image.data)
}

fn call_python_compute_embedding(image_bytes: &[u8]) -> Result<Option<Vec<f32>>, String> {
    Python::with_gil(|py| {
        let python_files_path = python_env::get_python_files_path()
            .map_err(|e| format!("Failed to get python files path: {}", e))?;
        let path_setup = format!(
            r#"
import sys
import os
sys.path.insert(0, r'{}')
"#,
            python_files_path.to_string_lossy()
        );
        py.run(&path_setup, None, None)
            .map_err(|e| format!("Failed to setup Python path: {}", e))?;
        let fallback_import = format!(
            r#"
import sys, os, importlib.util
module_name = 'face_recognition'
try:
    import face_recognition as mod
except Exception:
    file_path = os.path.join(r'{p}', 'face_recognition.py')
    if not os.path.exists(file_path):
        raise ModuleNotFoundError(f"face_recognition.py not found at {{file_path}}")
    spec = importlib.util.spec_from_file_location(module_name, file_path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    sys.modules[module_name] = mod
"#,
            p = python_files_path.to_string_lossy()
        );
        py.run(&fallback_import, None, None)
            .map_err(|e| format!("Failed to load face_recognition module: {}", e))?;
        let recog = py
            .import("face_recognition")
            .map_err(|e| format!("Failed to import face_recognition: {}", e))?;
        let py_bytes = pyo3::types::PyBytes::new(py, image_bytes);
        let result: Option<Vec<f32>> = recog
            .call_method1("compute_embedding", (py_bytes,))
            .map_err(|e| format!("Failed to call compute_embedding: {}", e))?
            .extract()
            .map_err(|e| format!("Failed to extract compute_embedding: {}", e))?;
        Ok(result)
    })
}

fn resolve_faces_dirs(_app_handle: &tauri::AppHandle) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    // 1) exe 同级 faces
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            dirs.push(dir.join("faces"));
        }
    }
    // 2) 开发环境兜底：工作区根目录 faces（exe 切换目录策略可能已指向可执行目录，此处作为备选）
    dirs.push(PathBuf::from("../faces"));
    dirs.push(PathBuf::from("faces"));
    dirs
}

fn collect_images(person_dir: &Path) -> Vec<(Vec<u8>, i32, i32)> {
    let mut images = Vec::new();
    if let Ok(entries) = fs::read_dir(person_dir) {
        for e in entries.flatten() {
            let path = e.path();
            if !is_image_file(&path) { continue; }
            if let Ok(bytes) = fs::read(path) {
                // 仅传 bytes 给 Python 做解码与检测，不需要宽高
                images.push((bytes, 0, 0));
            }
        }
    }
    images
}

fn is_image_file(path: &Path) -> bool {
    match path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
        Some(ext) if ["jpg","jpeg","png","webp","bmp"].contains(&ext.as_str()) => true,
        _ => false,
    }
}

fn l2_normalize_inplace(v: &mut [f32]) {
    let mut sum = 0f32;
    for &x in v.iter() { sum += x * x; }
    let n = sum.sqrt();
    if n > 0.0 { for x in v.iter_mut() { *x /= n; } }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let mut dot = 0f32;
    for i in 0..a.len().min(b.len()) { dot += a[i] * b[i]; }
    dot
}


