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
        setTimeout(() => {
          setStatus(prev => ({ ...prev, isVisible: false }));
        }, 3000);
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
          setStatus({ isVisible: true, message: '安装完成', type: 'success' });
          setTimeout(() => {
            setStatus(prev => ({ ...prev, isVisible: false }));
          }, 3000);
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

  const icon = status.type === 'success' ? '✓' : status.type === 'error' ? '✗' : status.type === 'progress' ? '⟳' : 'ℹ';

  return (
    <div
      style={{
        position: 'fixed',
        inset: 0 as unknown as number,
        background: 'rgba(0,0,0,0.5)',
        zIndex: 10000,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        pointerEvents: 'auto'
      }}
    >
      <div
        style={{
          background: '#111',
          color: '#fff',
          padding: '16px 20px',
          borderRadius: 12,
          width: 'min(92vw, 520px)',
          boxShadow: '0 10px 24px rgba(0,0,0,0.4)'
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 8 }}>
          <div style={{ fontSize: 20, fontWeight: 700 }}>{icon}</div>
          <div style={{ fontSize: 16, fontWeight: 600 }}>
            {status.type === 'progress' ? '正在安装依赖…' : status.type === 'success' ? '安装完成' : status.type === 'error' ? '安装失败' : '处理中'}
          </div>
        </div>
        <div style={{ fontSize: 14, lineHeight: 1.5, whiteSpace: 'pre-wrap' }}>{status.message}</div>
        {status.type === 'progress' && (
          <div style={{ marginTop: 12, display: 'flex', alignItems: 'center', gap: 12 }}>
            <div className="loading-spinner" />
            <div>正在安装…</div>
          </div>
        )}
        {(status.type === 'info' || status.type === 'error' || status.type === 'success') && (
          <div style={{ marginTop: 12, opacity: 0.8, fontSize: 12 }}>请稍候…</div>
        )}
      </div>
    </div>
  );
};

export default PythonInstallationProgress; 