const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

// 启动时恢复主题
const savedTheme = localStorage.getItem('theme') || 'light';
document.documentElement.setAttribute('data-theme', savedTheme);
document.body.setAttribute('data-theme', savedTheme);

// 监听主题变更（从设置窗口同步）
listen('theme-changed', (event) => {
  const theme = event.payload;
  document.documentElement.setAttribute('data-theme', theme);
  document.body.setAttribute('data-theme', theme);
  localStorage.setItem('theme', theme);
});

// 对号状态元素
const showItem = document.querySelector('[data-action="show"]');
const taskbarItem = document.querySelector('[data-action="taskbar"]');

// 启动时查询一次初始状态
(async () => {
  const [mainVisible, taskbarVisible] = await Promise.all([
    invoke('is_main_visible'),
    invoke('get_taskbar_visible'),
  ]);
  showItem.classList.toggle('checked', mainVisible);
  taskbarItem.classList.toggle('checked', taskbarVisible);
})();

// 每次弹窗显示时，Rust 直接携带当前状态，同步更新对号（避免异步闪烁）
listen('popup-shown', (event) => {
  const { main_visible, taskbar_visible } = event.payload;
  showItem.classList.toggle('checked', main_visible);
  taskbarItem.classList.toggle('checked', taskbar_visible);
});

// 监听设置变更（从设置窗口同步任务栏状态）
listen('settings-changed', (event) => {
  if (event.payload && event.payload.taskbar_visible !== undefined) {
    taskbarItem.classList.toggle('checked', event.payload.taskbar_visible);
  }
});

// 监听主窗口显隐变更（点击"显示主窗口"后更新对号，避免下次打开闪烁）
listen('main-visibility-changed', (event) => {
  showItem.classList.toggle('checked', event.payload);
});

// 菜单项点击 → 调用 Rust 命令执行操作
document.querySelectorAll('.menu-item').forEach((item) => {
  item.addEventListener('click', () => {
    const action = item.dataset.action;
    if (action) {
      if (action === 'taskbar') {
        invoke('toggle_taskbar_visible');
      } else {
        invoke('tray_popup_action', { action });
      }
    }
  });
});

// 窗口失去焦点（用户点击了外部）→ 自动隐藏弹窗
window.addEventListener('blur', () => {
  invoke('hide_tray_popup_cmd');
});
