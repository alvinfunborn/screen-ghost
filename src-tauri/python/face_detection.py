import cv2
import numpy as np
from typing import List, Tuple

def detect_faces(image_data: bytes, width: int, height: int) -> List[Tuple[int, int, int, int]]:
    """
    人脸检测函数
    
    Args:
        image_data: BGRA格式的图像字节数据
        width: 图像宽度
        height: 图像高度
    
    Returns:
        List[Tuple[int, int, int, int]]: 检测到的人脸矩形 (x, y, width, height)
    """
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
        
        # 加载人脸检测器
        face_cascade = cv2.CascadeClassifier(cv2.data.haarcascades + 'haarcascade_frontalface_alt2.xml')
        
        # 如果默认分类器不存在，使用备用分类器
        if face_cascade.empty():
            face_cascade = cv2.CascadeClassifier(cv2.data.haarcascades + 'haarcascade_frontalface_default.xml')
        
        # 人脸检测参数
        scale_factor = 1.1
        min_neighbors = 3
        min_size = (30, 30)
        max_size = (300, 300)
        
        # 执行人脸检测
        faces = face_cascade.detectMultiScale(
            gray_image,
            scaleFactor=scale_factor,
            minNeighbors=min_neighbors,
            minSize=min_size,
            maxSize=max_size
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
        
        return result
        
    except Exception as e:
        print(f"Face detection error: {e}")
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
    高性能人脸检测版本
    
    Args:
        image_data: BGRA格式的图像字节数据
        width: 图像宽度
        height: 图像高度
    
    Returns:
        检测到的人脸矩形列表
    """
    try:
        # 将字节数据转换为numpy数组
        image_array = np.frombuffer(image_data, dtype=np.uint8)
        image_array = image_array.reshape(height, width, 4)
        
        # 转换为灰度图像
        gray_image = cv2.cvtColor(image_array, cv2.COLOR_BGRA2GRAY)
        
        # 加载人脸检测器
        face_cascade = cv2.CascadeClassifier(cv2.data.haarcascades + 'haarcascade_frontalface_alt2.xml')
        if face_cascade.empty():
            face_cascade = cv2.CascadeClassifier(cv2.data.haarcascades + 'haarcascade_frontalface_default.xml')
        
        # 高性能参数
        faces = face_cascade.detectMultiScale(
            gray_image,
            scaleFactor=1.2,  # 更大的缩放因子，更快
            minNeighbors=2,   # 更少的邻居要求
            minSize=(20, 20), # 更小的最小尺寸
            maxSize=(200, 200) # 更小的最大尺寸
        )
        
        # 简单过滤
        result = []
        for (x, y, w, h) in faces:
            x = max(0, min(x, width - w))
            y = max(0, min(y, height - h))
            w = min(w, width - x)
            h = min(h, height - y)
            result.append((x, y, w, h))
        
        return result
        
    except Exception as e:
        print(f"High performance face detection error: {e}")
        return [] 