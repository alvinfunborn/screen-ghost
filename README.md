## Screen Ghost

> Real-time face detection and mosaic overlay desktop eye protection tool
> Built with Tauri + React + Rust, integrated with Python/OpenCV/InsightFace, supporting multi-monitor and high-frame low-latency rendering

---

### Language / 语言

- [English](README.md) | [中文](README_zh.md)

---

### Use Cases

- Automatically mask "target persons" when watching any video or image
- Covers all scenarios including local players, browsers, image viewers, etc.

![app](./docs/app.png)

![demo](./docs/demo.gif)

---

### Core Features

- **High Frame Rate & Low Latency**:
- **Two Working Modes**:
  - No target library: Detect "all faces" and apply masks
  - With target library: Execute InsightFace detection+recognition on the entire image with the same `image_scale`, returning only bounding boxes for matched "target persons"
- **Automated Python Environment**:
  - First launch automatically creates venv and silently installs dependencies (numpy/opencv/onnxruntime/insightface)
- **Security**:
  - Open source code, no network connections, no backdoors, no poison

---

### Screenshots/Examples

- Overlay renders mosaics in a separate window without modifying the original desktop
- Customizable `mosaic_style` (CSS string) for different mask styles

![demo](./docs/demo.gif)

---

### Installation & Usage

#### Method 1: Direct Download & Run (Recommended)

1. Go to [Releases page](https://github.com/alvinfunborn/screen-ghost/releases) to download the release package, extract to any directory
2. Ensure the directory structure is as follows:
   ```
   your-directory/
   ├── screen-ghost.exe
   ├── config.toml
   ├── python/
       ├── faces.py
   └── faces/
       ├── Zhang San/
       │   ├── photo1.jpg
       │   └── photo2.jpg
       └── Li Si/
           ├── photo1.jpg
           └── photo2.jpg
   ```
3. Create subfolders under `faces/` directory (e.g., person names) and place target face photos
4. Double-click `screen-ghost.exe` to run

First launch will automatically:
- Detect system Python; create venv and install dependencies if unavailable
- Subsequent launches will first verify if venv dependencies are complete, skipping installation if complete

> Environment: Windows 10/11 x64; GPU available will prioritize CUDA, then DirectML, otherwise CPU.
>
> **Optional GPU Acceleration**: If you want CUDA acceleration for better performance, you need to install NVIDIA CUDA 12.x and NVIDIA cuDNN 9. The current onnxruntime-gpu version (>=1.16) is compatible with CUDA 12.x + cuDNN 9 on Windows.
>
> ⚠️ **Note**: CUDA 13.x is NOT compatible with the current onnxruntime-gpu version.

#### Method 2: Source Code Compilation

```bash
# Clone repository
git clone https://github.com/alvinfunborn/screen-ghost.git
cd screen-ghost

# Install frontend dependencies
npm ci

# Build Tauri backend
cd src-tauri
cargo build

# Development mode launch
cd ..
npm run tauri dev
```

---

### CUDA/cuDNN Setup (Windows)

**Optional steps** (only if you want CUDA acceleration):

1. **Install NVIDIA CUDA 12.x** (NOT CUDA 13.x) - this provides the required runtime libraries
2. Update to the latest NVIDIA GameReady/Studio driver.
3. Install NVIDIA cuDNN 9 (Windows x64). After installation, make sure `cudnn64_9.dll` is discoverable by the app, either by:
   - Adding cuDNN's `bin` directory to your system `PATH`, or
   - Copying the DLL into the venv ONNX Runtime directory:
     - `%APPDATA%\screen-ghost\python_env\Lib\site-packages\onnxruntime\capi\cudnn64_9.dll`
3. Restart the app. The log should show `Applied providers: ['CUDAExecutionProvider', 'CPUExecutionProvider']`.

---

### Environment Initialization & On-disk Locations

- Installation / write locations (Windows)
  - Python virtual environment: `%APPDATA%/screen-ghost/python_env/`
  - Extracted Python scripts: `%APPDATA%/screen-ghost/python_files/`
  - App config (example): `config.toml` (next to the exe)
  - Target face library: `faces/` (next to the exe)
- Startup behavior
  - Prefers copying scripts from the exe-side `python/` to `python_files/`
  - Python dependencies (numpy/opencv/onnxruntime/insightface) are installed into an isolated venv; with `provider=auto`, the best ONNX Runtime variant is selected in order CUDA→DML→CPU.

---

### Configuration (`src-tauri/config.toml`)

```toml
[face.detection]
min_face_ratio = 0.05      # Minimum face detection ratio (short side percentage), falls back to *_face_size if not provided
max_face_ratio = 0.9
scale_factor = 1.2         # Haar upsampling step
min_neighbors = 3
confidence_threshold = 0.4 # Discard if below this confidence
use_gray = true
image_scale = 0.7          # Image scaling before detection

[face.recognition]
# auto | cpu | cuda | dml
provider = "auto"          # Recognition model runtime environment, auto will select and install corresponding ORT variants by CUDA→DML→CPU priority
threshold = 0.55           # Recognition hit threshold (cosine similarity)
outlier_threshold = 0.3    # Outlier removal threshold (building mean features for each person)
outlier_iter = 2

[monitoring]
interval = 8               # Main loop interval (ms)
mosaic_scale = 2.0         # Mosaic rectangle geometric magnification (independent of DPI)
capture_scale = 1.0        # Downsampling ratio after screenshot, before detection (speed up)
mosaic_style = """
{
    position: absolute;
    background-color: rgba(0,0,0,0.4);   /* Example: semi-transparent black mask */
    image-rendering: pixelated;
    border-radius: 4px;
}
"""

[system]
log_level = "debug"
```

---

### Target Face Photo Library (`faces/<name>/xxx.jpg`)

- **Quantity**: Recommend 5-10 photos per person (≥3 usable, >20 usually diminishing returns)
- **Quality**: Side length ≥ 160-200px, clear, unobstructed; diverse lighting but not over/under exposed
- **Diversity**: Slight pose/expression/lighting variations; some glasses acceptable; avoid high repetition
- **Organization**: One folder per person, don't mix others' photos; correct orientation if abnormal

Application will preload this directory on startup, calculate "mean features" for each person, and remove outliers by threshold/iteration.

---

### Common Issues & Solutions

#### CUDA Version Compatibility Issues

**Problem**: Many users encounter errors when trying to use CUDA 13.x with this project.

**Root Cause**: The current onnxruntime-gpu version (>=1.16) is specifically built for NVIDIA CUDA 12.x and NVIDIA cuDNN 9. CUDA 13.x introduces breaking changes that are not compatible.

**Solution**: 
1. Install NVIDIA CUDA 12.x (not 13.x) from NVIDIA's website
2. Install NVIDIA cuDNN 9 for Windows x64
3. Ensure your NVIDIA driver is up to date

**Verification**: After installation, the app should automatically detect CUDA 12.x runtime libraries and show `CUDAExecutionProvider` in the available providers.

#### Missing cuDNN 9 Error

**Problem**: App shows warning about missing CUDA/cuDNN and falls back to DirectML.

**Solution**: 
1. Download NVIDIA cuDNN 9 for Windows x64 from NVIDIA Developer website
2. Extract and add the `bin` directory to your system PATH, OR
3. Copy `cudnn64_9.dll` to the app's ONNX Runtime directory

**Note**: The app will automatically switch back to CUDA once cuDNN 9 is properly installed.
