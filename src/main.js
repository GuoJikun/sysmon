// 使用 Tauri 全局 API（withGlobalTauri: true）
const { listen } = window.__TAURI__.event;
const { invoke } = window.__TAURI__.core;
const { appWindow } = window.__TAURI__.window;

// 监听系统信息事件
const unlisten = await listen('sys-info', (event) => {
  const data = event.payload;

  // CPU
  document.getElementById('cpu-value').textContent = `${data.cpu.toFixed(1)}%`;

  // 内存
  document.getElementById('mem-value').textContent = `${data.mem_pct.toFixed(1)}%`;

  // 网速（Rust 端已格式化）
  document.getElementById('net-down-value').textContent = data.net_down_str;
  document.getElementById('net-up-value').textContent = data.net_up_str;
});

// 主题同步：监听设置窗口的主题变更事件
await listen('theme-changed', (event) => {
  const theme = event.payload;
  document.documentElement.setAttribute('data-theme', theme);
  document.body.setAttribute('data-theme', theme);
});

// 启动时恢复主题
const savedTheme = localStorage.getItem('theme') || 'light';
document.documentElement.setAttribute('data-theme', savedTheme);
document.body.setAttribute('data-theme', savedTheme);



// 右键弹出托盘弹窗
document.addEventListener('contextmenu', async (e) => {
  e.preventDefault();
  const screenX = window.screenX + e.clientX;
  const screenY = window.screenY + e.clientY;
  console.log('[main] contextmenu at', screenX, screenY);
  try {
    await invoke('show_popup_at_cursor', { x: screenX, y: screenY });
    console.log('[main] show_popup_at_cursor OK');
  } catch (err) {
    console.error('[main] show_popup_at_cursor failed:', err);
  }
});
