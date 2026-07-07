// 使用 Tauri 全局 API（withGlobalTauri: true）
const { listen } = window.__TAURI__.event;

// 监听系统信息事件
const unlisten = await listen('sys-info', (event) => {
  const data = event.payload;

  // CPU
  document.getElementById('cpu-value').textContent = `${data.cpu.toFixed(1)}%`;
  document.getElementById('cpu-bar').style.width = `${data.cpu}%`;

  // 内存
  const memUsedGB = (data.mem_used / 1073741824).toFixed(1);
  const memTotalGB = (data.mem_total / 1073741824).toFixed(1);
  document.getElementById('mem-value').textContent = `${memUsedGB} GB / ${memTotalGB} GB`;
  document.getElementById('mem-bar').style.width = `${data.mem_pct}%`;

  // GPU
  document.getElementById('gpu-value').textContent = `${data.gpu.toFixed(1)}%`;
  document.getElementById('gpu-bar').style.width = `${data.gpu}%`;

  // 网速
  document.getElementById('net-down-value').textContent = formatSpeed(data.net_down);
  document.getElementById('net-up-value').textContent = formatSpeed(data.net_up);
});

function formatSpeed(bytesPerSec) {
  if (bytesPerSec < 1024) {
    return `${bytesPerSec.toFixed(0)} B/s`;
  } else if (bytesPerSec < 1048576) {
    return `${(bytesPerSec / 1024).toFixed(1)} KB/s`;
  } else {
    return `${(bytesPerSec / 1048576).toFixed(1)} MB/s`;
  }
}

// 主题切换
document.getElementById('theme-toggle').addEventListener('click', () => {
  const current = document.documentElement.getAttribute('data-theme');
  const next = current === 'dark' ? 'light' : 'dark';
  document.documentElement.setAttribute('data-theme', next);
  document.body.setAttribute('data-theme', next);
  localStorage.setItem('theme', next);
});

// 启动时恢复主题
const savedTheme = localStorage.getItem('theme') || 'light';
document.documentElement.setAttribute('data-theme', savedTheme);
document.body.setAttribute('data-theme', savedTheme);
