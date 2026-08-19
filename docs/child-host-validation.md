# Child 任务栏宿主技术闸门

本文记录阶段 4 的技术验证结果和后续宿主路线。记录日期为 2026-08-19，结论仅代表
当前人工验证环境；正式发布前仍需执行完整 Windows 兼容矩阵。

## 验证范围

| 项目 | 结果 | 观察依据 |
| ---- | ---- | -------- |
| 无边框透明 WebView 渲染 | 通过 | 彩色测试块可在透明 Bar 窗口中正常渲染 |
| Child 窗口样式 | 通过 | `WS_POPUP` 已移除，Child、裁剪和分层样式写入后可读回 |
| 挂载主任务栏 | 通过 | `SetParent` 后通过 `GetParent` 验证父窗口为主任务栏 |
| 鼠标悬停和点击 | 通过 | Vue 点击计数正确递增，悬停不再触发下方任务栏应用的 Tooltip |
| 前台焦点 | 通过 | 点击 Bar 前后前台窗口句柄不变 |
| 任务栏自动隐藏 | 通过 | Bar 随父任务栏隐藏和重新出现，桌面边缘没有独立残留 |
| 全屏行为 | 通过 | 全屏内容不会被 Bar 覆盖，退出全屏后 Bar 随任务栏恢复 |
| Explorer 重启恢复 | 通过 | 应用保持运行，并重新创建 WebView、应用 Child 样式和挂载 |

## 输入问题排查

最初的 Child 可以正常显示，但整个 Bar 的悬停和点击都会穿透。排查过程确认：

- 命中采样落在 Bar 的 `Chrome_RenderWidgetHostHWND` 内，不是坐标偏移。
- WebView、Tauri 宿主和放在宿主内的原生 Win32 按钮都收不到鼠标消息。
- 移除 `WS_EX_NOACTIVATE`、返回 `MA_NOACTIVATE` 或移动到任务栏空白区均无效。
- 鼠标停留在 Bar 上时，下方任务栏应用仍显示 Tooltip。

FluentFlyout 的实现提供了关键对照：其 WPF 窗口先以支持透明的顶层窗口创建，再改为
`WS_CHILD` 并挂载任务栏。WPF 的 `AllowsTransparency=True` 会使用分层窗口；其可交互
内容也直接绘制在这个根 HWND 中。

为区分 WPF、WebView2 与窗口样式的影响，验证按以下顺序进行：

1. 创建一个直接以 `Shell_TrayWnd` 为父窗口的普通原生根 Child。坐标可以被
   `WindowFromPoint` 命中，但悬停和点击仍未到达。
2. 尝试从创建时就使用跨进程分层 Child，`CreateWindowExW` 被 Windows 拒绝。
3. 改为先创建顶层 `WS_EX_LAYERED` 窗口，设置分层属性，再修改为 Child 并调用
   `SetParent`。原生根窗口随即收到 `WM_MOUSEMOVE`、`WM_MOUSEACTIVATE`、
   `WM_LBUTTONDOWN` 和 `WM_LBUTTONUP`。
4. 对 Tauri 原生宿主采用相同生命周期：Tauri 先按顶层窗口创建宿主；在调用
   `SetParent` 之前，Rust 写入 Child 与 `WS_EX_LAYERED | WS_EX_NOACTIVATE` 样式并设置
   统一 Alpha；挂载任务栏后，最后通过 `Window::add_child` 创建 WebView。
5. 真实鼠标点击后，UI Automation 读取到 Vue 按钮从“已点击 0 次”变为“已点击 1 次”。
   点击前后前台窗口句柄一致；在 Bar 上悬停 2.2 秒也没有新增 Explorer Tooltip 或预览窗口。

因此根因不是 Vue、CSS 或 WebView2，而是任务栏对外部窗口的输入路由要求根窗口在挂载前
已经具备分层窗口属性。只在挂载后调整透明背景，或者创建普通 Child，都只能得到视觉嵌入，
无法得到正确的指针输入。

相关资料：

- [WPF AllowsTransparency](https://learn.microsoft.com/en-us/dotnet/api/system.windows.window.allowstransparency)
- [Windows Child 窗口关系与消息](https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features)
- [SetLayeredWindowAttributes](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setlayeredwindowattributes)
- [FluentFlyout 的 TaskbarWindow 实现](https://github.com/unchihugo/FluentFlyout/blob/master/FluentFlyoutWPF/Windows/TaskbarWindow.xaml.cs)
- [FluentFlyout 使用窗口区域避免阻塞任务栏消息的提交](https://github.com/unchihugo/FluentFlyout/commit/20fe632eab15aecbd5f587efcc753c216e934a8e)

本次验证机器为 Windows 11 25H2、Build 26200.8894。DPI 和显示器组合尚未形成完整记录，
阶段 12 仍需覆盖兼容矩阵。

## 技术结论

阶段 4 的技术闸门结论为 **Child 通过**：

1. 阶段 5 将 Child 整理为默认正式宿主。
2. 保留 Owner 兼容模式，供未来 Windows 版本、第三方任务栏或挂载恢复失败时回退。
3. `auto` 模式先尝试 Child，达到失败阈值后仅在本次运行中切换 Owner。
4. Explorer 重启恢复仍由唯一的 Rust 生命周期管理器负责，前端窗口不创建全局监听器。

## 后续验证

阶段 5 需要继续验证正式 Child 封装和 Owner 回退不会改变当前输入行为。阶段 8 加入真实媒体
控制后，补验按钮连续点击、拖动 seek 和长时间悬停。阶段 12 再覆盖 Windows 版本、DPI、
任务栏高度、多屏、锁屏恢复、第三方任务栏修改器和不同类型的全屏应用。
