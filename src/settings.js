const { emit } = window.__TAURI__.event;
const { invoke } = window.__TAURI__.core;

// 启动时恢复主题
const savedTheme = localStorage.getItem('theme') || 'light';
document.documentElement.setAttribute('data-theme', savedTheme);
document.body.setAttribute('data-theme', savedTheme);
updateThemeButtons(savedTheme);

// 主题切换
document.querySelectorAll('[data-theme-value]').forEach((btn) => {
  btn.addEventListener('click', () => {
    const theme = btn.dataset.themeValue;
    document.documentElement.setAttribute('data-theme', theme);
    document.body.setAttribute('data-theme', theme);
    localStorage.setItem('theme', theme);
    updateThemeButtons(theme);
    // 通知所有窗口主题变更
    emit('theme-changed', theme);
  });
});

function updateThemeButtons(currentTheme) {
  document.querySelectorAll('[data-theme-value]').forEach((btn) => {
    btn.classList.toggle('active', btn.dataset.themeValue === currentTheme);
  });
}

// 任务栏显示设置
const taskbarVisible = await invoke('get_taskbar_visible');
updateTaskbarButtons(taskbarVisible);

document.querySelectorAll('[data-taskbar-value]').forEach((btn) => {
  btn.addEventListener('click', () => {
    const visible = btn.dataset.taskbarValue === 'true';
    invoke('set_taskbar_visible', { visible });
    updateTaskbarButtons(visible);
  });
});

function updateTaskbarButtons(visible) {
  document.querySelectorAll('[data-taskbar-value]').forEach((btn) => {
    btn.classList.toggle('active', btn.dataset.taskbarValue === String(visible));
  });
}

// 窗口置顶设置
const alwaysOnTop = await invoke('get_always_on_top');
updateTopButtons(alwaysOnTop);

document.querySelectorAll('[data-top-value]').forEach((btn) => {
  btn.addEventListener('click', () => {
    const enabled = btn.dataset.topValue === 'true';
    invoke('set_always_on_top', { enabled });
    updateTopButtons(enabled);
  });
});

function updateTopButtons(enabled) {
  document.querySelectorAll('[data-top-value]').forEach((btn) => {
    btn.classList.toggle('active', btn.dataset.topValue === String(enabled));
  });
}

// 流量单位设置
const netUnit = await invoke('get_net_unit');
updateUnitButtons(netUnit);

document.querySelectorAll('[data-unit-value]').forEach((btn) => {
  btn.addEventListener('click', () => {
    const unit = btn.dataset.unitValue;
    invoke('set_net_unit', { unit });
    updateUnitButtons(unit);
  });
});

function updateUnitButtons(currentUnit) {
  document.querySelectorAll('[data-unit-value]').forEach((btn) => {
    btn.classList.toggle('active', btn.dataset.unitValue === currentUnit);
  });
}
