# Muse Bar

Muse Bar 是一个面向 Windows 11 的任务栏媒体控制工具。首版会通过 Windows
系统媒体会话显示封面、歌曲名、歌手、播放状态和进度，并提供上一曲、下一曲、
播放/暂停与可用时的进度跳转。

当前仓库已完成 Child 任务栏嵌入的技术闸门验证；透明渲染、鼠标交互、任务栏自动隐藏、
全屏行为和 Explorer 重启恢复均已通过。首版采用 Child 作为默认宿主，并保留 Owner
兼容回退。后续任务栏宿主和媒体功能会按照[架构与数据流](docs/architecture.md) 中的边界
逐步加入。

## 环境要求

- Windows 11 24H2 或更高版本
- Rust stable 与 MSVC Windows 工具链
- Vite+；Node.js 和 pnpm 版本由 Vite+ 根据 `package.json` 管理
- Microsoft Edge WebView2 Runtime

## 首次准备

```powershell
vp install
vp env doctor
```

## 启动

仅启动 Vue 前端预览：

```powershell
vp dev
```

启动完整 Tauri 桌面应用：

```powershell
vp exec tauri dev
```

Tauri 会依据 `src-tauri/tauri.conf.json` 自动启动前端开发服务器。

## 提交时自动校验

开发过程中不需要手动运行 lint、format 或 check。执行 Git 提交时，`vite.config.ts`
中的 staged hooks 会自动修复并检查前端代码，同时格式化和检查 Rust 代码。

`vp build` 只构建 Vue 前端。完整 Windows 应用和安装包使用：

```powershell
vp exec tauri build
```

当前发布目标是无需管理员权限的当前用户 NSIS 安装包。

## 文档

- [架构、目录与数据流](docs/architecture.md)
- [原始模板验证记录](docs/baseline.md)
- [Child 任务栏宿主技术闸门](docs/child-host-validation.md)

## 分步开发约定

每一个实现步骤都遵循相同节奏：

1. 先说明该步骤解决的问题、涉及文件和数据流。
2. 只实现当前范围，不提前加入后续功能。
3. 为项目自有函数说明用途；复杂逻辑额外解释设计原因，显而易见的简单逻辑不堆砌注释。
4. 提交前由用户运行当前功能；提交时由 staged hooks 自动执行代码检查。
5. 独立提交，提交信息只描述这一小步。
