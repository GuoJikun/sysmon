// 使用 Tauri 全局 API（withGlobalTauri: true）
const { listen } = window.__TAURI__.event;

const unlisten = await listen('net-speed', (event) => {
  const data = event.payload;

  // Rust 端已格式化
  document.getElementById('net-down').textContent = data.down_str;
  document.getElementById('net-up').textContent = data.up_str;
});

// 同步主题（与主窗口保持一致）
const savedTheme = localStorage.getItem('theme') || 'light';
document.body.setAttribute('data-theme', savedTheme);

// 监听设置窗口的主题变更事件，实时同步
await listen('theme-changed', (event) => {
  document.body.setAttribute('data-theme', event.payload);
});
