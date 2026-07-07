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
  ┌──────────────┐              ┌─────────────────────┐
  │ settings.rs  │              │ start_data_push      │
  │ (命令+持久化) │              │ _timer()             │
  └──────┬───────┘              └──┬──────────┬────────┘
         │                         │          │
         │ 读写 settings.json      │ 调用     │ 调用
         │                         ▼          ▼
         ▼                 ┌────────────┐ ┌──────────┐
  ┌──────────────┐         │ sys_info.rs│ │  gpu.rs  │
  │ AppSettings  │         │ CPU/内存/  │ │ PDH API  │
  │ {taskbar,    │         │ 网络/格式化 │ │ GPU采集   │
  │  always_top, │         └─────┬──────┘ └────┬─────┘
  │  net_unit}   │               │             │
  └──────────────┘               │             │
                                 ▼             ▼
                            ┌─────────────────────┐
                            │   commands.rs       │
                            │ SystemInfo {        │
                            │   cpu, mem_*, gpu,  │
                            │   net_down/up,      │
                            │   net_down/up_str } │
                            │ NetSpeedInfo {      │
                            │   down, up,         │
                            │   down_str, up_str }│
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
    │ 230×46     │  │ 120×32     │       │ 800×600    │
    │ 透明+圆角  │  │ Win32嵌入  │       │            │
    │ 置顶+无框  │  │ 默认隐藏   │       │            │
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
          │               │              get/set_net_unit
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
   │       (WebviewWindowBuilder, 800×600, skip_taskbar)
   │
   └─→ 显示 main 窗口 + set_focus

左键点击托盘 → 显示 main 窗口
```

### 1.4 主窗口圆角实现

```
tauri.conf.json                    lib.rs                           CSS
┌────────────────┐    ┌──────────────────────────────┐    ┌─────────────────┐
│ transparent:   │    │ set_main_window_rounded_     │    │ main.css:       │
│   true         │    │   region()                   │    │  border-radius: │
│ shadow: false  │    │                              │    │    10px         │
│ decorations:   │    │ GetWindowRect(hwnd)          │    │                 │
│   false        │    │ → width, height              │    │ public.css:    │
│                │    │                              │    │  border-radius: │
│                │    │ CreateRoundRectRgn(          │    │    12px         │
│                │    │   0, 0, w+1, h+1,            │    │                 │
│                │    │   16, 16)                    │    │ body:           │
│                │    │                              │    │  background:    │
│                │    │ SetWindowRgn(hwnd,           │    │   var(--bg-rgba)│
│                │    │   region, true)              │    │  ← rgba 半透明  │
└────────────────┘    └──────────────────────────────┘    └─────────────────┘
       │                        │                                │
       └────────────┬───────────┘                                │
                    ▼                                             │
          HWND 被裁剪为圆角矩形                                    │
          角落外区域真正透明                                       │
          CSS border-radius 裁剪 HTML 内容 ◄──────────────────────┘
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
       │  读取 net_unit │                   │
       │  (auto/kb/mb)  │                   │
       │               │                   │
       │  ┌────────────┘                   │
       ▼  ▼                                │
  ┌──────────────────┐                     │
  │ format_speed()   │  ←──────────────────┘
  │ format_speed_    │
  │   short()        │
  │                  │
  │ 根据 unit 格式化  │
  │ → "1.5 KB/s"     │
  │ → "1K" (短格式)   │
  └────────┬─────────┘
           │
           ▼
  ┌─────────────────────────────┐
  │  SystemInfo {}              │
  │  cpu, mem_used, mem_total,  │
  │  mem_pct, gpu,              │
  │  net_down, net_up,          │
  │  net_down_str, net_up_str   │
  └────────┬────────────────────┘
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
  │                 │
  │  直接用 *_str   │
  │  更新 DOM       │
  └─────────────────┘
```

### 2.2 网速数据流（任务栏窗口）

```
sysinfo Networks
       │
       ▼
sys_info.rs::compute_net_speed()
       │  增量: (当前累计 - PREV_NET_RX) / 时间差
       │  过滤: Loopback, vEthernet, Hyper-V, docker, veth, vnic
       ▼
       │  读取 net_unit 设置
       ▼
format_speed_short(down, unit)
       │  auto → B/K/M 自适应
       │  kb   → "{:.0}K"
       │  mb   → "{:.1}M"
       ▼
NetSpeedInfo { down, up, down_str, up_str }
       │
       ▼
lib.rs::emit_to("taskbar", "net-speed", NetSpeedInfo)
       │
       ▼
taskbar.js::listen("net-speed")
       │  直接用 data.down_str / data.up_str
       │  (前端无格式化逻辑)
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
   │  --bg-rgba: rgba(...)          │
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
invoke('set_net_unit', { unit: "auto" })
       │
       ▼
┌──────────────────────────────┐
│ settings.rs                  │
│ set_taskbar_visible()        │
│ set_always_on_top()          │
│ set_net_unit()               │
│                              │
│ 1. 读取现有 settings.json    │
│ 2. 合并新值                  │
│ 3. 写回 settings.json        │
│ 4. 立即应用 (hide/show,      │
│    set_always_on_top)        │
│    (net_unit 无即时应用,     │
│     下个推送周期自动生效)     │
└──────────┬───────────────────┘
           │
           ▼
┌──────────────────────────────┐
│ %APPDATA%\com.sysmon.app\    │
│ settings.json                │
│ {                            │
│   "taskbar_visible": false,  │
│   "always_on_top": true,     │
│   "net_unit": "auto"         │
│ }                            │
└──────────────────────────────┘

启动时:
lib.rs → apply_taskbar_setting()      → 读 settings.json → 隐藏/显示
       → apply_always_on_top_setting() → 读 settings.json → set_always_on_top
       → (net_unit 无启动 apply, 由 timer 每周期读取)

运行时:
start_data_push_timer() → get_net_unit_runtime() → 每个 tick 读取当前单位
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

启动后:
    settings::apply_taskbar_setting()
    → 如果 taskbar_visible == false (默认)
    → 隐藏 taskbar 窗口

退出时:
    cleanup_taskbar_window()
    → SetParent 恢复
    → 恢复 MSTaskSwWClass 原始尺寸
```

## 4. 依赖关系矩阵

### 4.1 Rust 依赖

| Crate | 版本 | 用途 | 使用模块 |
|-------|------|------|----------|
| `tauri` | v2 | 桌面框架 | lib.rs, tray.rs, taskbar_window.rs, settings.rs |
| `tauri-plugin-opener` | v2 | 打开外部链接 | lib.rs |
| `sysinfo` | 0.33 | CPU/内存/网络 | sys_info.rs |
| `serde` / `serde_json` | 1 | 序列化 | commands.rs, settings.rs |
| `tokio` | 1 | 异步定时器 | lib.rs |
| `windows` | **0.61** | Win32 API | gpu.rs, taskbar_window.rs, lib.rs |

> **版本约束**：`windows` crate 必须用 0.61，与 `tauri-runtime` 内部版本一致，否则 `HWND` 类型不匹配。

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
| `CreateRoundRectRgn` | lib.rs | 创建圆角区域（主窗口圆角） |
| `SetWindowRgn` | lib.rs | 裁剪窗口为圆角区域 |
| `GetWindowRect` | lib.rs | 获取窗口尺寸（圆角计算用） |

### 4.3 Tauri 事件清单

| 事件名 | 发送方 | 接收方 | Payload |
|--------|--------|--------|---------|
| `sys-info` | lib.rs (Rust) | main 窗口 | `SystemInfo`（含 `net_down_str`/`net_up_str`） |
| `net-speed` | lib.rs (Rust) | taskbar 窗口 | `NetSpeedInfo`（含 `down_str`/`up_str`） |
| `theme-changed` | settings.js (前端) | main + taskbar 窗口 | `string` ("light"/"dark") |

### 4.4 Tauri 命令清单

| 命令名 | 定义位置 | 调用方 | 返回值 | 说明 |
|--------|----------|--------|--------|------|
| `get_taskbar_visible` | settings.rs | settings.js | `bool` | 默认 `false` |
| `set_taskbar_visible` | settings.rs | settings.js | `()` | 立即隐藏/显示 taskbar 窗口 |
| `get_always_on_top` | settings.rs | settings.js | `bool` | 默认 `true` |
| `set_always_on_top` | settings.rs | settings.js | `()` | 立即设置置顶 |
| `get_net_unit` | settings.rs | settings.js | `String` | 默认 `"auto"` |
| `set_net_unit` | settings.rs | settings.js | `()` | 下个推送周期生效 |

### 4.5 设置项一览

| 设置项 | 字段名 | 类型 | 默认值 | 立即生效 | 说明 |
|--------|--------|------|--------|----------|------|
| 主题 | (localStorage) | string | "light" | 是 | 通过 emit 同步所有窗口 |
| 任务栏显示 | `taskbar_visible` | bool | `false` | 是 | 隐藏/显示 taskbar 窗口 |
| 窗口置顶 | `always_on_top` | bool | `true` | 是 | set_always_on_top |
| 流量单位 | `net_unit` | string | `"auto"` | 延迟 ≤1.5s | 下个推送周期读取 |

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
| 网速格式化位置 | Rust 端 | 前端零逻辑，切换单位只需后端处理 |
| 窗口圆角 | Win32 CreateRoundRectRgn | `windowEffects`(acrylic) 填满矩形导致圆角无效，必须裁剪 HWND 区域 |
| windows crate 版本 | 0.61 | 与 tauri-runtime 内部一致，避免 HWND 类型冲突 |
| 窗口半透明 | CSS rgba 背景 | `--bg-rgba` 变量，配合 `transparent: true` 实现半透明效果 |
| 任务栏默认状态 | 隐藏 | 减少干扰，用户按需在设置中开启 |
| 主窗口尺寸 | 230×46 | 超紧凑双行布局，长时间悬浮不遮挡 |

## 6. 配置文件关系

```
tauri.conf.json
  ├── windows[0] → main 窗口 (label: "main")
  │     ├── url: main.html
  │     ├── width: 230, height: 46 (超紧凑)
  │     ├── decorations: false (无边框)
  │     ├── transparent: true (透明窗口)
  │     ├── shadow: false
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
        ├── core:event:default
        └── core:window:allow-close

CSS 文件关系:
  public.css (公共 reset + 字体, 无 CSS 变量)
  variables.css (CSS 变量: 主题色 + --bg-rgba 半透明背景)
  main.css (主窗口: .drag-overlay, .row, .metric)
  settings.css (设置窗口: .settings-panel, .setting-item, .theme-option)
  taskbar.css (任务栏窗口)

运行时配置:
  %APPDATA%/com.sysmon.app/settings.json
    ├── taskbar_visible: bool  (默认 false)
    ├── always_on_top: bool    (默认 true)
    └── net_unit: string       (默认 "auto", 可选 "kb"/"mb")

  localStorage (浏览器端):
    └── theme: string          (默认 "light")
```

## 7. 数据结构定义

### SystemInfo（推送到 main 窗口）

```rust
struct SystemInfo {
    cpu: f32,              // CPU 使用率 %
    mem_used: u64,         // 已用内存 (bytes)
    mem_total: u64,        // 总内存 (bytes)
    mem_pct: f32,          // 内存使用率 %
    gpu: f32,              // GPU 使用率 %（当前未显示）
    net_down: f64,         // 下载速度 (bytes/s, 原始值)
    net_up: f64,           // 上传速度 (bytes/s, 原始值)
    net_down_str: String,  // 下载速度格式化字符串 ("1.5 KB/s")
    net_up_str: String,    // 上传速度格式化字符串
}
```

### NetSpeedInfo（推送到 taskbar 窗口）

```rust
struct NetSpeedInfo {
    down: f64,             // 下载速度 (bytes/s, 原始值)
    up: f64,               // 上传速度 (bytes/s, 原始值)
    down_str: String,      // 下载速度短格式 ("1K")
    up_str: String,        // 上传速度短格式
}
```

### AppSettings（持久化到 settings.json）

```rust
struct AppSettings {
    taskbar_visible: Option<bool>,   // 默认 false
    always_on_top: Option<bool>,     // 默认 true
    net_unit: Option<String>,        // 默认 "auto"
}
```
