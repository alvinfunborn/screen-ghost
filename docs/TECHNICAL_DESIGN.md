# Screen Ghost - 技术设计文档

## 系统架构设计

### 整体架构
```
┌─────────────────────────────────────────────────────────────┐
│                        HTTP API层                           │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │
│  │  单张检测   │  │  批量检测   │  │  健康检查   │        │
│  └─────────────┘  └─────────────┘  └─────────────┘        │
└─────────────────────────────────────────────────────────────┘
                                │
┌─────────────────────────────────────────────────────────────┐
│                      业务逻辑层                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │
│  │ 图片处理器  │  │ 检测管理器  │  │ 结果格式化  │        │
│  └─────────────┘  └─────────────┘  └─────────────┘        │
└─────────────────────────────────────────────────────────────┘
                                │
┌─────────────────────────────────────────────────────────────┐
│                      核心算法层                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │
│  │ 人脸检测器  │  │ 坐标转换器  │  │ 置信度计算  │        │
│  └─────────────┘  └─────────────┘  └─────────────┘        │
└─────────────────────────────────────────────────────────────┘
                                │
┌─────────────────────────────────────────────────────────────┐
│                      数据访问层                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │
│  │ 图片读取    │  │ 数据验证    │  │ 结果输出    │        │
│  └─────────────┘  └─────────────┘  └─────────────┘        │
└─────────────────────────────────────────────────────────────┘
```

## 核心模块设计

### 1. 人脸检测模块 (FaceDetector)

```python
class FaceDetector:
    """人脸检测器，基于MediaPipe实现图片人脸检测"""
    
    def __init__(self, model_selection=0, min_detection_confidence=0.5):
        self.mp_face_detection = mp.solutions.face_detection
        self.face_detection = self.mp_face_detection.FaceDetection(
            model_selection=model_selection,
            min_detection_confidence=min_detection_confidence
        )
    
    def detect_faces(self, image: np.ndarray) -> List[Dict[str, Any]]:
        """检测图像中的人脸"""
        results = self.face_detection.process(image)
        return self._extract_face_boxes(results)
    
    def _extract_face_boxes(self, results) -> List[Dict[str, Any]]:
        """提取人脸边界框"""
        face_boxes = []
        if results.detections:
            for detection in results.detections:
                bbox = detection.location_data.relative_bounding_box
                face_boxes.append({
                    'x': int(bbox.xmin * image.shape[1]),
                    'y': int(bbox.ymin * image.shape[0]),
                    'width': int(bbox.width * image.shape[1]),
                    'height': int(bbox.height * image.shape[0]),
                    'confidence': float(detection.score[0])
                })
        return face_boxes
```

### 2. 图片处理模块 (ImageProcessor)

```python
class ImageProcessor:
    """图片处理器，处理图片读取和预处理"""
    
    def __init__(self):
        self.supported_formats = {'.jpg', '.jpeg', '.png', '.bmp', '.tiff'}
    
    def load_image(self, image_data: bytes) -> np.ndarray:
        """从字节数据加载图片"""
        nparr = np.frombuffer(image_data, np.uint8)
        image = cv2.imdecode(nparr, cv2.IMREAD_COLOR)
        if image is None:
            raise ValueError("无法解析图片数据")
        return image
    
    def validate_image(self, image: np.ndarray) -> bool:
        """验证图片格式和大小"""
        if image is None or image.size == 0:
            return False
        
        # 检查图片尺寸
        height, width = image.shape[:2]
        if width > 4096 or height > 4096:
            return False
        
        return True
    
    def preprocess_image(self, image: np.ndarray) -> np.ndarray:
        """图片预处理"""
        # 转换为RGB格式
        rgb_image = cv2.cvtColor(image, cv2.COLOR_BGR2RGB)
        return rgb_image
```

### 3. API服务模块 (DetectionService)

```python
class DetectionService:
    """检测服务，处理API请求"""
    
    def __init__(self, face_detector: FaceDetector, image_processor: ImageProcessor):
        self.face_detector = face_detector
        self.image_processor = image_processor
    
    async def detect_single_image(self, image_data: bytes) -> Dict[str, Any]:
        """检测单张图片"""
        start_time = time.time()
        
        try:
            # 加载和验证图片
            image = self.image_processor.load_image(image_data)
            if not self.image_processor.validate_image(image):
                raise ValueError("图片格式无效或尺寸过大")
            
            # 预处理图片
            rgb_image = self.image_processor.preprocess_image(image)
            
            # 检测人脸
            faces = self.face_detector.detect_faces(rgb_image)
            
            processing_time = int((time.time() - start_time) * 1000)
            
            return {
                "success": True,
                "faces": faces,
                "processing_time": processing_time,
                "face_count": len(faces)
            }
            
        except Exception as e:
            return {
                "success": False,
                "error": str(e),
                "processing_time": int((time.time() - start_time) * 1000)
            }
    
    async def detect_batch_images(self, images_data: List[Tuple[str, bytes]]) -> Dict[str, Any]:
        """批量检测图片"""
        results = []
        
        for image_id, image_data in images_data:
            result = await self.detect_single_image(image_data)
            result["image_id"] = image_id
            results.append(result)
        
        return {
            "success": True,
            "results": results,
            "total_images": len(results)
        }
```

### 4. FastAPI应用模块 (main.py)

```python
from fastapi import FastAPI, File, UploadFile, HTTPException
from fastapi.responses import JSONResponse
import uvicorn

app = FastAPI(
    title="Screen Ghost API",
    description="图片人脸检测与马赛克位置服务",
    version="1.0.0"
)

# 初始化服务组件
detector = FaceDetector()
processor = ImageProcessor()
service = DetectionService(detector, processor)

@app.post("/api/detect")
async def detect_faces(image: UploadFile = File(...)):
    """检测单张图片中的人脸"""
    try:
        image_data = await image.read()
        result = await service.detect_single_image(image_data)
        
        if result["success"]:
            return JSONResponse(content=result)
        else:
            raise HTTPException(status_code=400, detail=result["error"])
            
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))

@app.post("/api/detect/batch")
async def detect_batch_faces(images: List[UploadFile] = File(...)):
    """批量检测图片中的人脸"""
    try:
        images_data = []
        for i, image in enumerate(images):
            image_data = await image.read()
            images_data.append((f"img_{i+1:03d}", image_data))
        
        result = await service.detect_batch_images(images_data)
        return JSONResponse(content=result)
        
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))

@app.get("/health")
async def health_check():
    """健康检查接口"""
    return {"status": "healthy", "service": "screen-ghost"}

if __name__ == "__main__":
    uvicorn.run(app, host="0.0.0.0", port=8000)
```

## 数据模型设计

### 1. 请求模型 (Pydantic)

```python
from pydantic import BaseModel
from typing import List, Optional

class FaceDetectionRequest(BaseModel):
    """人脸检测请求模型"""
    image_id: Optional[str] = None
    confidence_threshold: Optional[float] = 0.5

class FaceBox(BaseModel):
    """人脸边界框模型"""
    x: int
    y: int
    width: int
    height: int
    confidence: float

class DetectionResponse(BaseModel):
    """检测响应模型"""
    success: bool
    faces: List[FaceBox]
    processing_time: int
    face_count: int
    error: Optional[str] = None

class BatchDetectionResponse(BaseModel):
    """批量检测响应模型"""
    success: bool
    results: List[DetectionResponse]
    total_images: int
```

### 2. 错误处理模型

```python
class APIError(BaseModel):
    """API错误模型"""
    error: str
    detail: str
    status_code: int

class ValidationError(BaseModel):
    """验证错误模型"""
    field: str
    message: str
```

## 性能优化策略

### 1. 异步处理
```python
import asyncio
from concurrent.futures import ThreadPoolExecutor

class AsyncDetectionService:
    """异步检测服务"""
    
    def __init__(self):
        self.executor = ThreadPoolExecutor(max_workers=4)
    
    async def detect_faces_async(self, image_data: bytes) -> Dict[str, Any]:
        """异步检测人脸"""
        loop = asyncio.get_event_loop()
        result = await loop.run_in_executor(
            self.executor, 
            self._detect_faces_sync, 
            image_data
        )
        return result
    
    def _detect_faces_sync(self, image_data: bytes) -> Dict[str, Any]:
        """同步检测人脸（在线程池中执行）"""
        # 检测逻辑
        pass
```

### 2. 内存管理
```python
import gc

class MemoryManager:
    """内存管理器"""
    
    def __init__(self, max_memory_mb: int = 500):
        self.max_memory_mb = max_memory_mb
    
    def check_memory_usage(self) -> bool:
        """检查内存使用情况"""
        import psutil
        process = psutil.Process()
        memory_mb = process.memory_info().rss / 1024 / 1024
        
        if memory_mb > self.max_memory_mb:
            self._cleanup_memory()
            return False
        return True
    
    def _cleanup_memory(self):
        """清理内存"""
        gc.collect()
```

### 3. 缓存机制
```python
import hashlib
from typing import Dict, Any

class DetectionCache:
    """检测结果缓存"""
    
    def __init__(self, max_size: int = 1000):
        self.cache: Dict[str, Any] = {}
        self.max_size = max_size
    
    def get_cache_key(self, image_data: bytes) -> str:
        """生成缓存键"""
        return hashlib.md5(image_data).hexdigest()
    
    def get(self, image_data: bytes) -> Optional[Dict[str, Any]]:
        """获取缓存结果"""
        key = self.get_cache_key(image_data)
        return self.cache.get(key)
    
    def set(self, image_data: bytes, result: Dict[str, Any]):
        """设置缓存结果"""
        key = self.get_cache_key(image_data)
        
        if len(self.cache) >= self.max_size:
            # 移除最旧的缓存
            oldest_key = next(iter(self.cache))
            del self.cache[oldest_key]
        
        self.cache[key] = result
```

## 错误处理

### 1. 异常类型定义
```python
class ScreenGhostError(Exception):
    """Screen Ghost基础异常"""
    pass

class ImageProcessingError(ScreenGhostError):
    """图片处理错误"""
    pass

class DetectionError(ScreenGhostError):
    """检测错误"""
    pass

class ValidationError(ScreenGhostError):
    """验证错误"""
    pass
```

### 2. 错误处理中间件
```python
from fastapi import Request
from fastapi.responses import JSONResponse

@app.exception_handler(ScreenGhostError)
async def screen_ghost_exception_handler(request: Request, exc: ScreenGhostError):
    """统一异常处理"""
    return JSONResponse(
        status_code=400,
        content={
            "success": False,
            "error": str(exc),
            "error_type": exc.__class__.__name__
        }
    )

@app.exception_handler(Exception)
async def general_exception_handler(request: Request, exc: Exception):
    """通用异常处理"""
    return JSONResponse(
        status_code=500,
        content={
            "success": False,
            "error": "内部服务器错误",
            "error_type": "InternalServerError"
        }
    )
```

## 配置管理

### 1. 配置文件结构
```yaml
# config/default.yaml
detection:
  model_selection: 0
  min_detection_confidence: 0.5
  max_image_size: 4096

performance:
  max_memory_mb: 500
  max_concurrent_requests: 10
  cache_enabled: true
  cache_size: 1000

api:
  host: "0.0.0.0"
  port: 8000
  workers: 4
  timeout: 30

logging:
  level: "INFO"
  format: "%(asctime)s - %(name)s - %(levelname)s - %(message)s"
```

### 2. 配置管理器
```python
import yaml
from typing import Dict, Any

class ConfigManager:
    """配置管理器"""
    
    def __init__(self, config_path: str = "config/default.yaml"):
        self.config_path = config_path
        self.config = self._load_config()
    
    def _load_config(self) -> Dict[str, Any]:
        """加载配置文件"""
        with open(self.config_path, 'r', encoding='utf-8') as f:
            return yaml.safe_load(f)
    
    def get_detection_config(self) -> Dict[str, Any]:
        """获取检测配置"""
        return self.config.get('detection', {})
    
    def get_performance_config(self) -> Dict[str, Any]:
        """获取性能配置"""
        return self.config.get('performance', {})
    
    def get_api_config(self) -> Dict[str, Any]:
        """获取API配置"""
        return self.config.get('api', {})
```

## 测试策略

### 1. 单元测试
```python
import pytest
import numpy as np
from screen_ghost.detector import FaceDetector

class TestFaceDetector:
    """人脸检测器测试"""
    
    def test_detect_faces_empty_image(self):
        """测试空图像检测"""
        detector = FaceDetector()
        empty_image = np.zeros((100, 100, 3), dtype=np.uint8)
        faces = detector.detect_faces(empty_image)
        assert len(faces) == 0
    
    def test_detect_faces_with_face(self):
        """测试包含人脸的图像"""
        detector = FaceDetector()
        # 创建测试图像
        test_image = self._create_test_image_with_face()
        faces = detector.detect_faces(test_image)
        assert len(faces) > 0
```

### 2. API测试
```python
import httpx
import pytest

class TestAPI:
    """API测试"""
    
    @pytest.mark.asyncio
    async def test_detect_single_image(self):
        """测试单张图片检测API"""
        async with httpx.AsyncClient() as client:
            with open("test_image.jpg", "rb") as f:
                files = {"image": f}
                response = await client.post(
                    "http://localhost:8000/api/detect",
                    files=files
                )
            
            assert response.status_code == 200
            data = response.json()
            assert data["success"] == True
            assert "faces" in data
```

## 部署方案

### 1. Docker部署
```dockerfile
FROM python:3.8-slim

# 安装系统依赖
RUN apt-get update && apt-get install -y \
    libgl1-mesa-glx \
    libglib2.0-0 \
    libsm6 \
    libxext6 \
    libxrender-dev \
    && rm -rf /var/lib/apt/lists/*

# 复制项目文件
COPY . /app
WORKDIR /app

# 安装Python依赖
RUN pip install -r requirements.txt

# 暴露端口
EXPOSE 8000

# 启动服务
CMD ["uvicorn", "main:app", "--host", "0.0.0.0", "--port", "8000"]
```

### 2. 生产环境配置
```yaml
# docker-compose.yml
version: '3.8'

services:
  screen-ghost:
    build: .
    ports:
      - "8000:8000"
    environment:
      - ENVIRONMENT=production
    volumes:
      - ./config:/app/config
    restart: unless-stopped
```

这个重新设计的技术文档专注于图片人脸检测服务，提供了完整的API服务架构和实现方案！ 🚀 