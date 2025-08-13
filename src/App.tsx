import { useState, useEffect } from "react";
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

interface Image {
  width: number;
  height: number;
  data: number[]; // BGRA format
}

function App() {
  const [monitors, setMonitors] = useState<MonitorInfo[]>([]);
  const [selectedMonitor, setSelectedMonitor] = useState<MonitorInfo | null>(null);
  const [currentImage, setCurrentImage] = useState<Image | null>(null);

  // Load monitors on component mount
  useEffect(() => {
    loadMonitors();
  }, []);

  // Listen for image events
  useEffect(() => {
    if (selectedMonitor) {
      console.log("Setting up image event listener for monitor:", selectedMonitor.id);
      
      const unlisten = listen<Image>("image", (event) => {
        console.log("Received image:", event.payload);
        setCurrentImage(event.payload);
      });

      return () => {
        console.log("Cleaning up image event listener");
        unlisten.then(fn => fn()).catch(error => {
          console.error("Failed to cleanup event listener:", error);
        });
      };
    }
  }, [selectedMonitor]);

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
      setCurrentImage(null);
      console.log("Stopped monitoring");
    } catch (error) {
      console.error("Failed to stop monitoring:", error);
    }
  };

  const convertImageDataToCanvas = (image: Image): string => {
    try {
      console.log("Converting image data:", image.width, "x", image.height, "data length:", image.data.length);
      
      const canvas = document.createElement('canvas');
      canvas.width = image.width;
      canvas.height = image.height;
      const ctx = canvas.getContext('2d');
      
      if (!ctx) {
        console.error("Failed to get canvas context");
        return '';
      }

      const imageData = ctx.createImageData(image.width, image.height);
      const data = imageData.data;

      // 验证数据长度
      const expectedLength = image.width * image.height * 4;
      if (image.data.length !== expectedLength) {
        console.error("Image data length mismatch:", image.data.length, "expected:", expectedLength);
        return '';
      }

      // Convert BGRA to RGBA
      for (let i = 0; i < image.data.length; i += 4) {
        data[i] = image.data[i + 2];     // R (from B)
        data[i + 1] = image.data[i + 1]; // G
        data[i + 2] = image.data[i];     // B (from R)
        data[i + 3] = image.data[i + 3]; // A
      }

      ctx.putImageData(imageData, 0, 0);
      const dataUrl = canvas.toDataURL();
      console.log("Image conversion completed, data URL length:", dataUrl.length);
      return dataUrl;
    } catch (error) {
      console.error("Error converting image data:", error);
      return '';
    }
  };

  return (
    <div className="app">
      <PythonInstallationProgress />
      <header className="app-header">
        <h1>Screen Ghost - Monitor Demo</h1>
      </header>

      <main className="app-main">
        <section className="monitor-section">
          <h2>Available Monitors</h2>
          <div className="monitor-grid">
            {monitors.map((monitor) => (
              <div
                key={monitor.id}
                className={`monitor-card ${selectedMonitor?.id === monitor.id ? 'selected' : ''}`}
                onClick={() => selectMonitor(monitor)}
              >
                <h3>Monitor {monitor.id}</h3>
                <div className="monitor-info">
                  <p><strong>Position:</strong> ({monitor.x}, {monitor.y})</p>
                  <p><strong>Size:</strong> {monitor.width} × {monitor.height}</p>
                  <p><strong>Scale:</strong> {monitor.scale_factor}</p>
                </div>
                {selectedMonitor?.id === monitor.id && (
                  <div className="selected-indicator">✓ Selected</div>
                )}
              </div>
            ))}
          </div>
        </section>

        <section className="image-section">
          <h2>Live Image Display</h2>
          {selectedMonitor ? (
            <div className="image-container">
              <div className="monitor-controls">
                <h3>Monitoring Monitor {selectedMonitor.id}</h3>
                <button 
                  onClick={stopMonitoring}
                  className="stop-button"
                >
                  Stop Monitoring
                </button>
              </div>
              {currentImage ? (
                <div className="image-info">
                  <p>Receiving live image from Monitor {selectedMonitor.id}</p>
                  <p>Image size: {currentImage.width} × {currentImage.height}</p>
                  <img
                    src={convertImageDataToCanvas(currentImage)}
                    alt="Live monitor capture"
                    className="live-image"
                  />
                </div>
              ) : (
                <div className="waiting-message">
                  <p>Waiting for image data from Monitor {selectedMonitor.id}...</p>
                  <div className="loading-spinner"></div>
                </div>
              )}
            </div>
          ) : (
            <div className="no-monitor-selected">
              <p>Please select a monitor to start receiving live images</p>
            </div>
          )}
        </section>
      </main>
    </div>
  );
}

export default App;
