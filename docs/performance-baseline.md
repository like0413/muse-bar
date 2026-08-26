# Muse Bar 性能基线

更新日期：2026-08-26。

## 启动里程碑

Rust 日志一次性记录：

- `Tauri setup complete`
- `Bar WebView created`

WebView 开发者工具通过 `performance.mark` 和 `[startup]` 控制台日志记录：

- `vue-mounted`
- `listeners-ready`
- `first-media-snapshot`
- `bar-interactive`

这些时间点用于比较同一机器、同一构建模式下的回归，不把开发构建耗时当作发布性能。
本轮受控开发启动在 Rust 进程开始计时后记录到 `Tauri setup complete +32.2 ms`、
`Bar WebView created +442.8 ms`；该样本不包含首次 Cargo 编译时间。

## 前端 Bundle

| 产物                |    优化前 |    优化后 | 说明                                        |
| ------------------- | --------: | --------: | ------------------------------------------- |
| Settings 主 chunk   | 210.34 kB | 126.43 kB | 首屏保留 Taskbar，其余四个分区按需加载      |
| Settings API 共享块 | 118.68 kB |  94.97 kB | 动态分区形成更清晰的共享边界                |
| Bar 主 chunk        | 139.93 kB | 140.76 kB | 增加轻量播放事件和启动标记后基本持平        |
| 全局 CSS            |  98.55 kB |  98.55 kB | gzip 15.70 kB，当前不为扫描范围增加维护清单 |

Vite 单 chunk 警告预算设为 250 kB。当前按需分区为 1.22–13.27 kB，最大业务 chunk 为 Bar 的
140.76 kB。构建耗时主要位于 i18n 资源加载和 Tailwind 生成，但生产产物大小尚无继续改造证据。

## 封面传输

- 原始封面限制为 4 MiB，base64 最坏约增加三分之一体积。
- 标题/歌手/封面继续使用完整快照，保证同一歌曲数据原子更新。
- 播放状态和能力改用 `current-playback-state-changed` 轻量事件；播放/暂停不再重复传输封面。
- 时间轴继续使用独立轻量事件。
- JPEG/PNG/WebP 在 Rust 中提取主色；少见的 BMP/GIF 由 WebView 直接展示，主色回退系统强调色。
- 当前元数据 loader 已缓存最近快照且丢弃过期切歌结果，没有引入第二套封面缓存或 asset protocol。

## Rust 发布体积

此前仅启用 JPEG/PNG 解码器时，`cargo build --release` EXE 为 3,725,312 bytes；该数字不再代表
当前配置。恢复 WebP 后的发布体积留待下次完整 Release 构建重新记录。Release 继续使用 LTO、
单 codegen unit、`opt-level = "s"`、strip 和 panic abort。

Tauri `unstable` feature 必须保留：真正的任务栏 Child WebView 依赖 `WindowBuilder`、
`WebviewBuilder`、`get_window/get_webview` 和 `add_child`。Windows crate feature 均对应当前 WinRT、
DWM、GDI、COM、线程、DPI、UI Automation 或窗口 API。

`tauri build` 已生成并 patch 3,969,536-byte 发布 EXE，但 NSIS 3.11 下载连续两次发生网络全局超时，
因此本次无法提供安装包体积，不能把外部工具下载失败记为项目构建失败。
