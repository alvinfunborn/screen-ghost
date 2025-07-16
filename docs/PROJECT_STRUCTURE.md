# Screen Ghost - 项目目录结构

## 项目概览

```
screen-ghost-python/
├── 📁 核心代码
├── 📁 测试代码
├── 📁 配置文件
├── 📁 文档
├── 📁 资源文件
└── 📁 部署文件
```

## 详细目录结构

```
screen-ghost-python/
│
├── 📄 README.md                    # 项目主要文档
├── 📄 TECHNICAL_DESIGN.md          # 技术设计文档
├── 📄 DEVELOPMENT_PLAN.md          # 开发计划文档
├── 📄 API_DOCUMENTATION.md         # API文档
├── 📄 PROJECT_STRUCTURE.md         # 项目结构文档 (本文件)
├── 📄 pyproject.toml              # 项目配置文件
├── 📄 main.py                     # 主程序入口
│
├── 📁 screen_ghost/               # 核心代码包
│   ├── 📄 __init__.py             # 包初始化文件
│   ├── 📄 detector.py             # 人脸检测模块
│   ├── 📄 processor.py            # 图片处理模块
│   ├── 📄 service.py              # 检测服务模块
│   ├── 📄 config.py               # 配置管理模块
│   ├── 📄 cache.py                # 缓存管理模块
│   ├── 📄 exceptions.py           # 异常定义
│   └── 📄 utils.py                # 工具函数
│
├── 📁 tests/                      # 测试代码
│   ├── 📄 __init__.py
│   ├── 📄 test_detector.py        # 检测器测试
│   ├── 📄 test_processor.py       # 处理器测试
│   ├── 📄 test_service.py         # 服务测试
│   ├── 📄 test_api.py             # API测试
│   ├── 📄 test_performance.py     # 性能测试
│   └── 📄 conftest.py             # 测试配置
│
├── 📁 config/                     # 配置文件
│   ├── 📄 default.yaml            # 默认配置
│   ├── 📄 development.yaml        # 开发环境配置
│   ├── 📄 production.yaml         # 生产环境配置
│   └── 📄 test.yaml              # 测试环境配置
│
├── 📁 docs/                       # 文档目录
│   ├── 📄 installation.md         # 安装指南
│   ├── 📄 user_guide.md          # 用户指南
│   ├── 📄 developer_guide.md      # 开发者指南
│   ├── 📄 deployment.md           # 部署指南
│   └── 📄 troubleshooting.md      # 故障排除
│
├── 📁 resources/                  # 资源文件
│   ├── 📁 images/                 # 图片资源
│   │   ├── 📄 logo.png           # 项目logo
│   │   ├── 📄 icon.ico           # 应用图标
│   │   └── 📁 test_images/       # 测试图片
│   └── 📁 models/                 # 模型文件
│       └── 📄 face_detection/     # 人脸检测模型
│
├── 📁 scripts/                    # 脚本文件
│   ├── 📄 install_dependencies.sh # 依赖安装脚本
│   ├── 📄 run_tests.sh           # 测试运行脚本
│   ├── 📄 build_package.sh       # 打包脚本
│   └── 📄 deploy.sh              # 部署脚本
│
├── 📁 examples/                   # 示例代码
│   ├── 📄 basic_usage.py         # 基础使用示例
│   ├── 📄 api_client.py          # API客户端示例
│   ├── 📄 batch_processing.py    # 批量处理示例
│   └── 📄 performance_test.py    # 性能测试示例
│
├── 📁 deployment/                 # 部署相关
│   ├── 📄 Dockerfile             # Docker配置
│   ├── 📄 docker-compose.yml     # Docker Compose配置
│   ├── 📄 requirements.txt       # 依赖列表
│   └── 📄 setup.py              # 安装脚本
│
├── 📁 .github/                    # GitHub配置
│   ├── 📄 workflows/             # GitHub Actions
│   │   ├── 📄 ci.yml            # 持续集成
│   │   ├── 📄 test.yml          # 测试工作流
│   │   └── 📄 deploy.yml        # 部署工作流
│   └── 📄 ISSUE_TEMPLATE.md     # Issue模板
│
├── 📄 .gitignore                 # Git忽略文件
├── 📄 .python-version            # Python版本
├── 📄 .editorconfig              # 编辑器配置
└── 📄 LICENSE                    # 许可证文件
```

## 核心模块说明

### 1. screen_ghost/ - 核心代码包

#### detector.py - 人脸检测器
```python
# 主要类和方法
class FaceDetector:
    def __init__(self, model_selection=0, min_detection_confidence=0.5)
    def detect_faces(self, image) -> List[Dict]
    def _extract_face_boxes(self, results) -> List[Dict]
```

#### processor.py - 图片处理器
```python
# 主要类和方法
class ImageProcessor:
    def __init__(self)
    def load_image(self, image_data) -> np.ndarray
    def validate_image(self, image) -> bool
    def preprocess_image(self, image) -> np.ndarray
```

#### service.py - 检测服务
```python
# 主要类和方法
class DetectionService:
    def __init__(self, face_detector, image_processor)
    async def detect_single_image(self, image_data) -> Dict
    async def detect_batch_images(self, images_data) -> Dict
```

#### config.py - 配置管理器
```python
# 主要类和方法
class ConfigManager:
    def __init__(self, config_path='config/default.yaml')
    def load_config(self, config_path) -> Dict
    def get_detection_config(self) -> Dict
```

#### cache.py - 缓存管理器
```python
# 主要类和方法
class DetectionCache:
    def __init__(self, max_size=1000)
    def get(self, image_data) -> Optional[Dict]
    def set(self, image_data, result)
```

#### exceptions.py - 异常定义
```python
# 异常类
class ScreenGhostError(Exception)
class ImageProcessingError(ScreenGhostError)
class DetectionError(ScreenGhostError)
class ValidationError(ScreenGhostError)
```

#### utils.py - 工具函数
```python
# 工具函数
def convert_coordinates(face_box, image_shape) -> Tuple[int, int, int, int]
def calculate_processing_time(start_time) -> int
def validate_image_path(path) -> bool
def create_output_directory(path) -> bool
```

### 2. tests/ - 测试代码

#### test_detector.py
```python
# 测试用例
class TestFaceDetector:
    def test_detect_faces_empty_image(self)
    def test_detect_faces_with_face(self)
    def test_detect_faces_multiple_faces(self)
    def test_detect_faces_invalid_input(self)
```

#### test_processor.py
```python
# 测试用例
class TestImageProcessor:
    def test_load_image_valid(self)
    def test_load_image_invalid(self)
    def test_validate_image_size(self)
    def test_preprocess_image(self)
```

#### test_service.py
```python
# 测试用例
class TestDetectionService:
    def test_detect_single_image(self)
    def test_detect_batch_images(self)
    def test_error_handling(self)
    def test_performance_metrics(self)
```

#### test_api.py
```python
# API测试
class TestAPI:
    def test_detect_endpoint(self)
    def test_batch_detect_endpoint(self)
    def test_health_endpoint(self)
    def test_error_responses(self)
```

#### test_performance.py
```python
# 性能测试
class TestPerformance:
    def test_processing_speed(self)
    def test_memory_usage(self)
    def test_concurrent_requests(self)
    def test_cache_performance(self)
```

### 3. config/ - 配置文件

#### default.yaml
```yaml
# 默认配置
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

#### development.yaml
```yaml
# 开发环境配置
detection:
  model_selection: 0
  min_detection_confidence: 0.3  # 降低阈值便于测试

performance:
  max_memory_mb: 300  # 降低内存限制
  max_concurrent_requests: 5

logging:
  level: "DEBUG"
  file: "logs/development.log"
```

### 4. docs/ - 文档目录

#### installation.md
```markdown
# 安装指南

## 系统要求
- Python 3.8+
- OpenCV 4.8+
- MediaPipe 0.10+
- FastAPI 0.100+

## 安装步骤
1. 克隆项目
2. 安装依赖
3. 配置环境
4. 运行测试
```

#### user_guide.md
```markdown
# 用户指南

## 快速开始
1. 启动服务
2. 调用API接口
3. 处理返回结果

## API使用
- 单张图片检测
- 批量图片检测
- 错误处理
```

### 5. examples/ - 示例代码

#### basic_usage.py
```python
# 基础使用示例
import requests

def detect_faces(image_path):
    """检测单张图片中的人脸"""
    url = "http://localhost:8000/api/detect"
    
    with open(image_path, 'rb') as f:
        files = {'image': f}
        response = requests.post(url, files=files)
    
    return response.json()

# 使用示例
result = detect_faces("test.jpg")
print(result)
```

#### api_client.py
```python
# API客户端示例
import requests
import json

class ScreenGhostClient:
    def __init__(self, base_url="http://localhost:8000"):
        self.base_url = base_url
    
    def detect_single(self, image_path):
        """检测单张图片"""
        url = f"{self.base_url}/api/detect"
        with open(image_path, 'rb') as f:
            files = {'image': f}
            response = requests.post(url, files=files)
        return response.json()
    
    def detect_batch(self, image_paths):
        """批量检测图片"""
        url = f"{self.base_url}/api/detect/batch"
        files = []
        for i, path in enumerate(image_paths):
            files.append(('images', open(path, 'rb')))
        
        response = requests.post(url, files=files)
        return response.json()
```

#### batch_processing.py
```python
# 批量处理示例
import os
from api_client import ScreenGhostClient

def process_directory(directory_path):
    """处理目录中的所有图片"""
    client = ScreenGhostClient()
    
    # 获取所有图片文件
    image_extensions = {'.jpg', '.jpeg', '.png', '.bmp', '.tiff'}
    image_files = []
    
    for filename in os.listdir(directory_path):
        if any(filename.lower().endswith(ext) for ext in image_extensions):
            image_files.append(os.path.join(directory_path, filename))
    
    # 批量处理
    results = client.detect_batch(image_files)
    
    # 输出结果
    for result in results['results']:
        print(f"图片 {result['image_id']}: 检测到 {result['face_count']} 个人脸")
    
    return results
```

#### performance_test.py
```python
# 性能测试示例
import time
import requests
from concurrent.futures import ThreadPoolExecutor

def test_single_request(image_path):
    """测试单个请求"""
    start_time = time.time()
    
    url = "http://localhost:8000/api/detect"
    with open(image_path, 'rb') as f:
        files = {'image': f}
        response = requests.post(url, files=files)
    
    processing_time = time.time() - start_time
    return {
        'status_code': response.status_code,
        'processing_time': processing_time,
        'response_time': response.elapsed.total_seconds()
    }

def test_concurrent_requests(image_path, num_requests=10):
    """测试并发请求"""
    with ThreadPoolExecutor(max_workers=num_requests) as executor:
        futures = [executor.submit(test_single_request, image_path) 
                  for _ in range(num_requests)]
        
        results = [future.result() for future in futures]
    
    return results

# 使用示例
results = test_concurrent_requests("test.jpg", 10)
for i, result in enumerate(results):
    print(f"请求 {i+1}: {result}")
```

### 6. deployment/ - 部署文件

#### Dockerfile
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

#### requirements.txt
```txt
opencv-python>=4.8.0
mediapipe>=0.10.0
numpy>=1.24.0
fastapi>=0.100.0
uvicorn>=0.22.0
python-multipart>=0.0.6
pydantic>=2.0.0
pyyaml>=6.0
psutil>=5.9.0
```

#### docker-compose.yml
```yaml
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

## 文件命名规范

### Python文件
- 使用小写字母和下划线
- 例如: `face_detector.py`, `image_processor.py`

### 配置文件
- 使用小写字母和点号
- 例如: `config.yaml`, `default.yaml`

### 测试文件
- 以 `test_` 开头
- 例如: `test_detector.py`, `test_processor.py`

### 文档文件
- 使用小写字母和下划线
- 例如: `user_guide.md`, `installation.md`

## 导入规范

### 相对导入
```python
# 在screen_ghost包内
from .detector import FaceDetector
from .processor import ImageProcessor
```

### 绝对导入
```python
# 从外部导入
from screen_ghost.detector import FaceDetector
from screen_ghost.processor import ImageProcessor
```

## 代码组织原则

### 1. 单一职责
- 每个模块只负责一个特定功能
- 类和方法职责明确

### 2. 依赖注入
- 通过构造函数注入依赖
- 降低模块间耦合

### 3. 配置分离
- 配置与代码分离
- 支持多环境配置

### 4. 错误处理
- 统一的异常处理机制
- 详细的错误信息

### 5. 测试覆盖
- 每个模块都有对应测试
- 支持单元测试和集成测试

### 6. API设计
- RESTful API设计
- 标准化的请求响应格式
- 完善的错误处理

### 7. 性能优化
- 异步处理架构
- 缓存机制
- 内存管理

这个重新设计的项目结构专注于图片人脸检测服务，提供了清晰的API服务架构和模块划分！ 🏗️ 