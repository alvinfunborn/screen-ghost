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

def init_model(provider: str = "auto") -> bool:
    global _APP
    if _APP is not None:
        return True
    try:
        from insightface.app import FaceAnalysis
        # 根据可用性选择最优 provider
        providers = None
        try:
            import onnxruntime as ort
            avail = set(ort.get_available_providers())
        except Exception:
            avail = set()
        pl = provider.lower()
        if pl == 'auto':
            if 'CUDAExecutionProvider' in avail:
                providers = ["CUDAExecutionProvider", "CPUExecutionProvider"]
            elif 'DmlExecutionProvider' in avail:
                providers = ["DmlExecutionProvider", "CPUExecutionProvider"]
            else:
                providers = ["CPUExecutionProvider"]
        elif pl == 'cpu':
            providers = ["CPUExecutionProvider"]
        elif pl == 'cuda':
            providers = ["CUDAExecutionProvider", "CPUExecutionProvider"]
        elif pl == 'dml':
            providers = ["DmlExecutionProvider", "CPUExecutionProvider"]
        else:
            providers = ["CPUExecutionProvider"]

        app = FaceAnalysis(name='buffalo_l', providers=providers)
        app.prepare(ctx_id=0, det_size=(640, 640))
        _APP = app
        return True
    except Exception as e:
        print(f"init_model failed with provider {provider}: {e}")
        # 若 CUDA 失败且 DML 可用，则尝试 DML，再不行回退 CPU
        try:
            import onnxruntime as ort
            avail = set(ort.get_available_providers())
        except Exception:
            avail = set()
        # 优先 DML 回退
        if 'DmlExecutionProvider' in avail:
            try:
                from insightface.app import FaceAnalysis
                app = FaceAnalysis(name='buffalo_l', providers=["DmlExecutionProvider", "CPUExecutionProvider"])
                app.prepare(ctx_id=0, det_size=(640, 640))
                _APP = app
                return True
            except Exception as e2:
                print(f"fallback DML init failed: {e2}")
        # 最后 CPU
        try:
            from insightface.app import FaceAnalysis
            app = FaceAnalysis(name='buffalo_l', providers=["CPUExecutionProvider"])
            app.prepare(ctx_id=0, det_size=(640, 640))
            _APP = app
            return True
        except Exception as e3:
            print(f"fallback CPU init failed: {e3}")
            _APP = None
            return False


def _ensure_model() -> None:
    if _APP is None:
        ok = init_model('auto')
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
    行为统一：
    - 若存在目标库(_TARGETS 非空)且识别模型可用：按与检测相同的 image_scale 缩放整图，使用 InsightFace 检测+嵌入，选出命中最佳目标并返回其框。
    - 否则：按现有配置走 Haar 全人脸检测并返回所有人脸框。
    """
    # 若存在目标，优先走“目标检测”路径
    if _TARGETS:
        try:
            if not init_model('auto'):
                # 模型不可用则退回普通检测
                raise RuntimeError('model init failed')

            # 解码 BGRA，并按 image_scale 进行统一缩放
            arr = np.frombuffer(image_data, dtype=np.uint8).reshape(height, width, 4)
            bgr = cv2.cvtColor(arr, cv2.COLOR_BGRA2BGR)
            scale = float(image_scale) if image_scale and image_scale > 0 else 1.0
            if abs(scale - 1.0) > 1e-6:
                sw = max(1, int(round(width * scale)))
                sh = max(1, int(round(height * scale)))
                bgr_scaled = cv2.resize(bgr, (sw, sh), interpolation=cv2.INTER_LINEAR)
                inv = 1.0 / scale
            else:
                bgr_scaled = bgr
                inv = 1.0

            # 在缩放后的图上运行 InsightFace 检测+嵌入
            faces_info = _APP.get(bgr_scaled)
            if not faces_info:
                return []

            thr = float(recognition_threshold) if recognition_threshold is not None else float(_RECOG_THRESHOLD)

            def cosine(a: np.ndarray, b: np.ndarray) -> float:
                return float(np.dot(a, b))

            best_bbox = None
            best_score = -1.0
            for f in faces_info:
                emb = f.normed_embedding
                if emb is None:
                    continue
                emb = np.asarray(emb, dtype=np.float32)
                # 与目标库计算相似度（目标已归一化，InsightFace 输出通常已归一化）
                for _person, target in _TARGETS.items():
                    score = cosine(emb, target)
                    if score > best_score:
                        best_score = score
                        best_bbox = f.bbox

            if best_bbox is not None and best_score >= thr:
                x0, y0, x1, y1 = map(float, best_bbox)
                # 映射回原分辨率
                x0 = int(round(x0 * inv)); y0 = int(round(y0 * inv))
                x1 = int(round(x1 * inv)); y1 = int(round(y1 * inv))
                x0 = max(0, min(x0, width - 1))
                y0 = max(0, min(y0, height - 1))
                x1 = max(x0 + 1, min(x1, width))
                y1 = max(y0 + 1, min(y1, height))
                w = max(1, x1 - x0)
                h = max(1, y1 - y0)
                return [(x0, y0, w, h)]

            return []
        except Exception:
            # 任意异常回退到普通检测
            pass

    # 普通全人脸检测（统一使用相同 image_scale）
    return detect_faces_with_config(
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


# 旧的“基于已有检测框再识别”与“整图重新检测再识别”逻辑已移除，
# 统一由 detect_targets_or_all_faces 在单入口内根据 _TARGETS 与 image_scale 决策。


 


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

