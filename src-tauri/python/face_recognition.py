import cv2
import numpy as np
from typing import List, Optional

_APP = None


def init_model(provider: str = "cpu") -> bool:
    global _APP
    if _APP is not None:
        return True
    try:
        # 延迟导入，避免未安装时报错影响其余模块
        from insightface.app import FaceAnalysis
        # provider 可选: 'cpu', 'cuda', 'dml'
        providers = None
        if provider.lower() == 'cpu':
            providers = ["CPUExecutionProvider"]
        elif provider.lower() == 'cuda':
            providers = ["CUDAExecutionProvider", "CPUExecutionProvider"]
        elif provider.lower() == 'dml':
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
        arr = np.frombuffer(image_bytes, dtype=np.uint8)
        img = cv2.imdecode(arr, cv2.IMREAD_COLOR)
        if img is None:
            return None
        faces = _APP.get(img)
        if not faces:
            return None
        # 取最大的人脸
        faces.sort(key=lambda f: (f.bbox[2]-f.bbox[0]) * (f.bbox[3]-f.bbox[1]), reverse=True)
        face = faces[0]
        emb = face.normed_embedding
        if emb is None:
            return None
        # 确保为 L2 归一化
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


