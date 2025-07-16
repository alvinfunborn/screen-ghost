# Screen Ghost - API文档

## 概述

本文档描述了Screen Ghost项目的API接口，这是一个专门用于图片人脸检测的服务。项目接收图片输入，返回需要打马赛克的坐标信息，为其他应用提供人脸检测和位置分析功能。

## API基础信息

### 服务地址
```
Base URL: http://localhost:8000
API Version: v1
Content-Type: application/json
```

### 认证方式
当前版本无需认证，后续版本可能添加API密钥认证。

## 核心API接口

### 1. 单张图片检测

#### 接口信息
```http
POST /api/detect
Content-Type: multipart/form-data
```

#### 请求参数
| 参数名 | 类型 | 必填 | 描述 |
|--------|------|------|------|
| image | file | 是 | 图片文件 (支持 jpg, jpeg, png, bmp, tiff) |

#### 请求示例
```bash
curl -X POST "http://localhost:8000/api/detect" \
     -H "Content-Type: multipart/form-data" \
     -F "image=@test.jpg"
```

#### 响应格式
```json
{
  "success": true,
  "faces": [
    {
      "x": 100,
      "y": 150,
      "width": 80,
      "height": 100,
      "confidence": 0.95
    }
  ],
  "processing_time": 85,
  "face_count": 1
}
```

#### 响应字段说明
| 字段名 | 类型 | 描述 |
|--------|------|------|
| success | boolean | 请求是否成功 |
| faces | array | 检测到的人脸列表 |
| faces[].x | integer | 人脸边界框左上角x坐标 |
| faces[].y | integer | 人脸边界框左上角y坐标 |
| faces[].width | integer | 人脸边界框宽度 |
| faces[].height | integer | 人脸边界框高度 |
| faces[].confidence | float | 检测置信度 (0.0-1.0) |
| processing_time | integer | 处理时间 (毫秒) |
| face_count | integer | 检测到的人脸数量 |

#### 错误响应
```json
{
  "success": false,
  "error": "图片格式无效或尺寸过大",
  "processing_time": 15
}
```

---

### 2. 批量图片检测

#### 接口信息
```http
POST /api/detect/batch
Content-Type: multipart/form-data
```

#### 请求参数
| 参数名 | 类型 | 必填 | 描述 |
|--------|------|------|------|
| images | file[] | 是 | 图片文件数组 (最多10张) |

#### 请求示例
```bash
curl -X POST "http://localhost:8000/api/detect/batch" \
     -H "Content-Type: multipart/form-data" \
     -F "images=@image1.jpg" \
     -F "images=@image2.jpg"
```

#### 响应格式
```json
{
  "success": true,
  "results": [
    {
      "image_id": "img_001",
      "faces": [
        {
          "x": 100,
          "y": 150,
          "width": 80,
          "height": 100,
          "confidence": 0.95
        }
      ],
      "processing_time": 85,
      "face_count": 1
    },
    {
      "image_id": "img_002",
      "faces": [],
      "processing_time": 45,
      "face_count": 0
    }
  ],
  "total_images": 2
}
```

#### 响应字段说明
| 字段名 | 类型 | 描述 |
|--------|------|------|
| success | boolean | 请求是否成功 |
| results | array | 每张图片的检测结果 |
| results[].image_id | string | 图片ID |
| results[].faces | array | 检测到的人脸列表 |
| results[].processing_time | integer | 处理时间 (毫秒) |
| results[].face_count | integer | 检测到的人脸数量 |
| total_images | integer | 处理的图片总数 |

---

### 3. 健康检查

#### 接口信息
```http
GET /health
```

#### 请求示例
```bash
curl -X GET "http://localhost:8000/health"
```

#### 响应格式
```json
{
  "status": "healthy",
  "service": "screen-ghost",
  "version": "1.0.0",
  "timestamp": "2024-01-15T10:30:00Z"
}
```

---

### 4. API文档

#### 接口信息
```http
GET /docs
```

访问 `http://localhost:8000/docs` 查看交互式API文档。

---

## 数据模型

### 1. 人脸边界框 (FaceBox)
```python
class FaceBox(BaseModel):
    x: int                    # 左上角x坐标
    y: int                    # 左上角y坐标
    width: int                # 边界框宽度
    height: int               # 边界框高度
    confidence: float         # 检测置信度 (0.0-1.0)
```

### 2. 检测响应 (DetectionResponse)
```python
class DetectionResponse(BaseModel):
    success: bool             # 请求是否成功
    faces: List[FaceBox]     # 检测到的人脸列表
    processing_time: int      # 处理时间 (毫秒)
    face_count: int          # 检测到的人脸数量
    error: Optional[str]     # 错误信息 (可选)
```

### 3. 批量检测响应 (BatchDetectionResponse)
```python
class BatchDetectionResponse(BaseModel):
    success: bool                    # 请求是否成功
    results: List[DetectionResponse] # 每张图片的检测结果
    total_images: int               # 处理的图片总数
```

## 错误处理

### 1. HTTP状态码
| 状态码 | 描述 |
|--------|------|
| 200 | 请求成功 |
| 400 | 请求参数错误 |
| 413 | 请求体过大 |
| 415 | 不支持的媒体类型 |
| 500 | 服务器内部错误 |

### 2. 错误响应格式
```json
{
  "success": false,
  "error": "错误描述",
  "error_type": "ErrorType",
  "processing_time": 15
}
```

### 3. 常见错误类型
| 错误类型 | 描述 | 解决方案 |
|----------|------|----------|
| ImageFormatError | 图片格式不支持 | 使用支持的格式 (jpg, png, bmp, tiff) |
| ImageSizeError | 图片尺寸过大 | 图片尺寸不能超过4096x4096 |
| DetectionError | 检测过程出错 | 检查图片质量和内容 |
| ValidationError | 参数验证失败 | 检查请求参数格式 |

## 使用示例

### 1. Python客户端示例
```python
import requests
import json

def detect_faces(image_path):
    """检测单张图片中的人脸"""
    url = "http://localhost:8000/api/detect"
    
    with open(image_path, 'rb') as f:
        files = {'image': f}
        response = requests.post(url, files=files)
    
    if response.status_code == 200:
        result = response.json()
        if result['success']:
            print(f"检测到 {result['face_count']} 个人脸")
            for face in result['faces']:
                print(f"位置: ({face['x']}, {face['y']}) "
                      f"大小: {face['width']}x{face['height']} "
                      f"置信度: {face['confidence']:.2f}")
        else:
            print(f"检测失败: {result['error']}")
    else:
        print(f"请求失败: {response.status_code}")

# 使用示例
detect_faces("test.jpg")
```

### 2. JavaScript客户端示例
```javascript
async function detectFaces(imageFile) {
    const url = 'http://localhost:8000/api/detect';
    const formData = new FormData();
    formData.append('image', imageFile);
    
    try {
        const response = await fetch(url, {
            method: 'POST',
            body: formData
        });
        
        const result = await response.json();
        
        if (result.success) {
            console.log(`检测到 ${result.face_count} 个人脸`);
            result.faces.forEach(face => {
                console.log(`位置: (${face.x}, ${face.y}) ` +
                          `大小: ${face.width}x${face.height} ` +
                          `置信度: ${face.confidence.toFixed(2)}`);
            });
        } else {
            console.error(`检测失败: ${result.error}`);
        }
    } catch (error) {
        console.error('请求失败:', error);
    }
}

// 使用示例
const fileInput = document.getElementById('imageInput');
fileInput.addEventListener('change', (e) => {
    const file = e.target.files[0];
    if (file) {
        detectFaces(file);
    }
});
```

### 3. cURL命令行示例
```bash
# 检测单张图片
curl -X POST "http://localhost:8000/api/detect" \
     -H "Content-Type: multipart/form-data" \
     -F "image=@test.jpg" \
     | jq '.'

# 批量检测图片
curl -X POST "http://localhost:8000/api/detect/batch" \
     -H "Content-Type: multipart/form-data" \
     -F "images=@image1.jpg" \
     -F "images=@image2.jpg" \
     | jq '.'

# 健康检查
curl -X GET "http://localhost:8000/health" | jq '.'
```

## 性能指标

### 1. 处理速度
- **单张图片**: < 100ms
- **批量处理**: 10张图片 < 1秒
- **并发处理**: 支持10个并发请求

### 2. 准确率
- **人脸检测准确率**: > 90%
- **误检率**: < 5%
- **漏检率**: < 10%

### 3. 资源使用
- **内存使用**: < 500MB
- **CPU使用**: < 50% (4核CPU)
- **磁盘空间**: < 100MB

## 限制说明

### 1. 图片格式限制
- **支持格式**: JPG, JPEG, PNG, BMP, TIFF
- **最大尺寸**: 4096 x 4096 像素
- **最大文件大小**: 10MB

### 2. 请求限制
- **批量处理**: 最多10张图片
- **并发请求**: 最多10个并发
- **请求频率**: 无限制

### 3. 检测限制
- **人脸大小**: 最小20x20像素
- **人脸角度**: 支持±45度旋转
- **光照条件**: 支持正常光照条件

## 部署信息

### 1. 服务启动
```bash
# 开发环境
uvicorn main:app --reload --host 0.0.0.0 --port 8000

# 生产环境
uvicorn main:app --host 0.0.0.0 --port 8000 --workers 4
```

### 2. Docker部署
```bash
# 构建镜像
docker build -t screen-ghost .

# 运行容器
docker run -p 8000:8000 screen-ghost
```

### 3. 环境变量
| 变量名 | 默认值 | 描述 |
|--------|--------|------|
| HOST | 0.0.0.0 | 服务监听地址 |
| PORT | 8000 | 服务端口 |
| WORKERS | 4 | 工作进程数 |
| LOG_LEVEL | INFO | 日志级别 |

这个API文档提供了完整的接口说明和使用示例，帮助开发者快速集成Screen Ghost服务！ 📚 