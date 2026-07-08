// 使用 Tauri 全局 API（withGlobalTauri: true）
const { listen } = window.__TAURI__.event;
const { invoke } = window.__TAURI__.core;

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

// ── 右键上下文菜单 ──
const ctxMenu = document.getElementById('context-menu');
const ctxTaskbarItem = ctxMenu.querySelector('[data-action="taskbar"]');

// 更新任务栏对号状态
async function refreshTaskbarCheck() {
  const visible = await invoke('get_taskbar_visible');
  ctxTaskbarItem.classList.toggle('checked', visible);
}

// 右键显示上下文菜单
document.addEventListener('contextmenu', async (e) => {
  e.preventDefault();
  await refreshTaskbarCheck();
  ctxMenu.style.display = 'block';
  // 确保菜单不超出屏幕
  const mw = ctxMenu.offsetWidth;
  const mh = ctxMenu.offsetHeight;
  let x = e.clientX;
  let y = e.clientY;
  if (x + mw > window.innerWidth) x = window.innerWidth - mw - 4;
  if (y + mh > window.innerHeight) y = window.innerHeight - mh - 4;
  ctxMenu.style.left = x + 'px';
  ctxMenu.style.top = y + 'px';
});

// 点击菜单项
ctxMenu.querySelectorAll('.ctx-item').forEach((item) => {
  item.addEventListener('click', () => {
    const action = item.dataset.action;
    ctxMenu.style.display = 'none';
    if (action === 'taskbar') {
      invoke('toggle_taskbar_visible');
    } else if (action === 'settings') {
      invoke('tray_popup_action', { action: 'settings' });
    }
  });
});

// 点击其他地方关闭菜单
document.addEventListener('click', (e) => {
  if (!ctxMenu.contains(e.target)) {
    ctxMenu.style.display = 'none';
  }
});

// 监听设置变更（从托盘弹窗或设置窗口同步任务栏状态）
listen('settings-changed', (event) => {
  if (event.payload && event.payload.taskbar_visible !== undefined) {
    ctxTaskbarItem.classList.toggle('checked', event.payload.taskbar_visible);
  }
});
