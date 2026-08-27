# Muse Bar

Muse Bar 是一个专为 Windows 11 设计的任务栏媒体控制工具。它读取 Windows系统媒体传输控件（SMTC）中的当前媒体会话，将封面、歌曲信息、播放进度和常用控制直接嵌入 Explorer 任务栏。

> 当前项目仅面向 Windows 11 x64，仍处于早期开发阶段。歌词区域目前使用占位文本验证
> 布局与交互，尚未接入真实歌词来源。

### 当前边界

- 歌词模式只显示固定占位文本，用于验证任务栏空白区域计算和悬停切换效果。
- Bar 上尚未提供拖动播放进度的交互；Rust 媒体控制层已经具备 seek 能力。
- 安装包使用 Tauri 更新签名，但尚未配置 Windows Authenticode 代码签名，因此 Windows SmartScreen 仍可能显示未知发布者提示。

## 技术栈

| 层级       | 技术                                                         |
| ---------- | ------------------------------------------------------------ |
| 桌面运行时 | Tauri 2、WebView2                                            |
| 原生后端   | Rust、Windows/WinRT API、Win32、UI Automation                |
| 前端       | Vue 3、TypeScript、Composition API、Pinia、Vue Router        |
| UI 与样式  | Tailwind CSS 4、shadcn-vue / Reka UI、Lucide、Motion for Vue |
| 国际化     | Vue I18n（当前界面以中文为主）                               |
| Web 工具链 | Vite+、Vite、Oxlint、Oxfmt、vue-tsc                          |
| 安装与更新 | NSIS、Tauri Updater、GitHub Releases                         |
| 发布       | release-it、GitHub Actions                                   |

## 系统要求

### 使用应用

- Windows 11 x64
- Microsoft Edge WebView2 Runtime

### 本地开发

- Windows 11 x64
- [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)，安装
  “使用 C++ 的桌面开发”工作负载
- Rust stable MSVC 工具链（项目声明的最低 Rust 版本为 `1.77.2`）
- [Vite+](https://viteplus.dev/guide/) 全局命令 `vp`
- Node.js `^22.21.0` 或 `>=24.12.0`
- pnpm `11.22.0`，由 Vite+ 按 `package.json` 的 `devEngines` 管理

如果尚未安装 Vite+，可在 PowerShell 中执行官方安装命令：

```powershell
irm https://vite.plus/ps1 | iex
```

## 快速开始

```powershell
git clone https://github.com/like0413/muse-bar.git
cd muse-bar
vp install
vp env doctor
vp exec tauri dev
```

## License

本项目基于 [MIT License](LICENSE) 开源。
