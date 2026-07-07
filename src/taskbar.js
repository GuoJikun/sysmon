// 使用 Tauri 全局 API（withGlobalTauri: true）
const { listen } = window.__TAURI__.event;

const unlisten = await listen('net-speed', (event) => {
  const data = event.payload;

  document.getElementById('net-down').textContent = formatSpeedShort(data.down);
  document.getElementById('net-up').textContent = formatSpeedShort(data.up);
});

function formatSpeedShort(bytesPerSec) {
  if (bytesPerSec < 1024) {
    return `${bytesPerSec.toFixed(0)}B`;
  } else if (bytesPerSec < 1048576) {
    return `${(bytesPerSec / 1024).toFixed(0)}K`;
  } else {
    return `${(bytesPerSec / 1048576).toFixed(1)}M`;
  }
}

// 同步主题（与主窗口保持一致）
const savedTheme = localStorage.getItem('theme') || 'light';
document.body.setAttribute('data-theme', savedTheme);

// 监听设置窗口的主题变更事件，实时同步
await listen('theme-changed', (event) => {
  document.body.setAttribute('data-theme', event.payload);
});
