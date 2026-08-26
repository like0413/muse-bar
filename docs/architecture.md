# Muse Bar 架构与数据流

本文描述首版的目标架构。当前还没有实现的目录会在对应阶段首次使用时创建，
不会为了“看起来完整”而提前放置空模块。

## 四层职责

| 层级   | 主要技术               | 只负责什么                                   | 不负责什么                   |
| ------ | ---------------------- | -------------------------------------------- | ---------------------------- |
| 界面层 | Vue、Pinia、shadcn-vue | 渲染 Bar/设置页、接收输入、展示最新状态      | 不调用 Win32，不选择媒体会话 |
| 桥接层 | Tauri commands/events  | 校验窗口权限、传递请求、向指定窗口推送事件   | 不保存两份业务状态           |
| 应用层 | Rust                   | 设置、媒体选择、窗口策略、错误和生命周期管理 | 不依赖 Vue DOM               |
| 系统层 | Windows API、WinRT     | 任务栏、DPI、媒体会话和 Explorer 交互        | 不暴露原始句柄给前端         |

依赖方向只能从上到下：

```text
Vue 页面 -> Tauri 命令 -> Rust 应用服务 -> Windows 适配器
Vue Store <- Tauri 事件 <- 归一化状态 <- Windows 回调
```

前端永远不接收 `HWND`、`RECT` 或 WinRT 对象，只接收可序列化的数据结构。

## Tauri API 的实现位置

当 Tauri 同时提供 JavaScript 和 Rust API 时，先判断资源的生命周期：

- 页面内、窗口内或由用户动作触发的能力优先使用前端 JavaScript API。
- 初始窗口在前端运行前就必须存在，因此使用 `tauri.conf.json` 声明。
- 系统托盘、单实例、全局快捷键、全局媒体监听和 Explorer 恢复属于进程级唯一资源，
  由 Rust 创建并持有。
- 任务栏挂载涉及 `HWND` 和 Win32 生命周期，仍由 Rust 管理。
- 前端模块顶层禁止创建窗口、托盘或监听器；副作用必须绑定到明确的用户动作或页面
  生命周期，并在卸载时清理。

多个窗口都会执行 Vue 入口，因此按需窗口必须使用稳定 label 查重。Bar 创建 Settings
窗口时还会复用同一个 in-flight Promise，避免连续点击在异步查询期间创建两个同名
窗口。Settings 路由不会导入 Bar 的懒加载页面模块，因此自身不会再次执行创建逻辑。

## 两条通信路径

### 命令：前端主动请求

命令适合读取设置或执行一次控制动作：

```text
用户点击播放
  -> Bar 调用 Tauri command
  -> Rust 校验 ControlAction 与当前能力
  -> Windows 媒体会话执行动作
  -> command 返回成功或结构化错误
```

命令的返回值只表示这次请求是否被接受。最终播放状态仍由媒体事件更新，避免前端
“猜测”播放器已经成功响应。

### 事件：Rust 主动推送

事件适合媒体、设置和任务栏状态变化：

```text
Windows 会话发生变化
  -> Rust 读取并归一化状态
  -> 更新 AppState 中的唯一快照
  -> 向 bar 窗口发送 media-changed
  -> Pinia 替换最新 MediaSnapshot
  -> Vue 自动重新渲染
```

事件载荷使用完整快照而不是零散字段，前端不会短暂组合出“新标题 + 旧封面”的
不一致状态。

## 四个核心数据类型

- `AppSettings`：窗口模式、位置、宽度限制、偏移、进度样式和开机启动。
- `MediaSnapshot`：播放器身份、媒体属性、播放状态、时间轴和控制能力。
- `TaskbarRuntimeState`：实际宿主模式、任务栏矩形、DPI、挂载与碰撞状态。
- `ControlAction`：播放/暂停、上一曲、下一曲和目标播放位置。

这些类型以 Rust 定义为准。需要前端类型时，在桥接边界提供对应 TypeScript 类型，
字段命名和可空性必须逐项一致。

## 状态所有权

| 状态                 | 唯一所有者      | 其他层如何使用                 |
| -------------------- | --------------- | ------------------------------ |
| 持久化设置           | Rust `AppState` | Vue 通过命令读取，通过事件同步 |
| 当前媒体与会话选择   | Rust 媒体服务   | Bar 只显示 `MediaSnapshot`     |
| 任务栏句柄和挂载状态 | Rust 任务栏宿主 | 诊断页只读取安全摘要           |
| 进度动画的帧间估算   | Bar 页面        | 定期由 Rust 时间轴快照校准     |
| 设置表单草稿         | 设置页面        | 保存成功后以 Rust 返回值为准   |

## 当前目录与依赖边界

```text
src/
  components/ui/                  shadcn-vue 源码组件
  lib/media-types.ts              媒体 IPC 的共享 DTO
  lib/media-query-api.ts          媒体快照与会话选择查询
  lib/media-control-api.ts        用户媒体控制请求
  lib/media-event-api.ts          Rust 到前端的媒体事件
  lib/media-diagnostics-api.ts    设置页媒体诊断查询
  pages/bar/                       Bar Store 与显示组件
  pages/settings/                  设置窗口 Store 与分区组件
  router/                          /bar 与 /settings 的 Hash Router

src-tauri/src/
  commands/                       Tauri command 薄入口
  media/                          媒体领域 facade
    model.rs                      可序列化 DTO 与播放器识别策略
    selection.rs                  只依赖普通数据的会话选择策略
    artwork.rs                    封面读取、格式识别与颜色提取
    activity.rs                   活动跟踪与选择候选采集
    control.rs                    当前会话控制
    runtime.rs                    WinRT 媒体运行时与事件协调
  taskbar/                        任务栏领域 facade
    system.rs                     任务栏发现与运行时信息
    layout.rs                     只依赖矩形数据的布局决策
    occupancy.rs                  UI Automation / Win32 任务栏采集
    host.rs                       Child 窗口挂载和位置维护
    explorer.rs                   Explorer 重建监控与恢复
    bar.rs                        内容宽度到原生窗口尺寸的编排
  settings/                       设置领域 facade
    mod.rs                        AppSettings、归一化与 JSON 持久化
    update.rs                     启动项、保存和事件广播事务
  state.rs                        AppState 与跨线程共享状态

docs/              架构、基线与兼容性记录
```

三个领域通过各自的 `mod.rs` 只重导出调用方需要的最小接口，内部实现模块默认私有。
`media::selection` 和 `taskbar::layout` 不接收 WinRT、`HWND`
或 Tauri handle；Windows 采集结果先转换为普通 DTO 或矩形，再交给策略模块。前端只有
`src/lib/*-api.ts` 可以导入 Tauri 的 `invoke`，Vue 组件通过 Store 或 typed API 使用能力。

设置外观分区采用以下单向数据流：

```text
AppearanceSettingsSection（Store 连接容器）
  -> 各 Appearance*Card（只读 props）
    -> BarAppearancePreview（纯展示 props）
  <- change(SettingsPatch) 事件
```

子卡片不导入 Pinia Store；设置写入仍由连接容器调用唯一的 `saveSettingsPatch` action。

## 注释约定

- 项目自有函数说明用途、输入输出或需要遵守的约束。
- 并发、生命周期、回退策略和平台差异等难理解逻辑，需要解释“为什么这样实现”。
- 项目自有代码的注释统一使用中文。
- 显而易见的简单赋值、框架配置回调和 shadcn-vue 生成源码不堆砌重复代码含义的注释。
- 修改复杂逻辑时同步更新相邻注释，不能让注释与实际行为分离。

## 前端组件约束

- 优先使用仓库已安装的 shadcn-vue 组件。
- 设置表单使用 `FieldGroup + Field`，二到七个选项使用 `ToggleGroup`。
- 使用 Tailwind 语义颜色，不写固定的亮色/暗色覆盖。
- `Avatar` 必须包含失败回退；图标按钮必须有可访问名称。
- Bar 的显示 Store 集中订阅 Tauri 事件，页面卸载时释放监听，避免重复订阅。

## 阶段验收方式

本项目不维护自动化测试套件。每章至少运行前端格式、lint、类型检查、Rustfmt、Cargo check、
严格 Clippy 和生产构建，并包含一种可观察结果：诊断 JSON、窗口行为或日志。涉及 Child 挂载、
自动隐藏、全屏和 Explorer 重启的行为必须在真实 Windows 11 环境人工验证。
