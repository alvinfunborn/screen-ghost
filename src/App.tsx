import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";
import PythonInstallationProgress from "./components/PythonInstallationProgress";

interface MonitorInfo {
  id: number;
  x: number;
  y: number;
  width: number;
  height: number;
  scale_factor: number;
}

interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

interface ImagePayload {
  width: number;
  height: number;
  data: number[]; // BGRA
}

// 后端直接 emit Vec<Rect>，前端按数组解析
// type FrameInfo = Record<string, Rect[]>;

function App() {
  const [monitors, setMonitors] = useState<MonitorInfo[]>([]);
  const [selectedMonitor, setSelectedMonitor] = useState<MonitorInfo | null>(null);
  const [faceRects, setFaceRects] = useState<Rect[]>([]);
  const viewportRef = useRef<HTMLDivElement | null>(null);
  const [viewportSize, setViewportSize] = useState<{ width: number; height: number }>({ width: 800, height: 400 });
  const imageCanvasRef = useRef<HTMLCanvasElement | null>(null);

  // Load monitors on component mount
  useEffect(() => {
    loadMonitors();
  }, []);

  // Listen for frame_info (face rectangles) events
  useEffect(() => {
    if (!selectedMonitor) return;
    console.log("Setting up frame_info listener for monitor:", selectedMonitor.id);
    const unlisten = listen<Rect[]>("frame_info", (event) => {
      const payload = event.payload as unknown;
      const rects = Array.isArray(payload) ? (payload as Rect[]) : [];
      console.log("frame_info received:", rects);
      setFaceRects(rects);
    });
    return () => {
      unlisten.then(fn => fn()).catch(err => console.error("Failed to cleanup frame_info listener", err));
    };
  }, [selectedMonitor]);

  // Listen for image events and draw into canvas
  useEffect(() => {
    if (!selectedMonitor) return;
    console.log("Setting up image listener for monitor:", selectedMonitor.id);
    const draw = (img: ImagePayload) => {
      const canvas = imageCanvasRef.current;
      if (!canvas) return;
      const ctx = canvas.getContext('2d');
      if (!ctx) return;
      const { width, height, data } = img;
      if (canvas.width !== width || canvas.height !== height) {
        canvas.width = width;
        canvas.height = height;
      }
      const src = data as number[];
      const rgba = new Uint8ClampedArray(width * height * 4);
      // Convert BGRA -> RGBA
      for (let i = 0, j = 0; i < src.length; i += 4, j += 4) {
        const b = src[i];
        const g = src[i + 1];
        const r = src[i + 2];
        const a = src[i + 3];
        rgba[j] = r;
        rgba[j + 1] = g;
        rgba[j + 2] = b;
        rgba[j + 3] = a;
      }
      const imageData = new ImageData(rgba, width, height);
      ctx.putImageData(imageData, 0, 0);
    };

    const unlistenPromise = listen<ImagePayload>("image", (event) => {
      const payload = event.payload as unknown as ImagePayload;
      draw(payload);
    });
    return () => {
      unlistenPromise.then(fn => fn()).catch(err => console.error("Failed to cleanup image listener", err));
    };
  }, [selectedMonitor]);

  // Measure viewport size for scaling
  useEffect(() => {
    const updateSize = () => {
      if (viewportRef.current) {
        const rect = viewportRef.current.getBoundingClientRect();
        setViewportSize({ width: rect.width, height: rect.height });
      }
    };
    updateSize();
    window.addEventListener('resize', updateSize);
    return () => window.removeEventListener('resize', updateSize);
  }, [monitors.length]);

  const loadMonitors = async () => {
    try {
      const monitorList = await invoke<MonitorInfo[]>("get_monitors");
      setMonitors(monitorList);
      console.log("Loaded monitors:", monitorList);
    } catch (error) {
      console.error("Failed to load monitors:", error);
    }
  };

  const selectMonitor = async (monitor: MonitorInfo) => {
    try {
      await invoke("set_working_monitor", { monitor });
      setSelectedMonitor(monitor);
      console.log("Selected monitor:", monitor);
    } catch (error) {
      console.error("Failed to select monitor:", error);
    }
  };

  const stopMonitoring = async () => {
    try {
      await invoke("stop_monitoring");
      setSelectedMonitor(null);
      console.log("Stopped monitoring");
    } catch (error) {
      console.error("Failed to stop monitoring:", error);
    }
  };

  const handleMonitorClick = async (monitor: MonitorInfo) => {
    try {
      if (selectedMonitor && selectedMonitor.id === monitor.id) {
        await stopMonitoring();
        return;
      }
      if (selectedMonitor && selectedMonitor.id !== monitor.id) {
        await stopMonitoring();
      }
      await selectMonitor(monitor);
    } catch (e) {
      console.error('handleMonitorClick failed', e);
    }
  };

  // Compute virtual desktop bounds and scaling
  const bounds = (() => {
    if (monitors.length === 0) {
      return { minX: 0, minY: 0, width: 1, height: 1 };
    }
    const minX = Math.min(...monitors.map(m => m.x));
    const minY = Math.min(...monitors.map(m => m.y));
    const maxX = Math.max(...monitors.map(m => m.x + m.width));
    const maxY = Math.max(...monitors.map(m => m.y + m.height));
    return { minX, minY, width: Math.max(1, maxX - minX), height: Math.max(1, maxY - minY) };
  })();

  const scale = Math.min(
    viewportSize.width / bounds.width,
    viewportSize.height / bounds.height
  );
  const scaledTotalWidth = bounds.width * scale;
  const scaledTotalHeight = bounds.height * scale;
  const offsetX = (viewportSize.width - scaledTotalWidth) / 2;
  const offsetY = (viewportSize.height - scaledTotalHeight) / 2;

  return (
    <div className="app">
      <PythonInstallationProgress />

      <main className="app-main">
        <section className="monitor-section">
          <div className="display-viewport" ref={viewportRef}>
          <div className="display-canvas" style={{ width: `${viewportSize.width}px`, height: `${viewportSize.height}px` }}>
            {monitors.map(m => {
              const left = offsetX + (m.x - bounds.minX) * scale;
              const top = offsetY + (m.y - bounds.minY) * scale;
              const width = m.width * scale;
              const height = m.height * scale;
              const isSelected = selectedMonitor?.id === m.id;
              return (
                <div
                  key={m.id}
                  className={`display-monitor ${isSelected ? 'selected' : ''}`}
                  style={{ left, top, width, height }}
                  onClick={() => handleMonitorClick(m)}
                  title={`位置(${m.x}, ${m.y}) 尺寸 ${m.width}×${m.height} 缩放 ${m.scale_factor}`}
                >
                  <div className="display-label">{m.id + 1}</div>
                </div>
              );
            })}

            {/* Live screenshot canvas for selected monitor */}
            {selectedMonitor && (
              (() => {
                const left = offsetX + (selectedMonitor.x - bounds.minX) * scale;
                const top = offsetY + (selectedMonitor.y - bounds.minY) * scale;
                const width = selectedMonitor.width * scale;
                const height = selectedMonitor.height * scale;
                return (
                  <canvas
                    ref={imageCanvasRef}
                    style={{
                      position: 'absolute',
                      left,
                      top,
                      width,
                      height,
                      pointerEvents: 'none',
                    }}
                  />
                );
              })()
            )}

            {/* Face rectangles overlay for selected monitor */}
            {selectedMonitor && faceRects.map((r, idx) => {
              const left = offsetX + (selectedMonitor.x - bounds.minX + r.x) * scale;
              const top = offsetY + (selectedMonitor.y - bounds.minY + r.y) * scale;
              const width = r.width * scale;
              const height = r.height * scale;
              return (
                <div
                  key={`face-${idx}`}
                  style={{
                    position: 'absolute',
                    left,
                    top,
                    width,
                    height,
                    border: '2px solid #ff5252',
                    boxSizing: 'border-box',
                    pointerEvents: 'none',
                  }}
                  title={`face ${idx+1}: (${r.x}, ${r.y}, ${r.width}x${r.height})`}
                />
              );
            })}
          </div>
        </div>
        </section>
      </main>
    </div>
  );
}

export default App;
