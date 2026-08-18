# Muse Bar

Muse Bar 是一个面向 Windows 11 的任务栏媒体控制工具。首版会通过 Windows
系统媒体会话显示封面、歌曲名、歌手、播放状态和进度，并提供上一曲、下一曲、
播放/暂停与可用时的进度跳转。

当前仓库处于“阶段 0：开发基线”。任务栏嵌入和媒体功能会按照
[架构与数据流](docs/architecture.md) 中的边界逐步加入。

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

## 日常验证

```powershell
vp check
vp test
vp run type-check
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

`vp build` 只构建 Vue 前端。完整 Windows 应用和安装包使用：

```powershell
vp exec tauri build
```

当前发布目标是无需管理员权限的当前用户 NSIS 安装包。

## 文档

- [架构、目录与数据流](docs/architecture.md)
- [原始模板验证记录](docs/baseline.md)

## 分步开发约定

每一个实现步骤都遵循相同节奏：

1. 先说明该步骤解决的问题、涉及文件和数据流。
2. 只实现当前范围，不提前加入后续功能。
3. 运行与风险相称的自动检查，并给出可观察结果。
4. 独立提交，提交信息只描述这一小步。
