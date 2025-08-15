import cv2
import numpy as np
import time
from typing import List, Tuple

# 全局缓存分类器实例，避免重复加载
_FACE_CASCADE = None
_FACE_CASCADE_ALT = None

def get_face_cascade() -> cv2.CascadeClassifier:
    """获取人脸检测器实例，全局缓存"""
    global _FACE_CASCADE
    if _FACE_CASCADE is None:
        _FACE_CASCADE = cv2.CascadeClassifier(cv2.data.haarcascades + 'haarcascade_frontalface_alt2.xml')
        if _FACE_CASCADE.empty():
            print("Warning: haarcascade_frontalface_alt2.xml not found, using default")
            _FACE_CASCADE = cv2.CascadeClassifier(cv2.data.haarcascades + 'haarcascade_frontalface_default.xml')
    return _FACE_CASCADE

def get_face_cascade_alt() -> cv2.CascadeClassifier:
    """获取备用分类器实例"""
    global _FACE_CASCADE_ALT
    if _FACE_CASCADE_ALT is None:
        _FACE_CASCADE_ALT = cv2.CascadeClassifier(cv2.data.haarcascades + 'haarcascade_frontalface_default.xml')
    return _FACE_CASCADE_ALT

def detect_faces(image_data: bytes, width: int, height: int) -> List[Tuple[int, int, int, int]]:
    """
    人脸检测函数（优化版本）
    
    Args:
        image_data: BGRA格式的图像字节数据
        width: 图像宽度
        height: 图像高度
    
    Returns:
        List[Tuple[int, int, int, int]]: 检测到的人脸矩形 (x, y, width, height)
    """
    start_time = time.time()
    
    try:
        # 将字节数据转换为numpy数组
        image_array = np.frombuffer(image_data, dtype=np.uint8)
        image_array = image_array.reshape(height, width, 4)  # BGRA格式
        
        # 转换为BGR格式（OpenCV默认格式）
        bgr_image = cv2.cvtColor(image_array, cv2.COLOR_BGRA2BGR)
        
        # 转换为灰度图像以提高性能
        gray_image = cv2.cvtColor(bgr_image, cv2.COLOR_BGR2GRAY)
        
        # 直方图均衡化以提高对比度
        gray_image = cv2.equalizeHist(gray_image)
        
        # 使用缓存的分类器实例
        face_cascade = get_face_cascade()
        
        # 动态参数：根据分辨率调整
        min_size = max(20, min(width, height) // 20)
        max_size = min(300, min(width, height) // 3)
        
        # 人脸检测参数
        scale_factor = 1.1
        min_neighbors = 3
        
        # 执行人脸检测
        faces = face_cascade.detectMultiScale(
            gray_image,
            scaleFactor=scale_factor,
            minNeighbors=min_neighbors,
            minSize=(min_size, min_size),
            maxSize=(max_size, max_size)
        )
        
        # 后处理：置信度过滤和非极大值抑制
        filtered_faces = post_process_faces(faces, confidence_threshold=0.5)
        
        # 转换为列表格式
        result = []
        for (x, y, w, h) in filtered_faces:
            # 确保坐标在图像范围内
            x = max(0, min(x, width - w))
            y = max(0, min(y, height - h))
            w = min(w, width - x)
            h = min(h, height - y)
            result.append((x, y, w, h))
        
        elapsed = (time.time() - start_time) * 1000
        print(f"[detect_faces] Processed {width}x{height} in {elapsed:.1f}ms, found {len(result)} faces")
        
        return result
        
    except Exception as e:
        print(f"Face detection error: {e}")
        return []

def detect_faces_gray(gray_data: bytes, width: int, height: int, scale: float = 1.0) -> List[Tuple[int, int, int, int]]:
    """
    高性能版本：直接处理灰度图像，避免颜色空间转换
    
    Args:
        gray_data: 灰度图像字节数据（单通道）
        width: 图像宽度
        height: 图像高度
        scale: 图像缩放比例（1.0为原始大小）
    
    Returns:
        检测到的人脸矩形列表
    """
    start_time = time.time()
    
    try:
        # 直接重塑为灰度图像，避免转换
        gray_array = np.frombuffer(gray_data, dtype=np.uint8)
        gray_image = gray_array.reshape(height, width)
        
        # 如果指定了缩放，进行图像缩放
        if scale != 1.0:
            new_width = int(width * scale)
            new_height = int(height * scale)
            gray_image = cv2.resize(gray_image, (new_width, new_height), interpolation=cv2.INTER_LINEAR)
            width, height = new_width, new_height
        
        # 动态参数：根据分辨率调整
        min_size = max(15, min(width, height) // 25)  # 更激进的最小尺寸
        max_size = min(250, min(width, height) // 4)  # 更保守的最大尺寸
        
        # 使用缓存的分类器实例
        face_cascade = get_face_cascade()
        
        # 高性能参数
        faces = face_cascade.detectMultiScale(
            gray_image,
            scaleFactor=1.15,  # 稍微激进一点，减少检测层数
            minNeighbors=2,    # 降低误检
            minSize=(min_size, min_size),
            maxSize=(max_size, max_size)
        )
        
        # 简单过滤和坐标调整
        result = []
        for (x, y, w, h) in faces:
            # 如果图像被缩放，需要调整回原始坐标
            if scale != 1.0:
                x = int(x / scale)
                y = int(y / scale)
                w = int(w / scale)
                h = int(h / scale)
            
            # 确保坐标在图像范围内
            x = max(0, min(x, width - w))
            y = max(0, min(y, height - h))
            w = min(w, width - x)
            h = min(h, height - y)
            result.append((x, y, w, h))
        
        elapsed = (time.time() - start_time) * 1000
        print(f"[detect_faces_gray] Processed {width}x{height} (scale={scale}) in {elapsed:.1f}ms, found {len(result)} faces")
        
        return result
        
    except Exception as e:
        print(f"Gray face detection error: {e}")
        return []

def post_process_faces(faces: np.ndarray, confidence_threshold: float = 0.5) -> np.ndarray:
    """
    后处理人脸检测结果
    
    Args:
        faces: 检测到的人脸数组
        confidence_threshold: 置信度阈值
    
    Returns:
        过滤后的人脸数组
    """
    if len(faces) == 0:
        return faces
    
    # 转换为列表进行过滤
    face_list = []
    for (x, y, w, h) in faces:
        confidence = calculate_confidence(x, y, w, h)
        if confidence >= confidence_threshold:
            face_list.append((x, y, w, h))
    
    # 非极大值抑制
    filtered_faces = non_maximum_suppression(face_list, overlap_threshold=0.3)
    
    return np.array(filtered_faces) if filtered_faces else np.empty((0, 4))

def calculate_confidence(x: int, y: int, w: int, h: int) -> float:
    """
    计算人脸检测的置信度
    
    Args:
        x, y, w, h: 人脸矩形坐标
    
    Returns:
        置信度分数 (0.0-1.0)
    """
    area = w * h
    aspect_ratio = w / h if h > 0 else 0
    
    # 理想人脸宽高比约为1.0-1.2
    ratio_score = 1.0 if 0.8 <= aspect_ratio <= 1.3 else 0.5
    
    # 基于面积的置信度
    if 1000 <= area <= 50000:
        area_score = 1.0
    elif 500 <= area <= 100000:
        area_score = 0.8
    else:
        area_score = 0.3
    
    return (ratio_score + area_score) / 2.0

def non_maximum_suppression(faces: List[Tuple[int, int, int, int]], overlap_threshold: float) -> List[Tuple[int, int, int, int]]:
    """
    非极大值抑制，去除重叠的人脸
    
    Args:
        faces: 人脸列表
        overlap_threshold: 重叠阈值
    
    Returns:
        抑制后的人脸列表
    """
    if not faces:
        return faces
    
    # 按面积排序，保留最大的人脸
    faces_with_area = [(face, face[2] * face[3]) for face in faces]
    faces_with_area.sort(key=lambda x: x[1], reverse=True)
    
    result = []
    while faces_with_area:
        current_face, _ = faces_with_area.pop(0)
        result.append(current_face)
        
        # 移除与当前人脸重叠度高的其他人脸
        faces_with_area = [
            (face, area) for face, area in faces_with_area
            if calculate_overlap(current_face, face) < overlap_threshold
        ]
    
    return result

def calculate_overlap(rect1: Tuple[int, int, int, int], rect2: Tuple[int, int, int, int]) -> float:
    """
    计算两个矩形的重叠度
    
    Args:
        rect1, rect2: 矩形坐标 (x, y, w, h)
    
    Returns:
        重叠度 (0.0-1.0)
    """
    x1, y1, w1, h1 = rect1
    x2, y2, w2, h2 = rect2
    
    # 计算交集
    x_left = max(x1, x2)
    y_top = max(y1, y2)
    x_right = min(x1 + w1, x2 + w2)
    y_bottom = min(y1 + h1, y2 + h2)
    
    if x_right <= x_left or y_bottom <= y_top:
        return 0.0
    
    intersection_area = (x_right - x_left) * (y_bottom - y_top)
    union_area = w1 * h1 + w2 * h2 - intersection_area
    
    return intersection_area / union_area if union_area > 0 else 0.0

# 高性能版本的人脸检测
def detect_faces_high_performance(image_data: bytes, width: int, height: int) -> List[Tuple[int, int, int, int]]:
    """
    高性能人脸检测版本（优化版）
    
    Args:
        image_data: BGRA格式的图像字节数据
        width: 图像宽度
        height: 图像高度
    
    Returns:
        检测到的人脸矩形列表
    """
    start_time = time.time()
    
    try:
        # 将字节数据转换为numpy数组
        image_array = np.frombuffer(image_data, dtype=np.uint8)
        image_array = image_array.reshape(height, width, 4)
        
        # 转换为灰度图像
        gray_image = cv2.cvtColor(image_array, cv2.COLOR_BGRA2GRAY)
        
        # 使用缓存的分类器实例
        face_cascade = get_face_cascade()
        
        # 动态参数：根据分辨率调整
        min_size = max(15, min(width, height) // 25)
        max_size = min(250, min(width, height) // 4)
        
        # 高性能参数
        faces = face_cascade.detectMultiScale(
            gray_image,
            scaleFactor=1.2,  # 更大的缩放因子，更快
            minNeighbors=2,   # 更少的邻居要求
            minSize=(min_size, min_size),
            maxSize=(max_size, max_size)
        )
        
        # 简单过滤
        result = []
        for (x, y, w, h) in faces:
            x = max(0, min(x, width - w))
            y = max(0, min(y, height - h))
            w = min(w, width - x)
            h = min(h, height - y)
            result.append((x, y, w, h))
        
        elapsed = (time.time() - start_time) * 1000
        print(f"[detect_faces_high_performance] Processed {width}x{height} in {elapsed:.1f}ms, found {len(result)} faces")
        
        return result
        
    except Exception as e:
        print(f"High performance face detection error: {e}")
        return []

def detect_faces_batch(images_data: List[Tuple[bytes, int, int]]) -> List[List[Tuple[int, int, int, int]]]:
    """
    批量人脸检测，提高处理效率
    
    Args:
        images_data: 图像数据列表，每个元素为 (image_bytes, width, height)
    
    Returns:
        检测结果列表，每个元素为对应图像的人脸矩形列表
    """
    start_time = time.time()
    
    try:
        # 确保分类器已加载
        face_cascade = get_face_cascade()
        
        results = []
        for i, (image_data, width, height) in enumerate(images_data):
            try:
                # 转换为灰度图像
                image_array = np.frombuffer(image_data, dtype=np.uint8)
                image_array = image_array.reshape(height, width, 4)
                gray_image = cv2.cvtColor(image_array, cv2.COLOR_BGRA2GRAY)
                
                # 动态参数
                min_size = max(15, min(width, height) // 25)
                max_size = min(250, min(width, height) // 4)
                
                # 检测
                faces = face_cascade.detectMultiScale(
                    gray_image,
                    scaleFactor=1.15,
                    minNeighbors=2,
                    minSize=(min_size, min_size),
                    maxSize=(max_size, max_size)
                )
                
                # 处理结果
                result = []
                for (x, y, w, h) in faces:
                    x = max(0, min(x, width - w))
                    y = max(0, min(y, height - h))
                    w = min(w, width - x)
                    h = min(h, height - y)
                    result.append((x, y, w, h))
                
                results.append(result)
                
            except Exception as e:
                print(f"Batch detection error for image {i}: {e}")
                results.append([])
        
        elapsed = (time.time() - start_time) * 1000
        print(f"[detect_faces_batch] Processed {len(images_data)} images in {elapsed:.1f}ms")
        
        return results
        
    except Exception as e:
        print(f"Batch detection error: {e}")
        return [[] for _ in images_data]

# 统一配置驱动的人脸检测
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
        if use_gray:
            # 直接转灰度，避免多余转换
            arr = np.frombuffer(image_data, dtype=np.uint8)
            img = arr.reshape(height, width, 4)
            gray = cv2.cvtColor(img, cv2.COLOR_BGRA2GRAY)
            working = gray
        else:
            arr = np.frombuffer(image_data, dtype=np.uint8)
            img = arr.reshape(height, width, 4)
            bgr = cv2.cvtColor(img, cv2.COLOR_BGRA2BGR)
            working = bgr

        # 可选缩放
        scale = float(image_scale) if image_scale and image_scale > 0 else 1.0
        if scale != 1.0:
            new_w = max(1, int(width * scale))
            new_h = max(1, int(height * scale))
            working = cv2.resize(working, (new_w, new_h), interpolation=cv2.INTER_LINEAR)
        else:
            new_w, new_h = width, height

        face_cascade = get_face_cascade()

        # 选择输入（灰度或从 BGR 转灰）
        if use_gray:
            gray_input = working
        else:
            gray_input = cv2.cvtColor(working, cv2.COLOR_BGR2GRAY)

        # 均衡化提升鲁棒性
        gray_input = cv2.equalizeHist(gray_input)

        # 调整阈值与参数
        min_size = (int(max(1, min_face_size)), int(max(1, min_face_size)))
        max_size = (int(max(1, max_face_size)), int(max(1, max_face_size)))

        faces = face_cascade.detectMultiScale(
            gray_input,
            scaleFactor=float(scale_factor),
            minNeighbors=int(min_neighbors),
            minSize=min_size,
            maxSize=max_size,
        )

        # 置信度过滤 + NMS
        thr = float(confidence_threshold) if confidence_threshold is not None else 0.0
        filtered = post_process_faces(faces, confidence_threshold=thr)

        # 若进行了缩放，将坐标还原到原图尺度
        result: List[Tuple[int, int, int, int]] = []
        inv_scale = 1.0 / scale if scale != 1.0 else 1.0
        for (x, y, w, h) in filtered:
            ox = int(x * inv_scale)
            oy = int(y * inv_scale)
            ow = int(w * inv_scale)
            oh = int(h * inv_scale)
            # 边界裁剪
            ox = max(0, min(ox, width - 1))
            oy = max(0, min(oy, height - 1))
            ow = max(1, min(ow, width - ox))
            oh = max(1, min(oh, height - oy))
            result.append((ox, oy, ow, oh))

        return result
    except Exception as e:
        print(f"detect_faces_with_config error: {e}")
        return []

def get_detection_stats() -> dict:
    """
    获取检测器统计信息
    
    Returns:
        包含分类器状态和性能信息的字典
    """
    stats = {
        "cascade_loaded": _FACE_CASCADE is not None,
        "cascade_alt_loaded": _FACE_CASCADE_ALT is not None,
        "cascade_path": cv2.data.haarcascades + 'haarcascade_frontalface_alt2.xml',
        "cascade_alt_path": cv2.data.haarcascades + 'haarcascade_frontalface_default.xml'
    }
    
    if _FACE_CASCADE is not None:
        stats["cascade_empty"] = _FACE_CASCADE.empty()
    
    return stats 