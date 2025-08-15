import React, { useState, useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';

interface InstallationStatus {
  isVisible: boolean;
  message: string;
  type: 'info' | 'success' | 'error' | 'progress';
}

const PythonInstallationProgress: React.FC = () => {
  const [status, setStatus] = useState<InstallationStatus>({
    isVisible: false,
    message: '',
    type: 'info'
  });

  useEffect(() => {
    const unlistenPromises: Promise<() => void>[] = [];

    // 监听安装开始事件
    unlistenPromises.push(
      listen('python-installation-started', (event) => {
        setStatus({
          isVisible: true,
          message: event.payload as string,
          type: 'info'
        });
      })
    );

    // 监听安装进度事件
    unlistenPromises.push(
      listen('python-installation-progress', (event) => {
        setStatus({
          isVisible: true,
          message: event.payload as string,
          type: 'progress'
        });
      })
    );

    // 监听安装成功事件
    unlistenPromises.push(
      listen('python-installation-success', (event) => {
        setStatus({
          isVisible: true,
          message: event.payload as string,
          type: 'success'
        });
      })
    );

    // 监听安装错误事件
    unlistenPromises.push(
      listen('python-installation-error', (event) => {
        setStatus({
          isVisible: true,
          message: event.payload as string,
          type: 'error'
        });
      })
    );

    // 监听安装完成事件
    unlistenPromises.push(
      listen('python-installation-completed', (event) => {
        setStatus({
          isVisible: true,
          message: event.payload as string,
          type: 'success'
        });
        
        // 3秒后隐藏进度条
        setTimeout(() => {
          setStatus(prev => ({ ...prev, isVisible: false }));
        }, 3000);
      })
    );

    // 监听通用 toast 事件
    unlistenPromises.push(
      listen<string>('toast', (event) => {
        const msg = event.payload;
        if (msg === 'close') {
          setStatus(prev => ({ ...prev, isVisible: false }));
          return;
        }
        setStatus({
          isVisible: true,
          message: msg,
          type: 'progress'
        });
      })
    );

    return () => {
      unlistenPromises.forEach(unlisten => unlisten.then(fn => fn()));
    };
  }, []);

  if (!status.isVisible) {
    return null;
  }

  const getStatusColor = () => {
    switch (status.type) {
      case 'success':
        return 'bg-green-500';
      case 'error':
        return 'bg-red-500';
      case 'progress':
        return 'bg-blue-500';
      default:
        return 'bg-gray-500';
    }
  };

  const getStatusIcon = () => {
    switch (status.type) {
      case 'success':
        return '✓';
      case 'error':
        return '✗';
      case 'progress':
        return '⟳';
      default:
        return 'ℹ';
    }
  };

  return (
    <div className="fixed top-4 right-4 z-50">
      <div className={`${getStatusColor()} text-white px-4 py-3 rounded-lg shadow-lg max-w-md`}>
        <div className="flex items-center space-x-3">
          <div className="text-lg font-bold">{getStatusIcon()}</div>
          <div className="flex-1">
            <div className="text-sm font-medium">{status.message}</div>
            {status.type === 'progress' && (
              <div className="mt-2">
                <div className="w-full bg-white bg-opacity-20 rounded-full h-2">
                  <div className="bg-white h-2 rounded-full animate-pulse" style={{ width: '60%' }}></div>
                </div>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};

export default PythonInstallationProgress; 