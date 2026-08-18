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

## 计划中的目录边界

```text
src/
  components/ui/   shadcn-vue 源码组件
  pages/           Bar 与设置页，只组合 UI
  stores/          媒体和设置的前端投影
  router/          /bar 与 /settings 的 Hash Router
  types/           可序列化桥接类型
  __tests__/       前端单元测试

src-tauri/src/
  commands/        Tauri command 薄入口
  events/          事件名称和发送帮助函数
  settings/        AppSettings 与 JSON 持久化
  media/           WinRT 会话读取、选择与控制
  taskbar/         Win32 发现、测量和窗口宿主
  state/           AppState 与跨线程共享状态

src-tauri/tests/   Rust 集成测试
docs/              架构、基线与兼容性记录
```

模块首次出现时只公开当前步骤需要的最小接口。Windows 专用模块使用
`#[cfg(target_os = "windows")]` 隔离，纯逻辑（例如媒体选择器）保持不依赖 Win32，
以便直接单元测试。

## 前端组件约束

- 优先使用仓库已安装的 shadcn-vue 组件。
- 设置表单使用 `FieldGroup + Field`，二到七个选项使用 `ToggleGroup`。
- 使用 Tailwind 语义颜色，不写固定的亮色/暗色覆盖。
- `Avatar` 必须包含失败回退；图标按钮必须有可访问名称。
- Bar 的显示 Store 集中订阅 Tauri 事件，页面卸载时释放监听，避免重复订阅。

## 阶段验收方式

每个步骤至少包含一种可观察结果：测试输出、诊断 JSON、窗口行为或日志。阶段结束时
再运行 README 中的完整检查集。涉及 Child 挂载、自动隐藏、全屏和 Explorer 重启的
行为必须在真实 Windows 11 环境人工验证，单元测试不能替代这些系统级验收。
