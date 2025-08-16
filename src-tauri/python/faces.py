import cv2
import numpy as np
import os
import io
from typing import List, Tuple, Optional

# 统一入口：检测与识别（内部自带检测实现）

# —— 识别模型（InsightFace）合并至本文件 ——
_APP = None
_TARGETS = {}
_RECOG_THRESHOLD = 0.35


def init_model(provider: str = "cpu") -> bool:
    global _APP
    if _APP is not None:
        return True
    try:
        from insightface.app import FaceAnalysis
        providers = None
        pl = provider.lower()
        if pl == 'cpu':
            providers = ["CPUExecutionProvider"]
        elif pl == 'cuda':
            providers = ["CUDAExecutionProvider", "CPUExecutionProvider"]
        elif pl == 'dml':
            providers = ["DmlExecutionProvider", "CPUExecutionProvider"]

        app = FaceAnalysis(name='buffalo_l', providers=providers)
        app.prepare(ctx_id=0, det_size=(640, 640))
        _APP = app
        return True
    except Exception as e:
        print(f"init_model failed: {e}")
        _APP = None
        return False


def _ensure_model() -> None:
    if _APP is None:
        ok = init_model('cpu')
        if not ok:
            raise RuntimeError('face recognition model not initialized')


def compute_embedding(image_bytes: bytes) -> Optional[List[float]]:
    """
    从编码图像字节（jpg/png 等）计算 L2 归一化后的特征向量
    返回 None 表示未检测到人脸
    """
    try:
        _ensure_model()
        # 优先走 Pillow + EXIF 矫正，失败则回退 OpenCV 解码
        img = None
        try:
            from PIL import Image, ImageOps
            pil = Image.open(io.BytesIO(image_bytes))
            pil = ImageOps.exif_transpose(pil)
            img_rgb = np.array(pil)
            if img_rgb.ndim == 2:  # 灰度
                img = cv2.cvtColor(img_rgb, cv2.COLOR_GRAY2BGR)
            else:
                img = cv2.cvtColor(img_rgb, cv2.COLOR_RGB2BGR)
        except Exception:
            arr = np.frombuffer(image_bytes, dtype=np.uint8)
            img = cv2.imdecode(arr, cv2.IMREAD_COLOR)
        if img is None:
            return None
        faces = _APP.get(img)
        if not faces:
            return None
        faces.sort(key=lambda f: (f.bbox[2]-f.bbox[0]) * (f.bbox[3]-f.bbox[1]), reverse=True)
        face = faces[0]
        emb = face.normed_embedding
        if emb is None:
            return None
        emb = np.asarray(emb, dtype=np.float32)
        norm = np.linalg.norm(emb)
        if norm > 0:
            emb = emb / norm
        return emb.astype(np.float32).tolist()
    except Exception as e:
        print(f"compute_embedding failed: {e}")
        return None


def batch_compute_embeddings(images_bytes: List[bytes]) -> List[Optional[List[float]]]:
    _ensure_model()
    results: List[Optional[List[float]]] = []
    for data in images_bytes:
        results.append(compute_embedding(data))
    return results


def detect_targets_or_all_faces(
    image_data: bytes,
    width: int,
    height: int,
    use_gray: bool,
    image_scale: float,
    min_face_size: int,
    max_face_size: int,
    scale_factor: float,
    min_neighbors: int,
    confidence_threshold: float,
    recognition_threshold: float | None = None,
) -> List[Tuple[int,int,int,int]]:
    """
    若识别模型可用：尝试根据整图识别返回最大目标的人脸框；
    否则：返回所有检测到的人脸框。
    """
    # 先做检测（使用内置可配置入口）
    faces = detect_faces_with_config(
        image_data,
        width,
        height,
        use_gray=bool(use_gray),
        image_scale=float(image_scale),
        min_face_size=int(min_face_size),
        max_face_size=int(max_face_size),
        scale_factor=float(scale_factor),
        min_neighbors=int(min_neighbors),
        confidence_threshold=float(confidence_threshold),
    )

    if not faces:
        return faces

    # 若没有目标库，直接返回检测结果
    if not _TARGETS:
        return faces

    # 若识别不可用或初始化失败，返回检测结果
    if not init_model('cpu'):
        return faces

    # 计算与目标库的相似度，命中则只返回最佳目标
    try:
        thr = float(recognition_threshold) if recognition_threshold is not None else float(_RECOG_THRESHOLD)
        arr = np.frombuffer(image_data, dtype=np.uint8).reshape(height, width, 4)
        bgr = cv2.cvtColor(arr, cv2.COLOR_BGRA2BGR)
        all_faces = _APP.get(bgr) if _APP is not None else []
        if not all_faces:
            return []

        def cosine(a, b):
            return float(np.dot(a, b))

        best = None
        best_score = -1.0
        for f in all_faces:
            emb = f.normed_embedding
            if emb is None:
                continue
            emb = np.asarray(emb, dtype=np.float32)
            # emb 已归一化
            for _person, target in _TARGETS.items():
                score = cosine(emb, target)
                if score > best_score:
                    best_score = score
                    best = f
        if best is not None and best_score >= thr:
            x0, y0, x1, y1 = map(int, best.bbox)
            w = max(1, x1 - x0)
            h = max(1, y1 - y0)
            return [(x0, y0, w, h)]
        # 目标库已存在但未命中阈值：返回空（保持“仅返回命中目标”的语义）
        return []
    except Exception as e:
        print(f"recognition selection failed: {e}")
        return []


def get_face_cascade() -> cv2.CascadeClassifier:
    global _FACE_CASCADE
    try:
        return _FACE_CASCADE
    except NameError:
        pass
    _FACE_CASCADE = cv2.CascadeClassifier(cv2.data.haarcascades + 'haarcascade_frontalface_alt2.xml')
    if _FACE_CASCADE.empty():
        _FACE_CASCADE = cv2.CascadeClassifier(cv2.data.haarcascades + 'haarcascade_frontalface_default.xml')
    return _FACE_CASCADE


def detect_faces_with_config(
    image_data: bytes,
    width: int,
    height: int,
    use_gray: bool,
    image_scale: float,
    min_face_size: int,
    max_face_size: int,
    scale_factor: float,
    min_neighbors: int,
    confidence_threshold: float,
) -> List[Tuple[int, int, int, int]]:
    try:
        # 解码 BGRA 到所需颜色空间
        arr = np.frombuffer(image_data, dtype=np.uint8)
        img = arr.reshape(height, width, 4)
        if use_gray:
            working = cv2.cvtColor(img, cv2.COLOR_BGRA2GRAY)
        else:
            working = cv2.cvtColor(img, cv2.COLOR_BGRA2BGR)

        # 可选缩放
        scale = float(image_scale) if image_scale and image_scale > 0 else 1.0
        if scale != 1.0:
            new_w = max(1, int(width * scale))
            new_h = max(1, int(height * scale))
            working = cv2.resize(working, (new_w, new_h), interpolation=cv2.INTER_LINEAR)
        else:
            new_w, new_h = width, height

        face_cascade = get_face_cascade()

        if use_gray:
            gray_input = working
        else:
            gray_input = cv2.cvtColor(working, cv2.COLOR_BGR2GRAY)

        gray_input = cv2.equalizeHist(gray_input)

        min_size = (int(max(1, min_face_size)), int(max(1, min_face_size)))
        max_size = (int(max(1, max_face_size)), int(max(1, max_face_size)))

        faces = face_cascade.detectMultiScale(
            gray_input,
            scaleFactor=float(scale_factor),
            minNeighbors=int(min_neighbors),
            minSize=min_size,
            maxSize=max_size,
        )

        # 简单过滤
        result: List[Tuple[int, int, int, int]] = []
        inv_scale = 1.0 / scale if scale != 1.0 else 1.0
        for (x, y, w, h) in faces:
            ox = int(x * inv_scale)
            oy = int(y * inv_scale)
            ow = int(w * inv_scale)
            oh = int(h * inv_scale)
            ox = max(0, min(ox, width - 1))
            oy = max(0, min(oy, height - 1))
            ow = max(1, min(ow, width - ox))
            oh = max(1, min(oh, height - oy))
            result.append((ox, oy, ow, oh))

        return result
    except Exception as e:
        print(f"detect_faces_with_config error: {e}")
        return []


def _candidate_faces_dirs() -> list[str]:
    import sys, os
    cands = []
    cwd = os.getcwd()
    exe_dir = os.path.dirname(sys.executable) if hasattr(sys, 'executable') else cwd
    for base in [cwd, os.path.join(cwd, '..'), exe_dir, os.path.join(exe_dir, '..')]:
        cands.append(os.path.abspath(os.path.join(base, 'faces')))
    # 去重保序
    seen = set()
    uniq = []
    for p in cands:
        if p not in seen:
            seen.add(p)
            uniq.append(p)
    return uniq


def _l2_normalize(v: np.ndarray) -> np.ndarray:
    n = np.linalg.norm(v)
    return v / n if n > 0 else v


def _mean_embedding(embs: list[np.ndarray]) -> Optional[np.ndarray]:
    if not embs:
        return None
    arr = np.stack(embs, axis=0)
    mean = arr.mean(axis=0)
    return _l2_normalize(mean.astype(np.float32))


def _filter_outliers(embs: list[np.ndarray], thr: float = 0.3, max_iter: int = 2) -> list[np.ndarray]:
    # 迭代剔除与当前均值相似度过低的样本
    kept = [e for e in embs]
    for _ in range(max_iter):
        if len(kept) <= 1:
            return kept
        mean = _mean_embedding(kept)
        if mean is None:
            return kept
        filtered = []
        for e in kept:
            score = float(np.dot(e, mean))
            if score >= thr:
                filtered.append(e)
        if len(filtered) == len(kept):
            return kept
        kept = filtered
    return kept


def preload_targets_from_faces_dir(
    outlier_threshold: float | None = None,
    outlier_iter: int | None = None,
) -> dict:
    """从候选 faces 目录加载每人均值特征，存入全局 _TARGETS。返回已加载人员计数。"""
    try:
        _ensure_model()
    except Exception:
        # 无法初始化识别模型，清空并返回
        _TARGETS.clear()
        return {"loaded": 0}

    loaded = 0
    for root in _candidate_faces_dirs():
        if not os.path.isdir(root):
            continue
        for name in os.listdir(root):
            person_dir = os.path.join(root, name)
            if not os.path.isdir(person_dir):
                continue
            embs = []
            for fname in os.listdir(person_dir):
                if not fname.lower().split('.')[-1] in { 'jpg','jpeg','png','webp','bmp' }:
                    continue
                fpath = os.path.join(person_dir, fname)
                try:
                    with open(fpath, 'rb') as f:
                        data = f.read()
                    emb = compute_embedding(data)
                    if emb is not None:
                        embs.append(np.asarray(emb, dtype=np.float32))
                except Exception:
                    pass
            # 剔除离群样本后再求均值
            thr = float(outlier_threshold) if outlier_threshold is not None else 0.3
            iters = int(outlier_iter) if outlier_iter is not None else 2
            embs = _filter_outliers(embs, thr=thr, max_iter=iters)
            mean = _mean_embedding(embs)
            if mean is not None:
                _TARGETS[name] = mean
                loaded += 1
    return {"loaded": loaded}

