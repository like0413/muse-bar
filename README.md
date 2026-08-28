![Muse Bar — 将媒体控制放进 Windows 11 任务栏](docs/assets/muse-bar-banner.png)

Muse Bar 是一个专为 Windows 11 设计的任务栏媒体控制工具。它读取 Windows系统媒体传输控件（SMTC）中的当前媒体会话，将封面、歌曲信息、播放进度、频谱和常用控制直接嵌入 Explorer 任务栏。

> 当前项目仅面向 Windows 11 x64，仍处于早期开发阶段。歌词区域目前使用占位文本验证
> 布局与交互，尚未接入真实歌词来源。

### 当前边界

- 歌词模式只显示固定占位文本，用于验证任务栏空白区域计算和悬停切换效果。
- Bar 上尚未提供拖动播放进度的交互；Rust 媒体控制层已经具备 seek 能力。
- 安装包使用 Tauri 更新签名，但尚未配置 Windows Authenticode 代码签名，因此 Windows SmartScreen 仍可能显示未知发布者提示。
