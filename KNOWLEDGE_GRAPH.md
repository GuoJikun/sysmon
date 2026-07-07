# SysMon 项目知识图谱

> 本文档描述 SysMon 项目的组件关系、数据流、依赖链和关键决策点。

## 1. 实体关系图

### 1.1 模块依赖关系

```
                    ┌──────────┐
                    │ main.rs  │  入口点
                    └────┬─────┘
                         │ 调用
                         ▼
                    ┌──────────┐
                    │  lib.rs  │  应用配置中心
                    └──┬───┬───┘
            注册命令 ──┤   ├── 启动流程
                       │   │
         ┌─────────────┘   └──────────────┐
         ▼                                ▼
  ┌──────────────┐              ┌─────────────────┐
  │ settings.rs  │              │ start_data_push  │
  │ (命令+持久化) │              │ _timer()         │
  └──────┬───────┘              └──┬──────────┬────┘
         │                         │          │
         │ 读写 settings.json      │ 调用     │ 调用
         │                         ▼          ▼
         ▼                 ┌────────────┐ ┌──────────┐
  ┌──────────────┐         │ sys_info.rs│ │  gpu.rs  │
  │ AppSettings  │         │ CPU/内存/  │ │ PDH API  │
  │ {taskbar,    │         │ 网络采集    │ │ GPU采集   │
  │  always_top} │         └─────┬──────┘ └────┬─────┘
  └──────────────┘               │             │
                                 ▼             ▼
                            ┌─────────────────────┐
                            │   commands.rs       │
                            │ SystemInfo /        │
                            │ NetSpeedInfo 结构体  │
                            └─────────────────────┘
```

### 1.2 窗口体系

```
                    ┌─────────────────────┐
                    │     Tauri App       │
                    └──────┬──────┬───────┘
                           │      │
           ┌───────────────┤      │
           │               │      └──────────────┐
           ▼               ▼                     ▼
    ┌────────────┐  ┌────────────┐       ┌────────────┐
    │ main 窗口  │  │taskbar 窗口│       │settings 窗口│
    │ (静态配置) │  │ (动态创建) │       │ (动态创建)  │
    └─────┬──────┘  └─────┬──────┘       └─────┬──────┘
          │               │                     │
    监听事件:        监听事件:             监听事件:
    "sys-info"       "net-speed"           (无)
    "theme-changed"  "theme-changed"
          │               │                     │
          │               │              发送事件:
          │               │              "theme-changed"
          │               │                     │
          │               │              invoke 命令:
          │               │              get/set_taskbar_visible
          │               │              get/set_always_on_top
          │               │                     │
          ▼               ▼                     ▼
    ┌─────────────────────────────────────────────────┐
    │              Rust 后端 (lib.rs)                  │
    │  emit_to() 推送数据    invoke_handler 处理命令   │
    └─────────────────────────────────────────────────┘
```

### 1.3 托盘与窗口管理

```
┌──────────────────┐
│  tray.rs         │
│  setup_tray()    │
└──┬───┬───┬───────┘
   │   │   │
   ▼   ▼   ▼
 "show" "settings" "quit"
   │   │   │
   │   │   └─→ cleanup_taskbar_window() + gpu cleanup + exit
   │   │
   │   └─→ 创建/显示 settings 窗口
   │       (WebviewWindowBuilder, 300×240, skip_taskbar)
   │
   └─→ 显示 main 窗口 + set_focus

左键点击托盘 → 显示 main 窗口
```

## 2. 数据流图

### 2.1 系统信息采集流

```
┌──────────────────────────────────────────────────────────┐
│                    Windows 系统                           │
├──────────────┬───────────────┬───────────────────────────┤
│  sysinfo     │  sysinfo      │  PDH API                  │
│  CPU cores   │  Memory       │  \GPU Engine(*)\          │
│  usage()     │  used/total   │  Utilization Percentage   │
├──────────────┴───────────────┴───────────────────────────┤
│  Networks (sysinfo)                                      │
│  rx_bytes / tx_bytes (累计值)                            │
└──────┬───────────────┬───────────────────┬───────────────┘
       │               │                   │
       ▼               ▼                   ▼
┌────────────┐  ┌────────────┐      ┌────────────┐
│sys_info.rs │  │sys_info.rs │      │  gpu.rs    │
│get_current │  │compute_net │      │get_gpu_    │
│_info()     │  │_speed()    │      │usage()     │
│            │  │            │      │            │
│ CPU: 平均  │  │ 增量计算:  │      │ 过滤3D引擎 │
│ mem_pct    │  │ (当前-上次)│      │ 求和≤100%  │
│            │  │ /时间差    │      │            │
│ 过滤虚拟   │  │ 过滤虚拟   │      │            │
│ 网卡       │  │ 网卡       │      │            │
└──────┬─────┘  └──────┬─────┘      └──────┬─────┘
       │               │                   │
       └───────┬───────┘                   │
               │ ←─────────────────────────┘
               ▼
      ┌─────────────────┐
      │  SystemInfo {}  │
      │  cpu, mem_used, │
      │  mem_total,     │
      │  mem_pct, gpu,  │
      │  net_down,      │
      │  net_up         │
      └────────┬────────┘
               │
               ▼
      ┌─────────────────┐
      │  lib.rs         │
      │  emit_to("main",│
      │  "sys-info",    │
      │   SystemInfo)   │
      └────────┬────────┘
               │
               ▼
      ┌─────────────────┐
      │  main.js        │
      │  listen(        │
      │  "sys-info")    │
      │  → 更新 DOM     │
      └─────────────────┘
```

### 2.2 网速数据流（任务栏窗口）

```
sysinfo Networks
       │
       ▼
sys_info.rs::compute_net_speed()
       │  增量: (当前累计 - PREV_NET_RX) / 时间差
       │  过滤: Loopback, vEthernet, Hyper-V, docker
       ▼
NetSpeedInfo { down, up }
       │
       ▼
lib.rs::emit_to("taskbar", "net-speed", NetSpeedInfo)
       │
       ▼
taskbar.js::listen("net-speed")
       │  formatSpeedShort(): B → K → M
       ▼
DOM 更新: #net-down, #net-up
```

### 2.3 主题同步流

```
┌──────────────┐
│settings.js   │
│用户点击主题   │
└──────┬───────┘
       │
       ├─→ localStorage.setItem('theme', theme)
       ├─→ document.setAttribute('data-theme', theme)
       └─→ emit('theme-changed', theme)  ── 广播到所有窗口
                    │
          ┌─────────┴─────────┐
          ▼                   ▼
   ┌────────────┐      ┌────────────┐
   │ main.js    │      │taskbar.js  │
   │ listen(    │      │ listen(    │
   │"theme-     │      │"theme-     │
   │ changed")  │      │ changed")  │
   │            │      │            │
   │ setAttribute│      │ setAttribute│
   │('data-theme'│      │('data-theme'│
   │, theme)    │      │, theme)    │
   └────────────┘      └────────────┘
          │                   │
          ▼                   ▼
   ┌────────────────────────────────┐
   │     variables.css              │
   │  [data-theme="light"] { ... }  │
   │  [data-theme="dark"]  { ... }  │
   │  CSS 变量自动切换               │
   └────────────────────────────────┘
```

### 2.4 设置持久化流

```
┌──────────────┐
│settings.js   │
│用户点击按钮   │
└──────┬───────┘
       │
       ▼
invoke('set_taskbar_visible', { visible: true })
invoke('set_always_on_top', { enabled: true })
       │
       ▼
┌──────────────────────────┐
│ settings.rs              │
│ set_taskbar_visible()    │
│ set_always_on_top()      │
│                          │
│ 1. 读取现有 settings.json│
│ 2. 合并新值              │
│ 3. 写回 settings.json    │
│ 4. 立即应用 (hide/show   │
│    window, set_always_   │
│    on_top)               │
└──────────┬───────────────┘
           │
           ▼
┌──────────────────────────┐
│ %APPDATA%\com.sysmon.app\│
│ settings.json            │
│ {                        │
│   "taskbar_visible":true,│
│   "always_on_top":true   │
│ }                        │
└──────────────────────────┘

启动时:
lib.rs → apply_taskbar_setting() → 读 settings.json → 应用
       → apply_always_on_top_setting() → 读 settings.json → 应用
```

## 3. 任务栏嵌入流程

```
┌─────────────────────────────────────────────────────┐
│ taskbar_window.rs::create_taskbar_window()          │
└──────────────────────┬──────────────────────────────┘
                       │
                       ▼
              ┌────────────────┐
              │ 创建 WebView    │
              │ 窗口 (taskbar)  │
              │ 120×32 px      │
              └───────┬────────┘
                      │
                      ▼
              ┌────────────────┐
              │ 查找窗口层级    │
              │ Shell_TrayWnd  │
              │  → ReBarWindow32│
              │   → MSTaskSwWClass│
              └───────┬────────┘
                      │
               找到?  │
              ┌───────┴───────┐
              ▼               ▼
         模式 A: 嵌入     模式 B: 贴边置顶
              │               │
              ▼               ▼
    ┌─────────────────┐  ┌──────────────────┐
    │ 1.保存原始尺寸   │  │ 获取任务栏矩形    │
    │ 2.缩小MSTaskSw  │  │ 定位到任务栏右上角│
    │   留出120px     │  │ HWND_TOPMOST置顶 │
    │ 3.SetParent到   │  └──────────────────┘
    │   ReBarWindow32 │
    │ 4.WS_EX_NOACTIVATE│
    │   不抢焦点       │
    │ 5.定位到右侧     │
    └─────────────────┘
              │
              ▼
    ┌─────────────────────┐
    │ 重定位定时器 (5s)    │
    │ 检查任务栏位置变化   │
    │ 变化则重新定位       │
    └─────────────────────┘

退出时:
    cleanup_taskbar_window()
    → SetParent 恢复
    → 恢复 MSTaskSwWClass 原始尺寸
```

## 4. 依赖关系矩阵

### 4.1 Rust 依赖

| Crate | 用途 | 使用模块 |
|-------|------|----------|
| `tauri` (v2) | 桌面框架 | lib.rs, tray.rs, taskbar_window.rs, settings.rs |
| `tauri-plugin-opener` | 打开外部链接 | lib.rs |
| `sysinfo` (0.33) | CPU/内存/网络 | sys_info.rs |
| `serde` / `serde_json` | 序列化 | commands.rs, settings.rs |
| `tokio` | 异步定时器 | lib.rs |
| `windows` (0.62) | Win32 API | gpu.rs, taskbar_window.rs |

### 4.2 Windows API 使用

| API | 模块 | 用途 |
|-----|------|------|
| PDH (Performance Data Helper) | gpu.rs | GPU 利用率采集 |
| `FindWindowW` / `FindWindowExW` | taskbar_window.rs | 查找任务栏窗口 |
| `SetParent` | taskbar_window.rs | 嵌入窗口到任务栏 |
| `SetWindowLongPtrW` | taskbar_window.rs | 设置窗口样式 |
| `GetWindowRect` / `MoveWindow` | taskbar_window.rs | 窗口定位 |
| `GetDpiForWindow` | taskbar_window.rs | DPI 感知 |
| `SetWindowPos` | taskbar_window.rs | HWND_TOPMOST 置顶 |

### 4.3 Tauri 事件清单

| 事件名 | 发送方 | 接收方 | Payload |
|--------|--------|--------|---------|
| `sys-info` | lib.rs (Rust) | main 窗口 | `SystemInfo` |
| `net-speed` | lib.rs (Rust) | taskbar 窗口 | `NetSpeedInfo` |
| `theme-changed` | settings.js (前端) | main + taskbar 窗口 | `string` ("light"/"dark") |

### 4.4 Tauri 命令清单

| 命令名 | 定义位置 | 调用方 | 返回值 |
|--------|----------|--------|--------|
| `get_taskbar_visible` | settings.rs | settings.js | `bool` |
| `set_taskbar_visible` | settings.rs | settings.js | `()` |
| `get_always_on_top` | settings.rs | settings.js | `bool` |
| `set_always_on_top` | settings.rs | settings.js | `()` |

## 5. 关键技术决策

| 决策 | 选择 | 原因 |
|------|------|------|
| GPU 监控方式 | PDH API | 比 NVAPI/D3DKMT 更通用，支持所有 GPU 厂商 |
| 网速计算 | 增量法 | sysinfo 只提供累计值，需自行计算差值 |
| 任务栏嵌入 | SetParent | 唯一能真正嵌入任务栏的方式，贴边只是 fallback |
| 全局状态 | OnceLock+Mutex | Rust 安全的全局可变状态模式 |
| 前端框架 | 无 | 窗口极简，原生 JS 足够，避免构建复杂度 |
| 主题方案 | CSS 变量 | 运行时切换零成本，不需要重新编译 |
| 窗口拖拽 | 透明覆盖层 | 无边框窗口下实现全区域可拖拽 |
| 设置存储 | JSON 文件 | 简单直观，无需数据库依赖 |

## 6. 配置文件关系

```
tauri.conf.json
  ├── windows[0] → main 窗口 (label: "main")
  │     ├── url: main.html
  │     ├── decorations: false (无边框)
  │     ├── skipTaskbar: true (不在任务栏)
  │     └── alwaysOnTop: true (置顶)
  │
  ├── capabilities/default.json → main 窗口权限
  │     ├── core:window:allow-start-dragging
  │     ├── core:window:allow-set-always-on-top
  │     └── core:window:allow-hide/show/close
  │
  ├── capabilities/taskbar.json → taskbar 窗口权限
  │     └── core:event:default + core:window:default
  │
  └── capabilities/settings.json → settings 窗口权限
        └── core:window:allow-close

运行时配置:
  %APPDATA%/com.sysmon.app/settings.json
    ├── taskbar_visible: bool
    └── always_on_top: bool
```
