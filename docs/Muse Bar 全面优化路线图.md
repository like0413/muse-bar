# Muse Bar 全面优化路线图

---

# 一、实施优先级

本章是其余优化工作的控制面。后续改动以这里定义的顺序、阶段门槛和累计回归规则为准；
阶段编号表示依赖关系，不代表必须把某一阶段的所有长期事项一次完成。

## 1. 优先级判定规则

每个优化项必须同时记录优先级、收益、修改风险和成本：

| 优先级 | 判定标准                                         | 处理规则               |
| ------ | ------------------------------------------------ | ---------------------- |
| P0     | 数据丢失、任意代码执行、稳定复现的崩溃或死锁     | 停止当前阶段，立即处理 |
| P1     | 安全边界缺失、关键功能不可靠、质量门禁失效       | 当前阶段优先完成       |
| P2     | 明显维护性问题、可靠性缺口、可证实的中等性能问题 | 纳入对应阶段           |
| P3     | 命名、局部重复、轻微结构或格式问题               | 仅在不扩大改动时处理   |

同一优先级内按以下顺序排序：

1. 高收益、低风险、低成本。
2. 会阻塞后续阶段的基础设施。
3. 能为后续重构提供类型或编译保护的工作。
4. 有测量证据的性能问题。
5. 单纯代码风格问题。

不得仅因为文件较长、存在 `clone`、未使用设计模式或不符合某种目录模板就创建优化项。

### 证据状态

路线图中的事项必须先归入以下一种状态，只有 `Confirmed` 默认进入实施：

| 状态          | 含义                                     | 当前处理方式                       |
| ------------- | ---------------------------------------- | ---------------------------------- |
| `Confirmed`   | 已由代码、命令输出或可复现行为确认       | 按优先级实施并验证                 |
| `Verify`      | 存在与当前项目相关的合理疑点，但尚未确认 | 只做低成本、只读验证；无问题即结束 |
| `Conditional` | 仅在未来需求、规模或指标达到条件时有价值 | 当前不实施，记录触发条件           |
| `Keep`        | 当前设计清晰、简单且工作正常             | 明确保留，防止后续机械重构         |

通用最佳实践、审计清单和“可能更现代”的方案都不能直接视为项目问题。`Verify` 未发现证据时，
结论必须是“不建议修改”，不得为了完成路线图将其升级为优化项。

## 2. 当前基线

开始第二章前的已知状态：

| 检查                                                        | 当前结果 | 已知原因                                               |
| ----------------------------------------------------------- | -------- | ------------------------------------------------------ |
| `vp install`                                                | 通过     | 依赖已同步                                             |
| `vp run type-check`                                         | 通过     | Vue 和 TypeScript 可编译                               |
| `vp build`                                                  | 通过     | 前端生产构建成功                                       |
| `cargo check --manifest-path src-tauri/Cargo.toml --locked` | 通过     | Rust 可编译                                            |
| `vp check`                                                  | 通过     | `.agents/**` 已排除，产品文件格式、lint 和类型检查通过 |
| Rust Clippy `-D warnings`                                   | 通过     | 动画调用已改用具名 request 结构体                      |

每完成一章，都在对应实施记录中更新实际命令结果；不得把“命令未运行”写成“通过”。

## 3. 累计优化规则

1. 每批只处理一个可独立描述的问题或一组强相关问题。
2. 修改前记录问题、范围、预期收益、风险和验证方法。
3. 保持用户可见行为不变，除非当前任务明确要求改变行为。
4. 当前阶段不得破坏此前阶段已经建立的安全、类型、可靠性和架构约束。
5. 发现前序阶段回归时，暂停新增工作并先修复回归。
6. 性能修改必须记录指标、测量场景和 Before/After；没有证据时只建立基线。
7. 大模块拆分先移动纯逻辑，保持 IPC 名称、DTO 和事件语义不变。
8. 不顺手修改当前批次以外的代码，不进行大爆炸式重构。
9. 每批结束检查工作树，只交付本批次有意产生的文件变化。
10. Windows 行为无法自动验证时，明确列入人工验收，不以静态检查代替。
11. `Verify` 项只允许低成本验证；没有发现实际问题时不得修改代码或依赖。
12. `Conditional` 项未满足触发条件时不得提前实现。

## 4. 章节执行顺序与里程碑

后续优化以“章”为唯一执行单位。上一章完成并通过累计回归后，才进入下一章：

```text
第一章 实施优先级
  → 第二章 工程化与依赖治理
    → 第三章 安全
      → 第四章 TypeScript 与运行时验证
        → 第五章 IPC 与 Tauri Command
          → 第六章 可靠性与恢复能力
            → 第七章 测试体系（按项目约束跳过）
              → 第八章 架构与 SOLID
                → 第九章 Rust 模块和代码质量
                  → 第十章 状态管理与数据一致性
                    → 第十一章 Vue 组件与前端结构
                      → 第十二章 性能优化
                        → 第十三章 文档与可维护性
```

Phase 不再决定下一项工作，只用于汇总跨章节成果：

| 里程碑         | 包含章节           | 达成标志                                  |
| -------------- | ------------------ | ----------------------------------------- |
| 质量与安全基线 | 第二章、第三章     | 质量门禁可信，最小权限和 CSP 生效         |
| IPC 契约稳定   | 第四章、第五章     | DTO、运行时验证、命令、事件和错误语义稳定 |
| 可靠性保护网   | 第六章             | 恢复路径明确，关键系统场景有人工验收项    |
| 架构边界稳定   | 第八章、第九章     | SOLID 边界明确，Rust 复杂模块可维护       |
| 前端边界稳定   | 第十章、第十一章   | 状态所有权、组件契约和用户状态清晰        |
| 性能与交付收尾 | 第十二章、第十三章 | 性能有测量证据，文档与当前实现一致        |

章节内部仍采用小批次交付；不得为了完成里程碑而同时修改多个尚未轮到的章节。

## 5. 每批优化记录模板

后续每批工作使用以下最小记录：

```text
优化项 ID / 章节：
证据状态：Confirmed / Verify / Conditional / Keep
问题与证据：
涉及文件：
本批范围：
明确不处理：
预期收益：
修改风险：
验证命令：
Windows 人工验收：
结果与残留风险：
```

## 6. 章节状态

| 顺序 | 章节                            | 状态   | 前置条件     | 下一动作     |
| ---: | ------------------------------- | ------ | ------------ | ------------ |
|    1 | 第一章：实施优先级              | 已完成 | 无           | 已进入第二章 |
|    2 | 第二章：工程化与依赖治理        | 已完成 | 第一章完成   | 已进入第三章 |
|    3 | 第三章：安全                    | 已完成 | 第二章完成   | 已进入第四章 |
|    4 | 第四章：TypeScript 与运行时验证 | 已完成 | 第三章完成   | 已进入第五章 |
|    5 | 第五章：IPC 与 Tauri Command    | 已完成 | 第四章完成   | 已进入第六章 |
|    6 | 第六章：可靠性与恢复能力        | 已完成 | 第五章完成   | 已完成       |
|    7 | 第七章：测试体系                | 已跳过 | 第六章完成   | 不实施       |
|    8 | 第八章：架构与 SOLID            | 已完成 | 第六章完成   | 已完成       |
|    9 | 第九章：Rust 模块和代码质量     | 未开始 | 第八章完成   | 下一章       |
|   10 | 第十章：状态管理与数据一致性    | 未开始 | 第九章完成   | 等待第九章   |
|   11 | 第十一章：Vue 组件与前端结构    | 未开始 | 第十章完成   | 等待第十章   |
|   12 | 第十二章：性能优化              | 未开始 | 第十一章完成 | 等待第十一章 |
|   13 | 第十三章：文档与可维护性        | 未开始 | 第十二章完成 | 最终文档同步 |

---

# 二、工程化与依赖治理

## 7. 质量门禁

本节只处理已经由命令确认的问题。

`Confirmed`：

- `vp check` 当前被 `.agents/**` 等非产品文件的格式问题阻断。
- 严格 Clippy 当前被 `animate_window_width` 参数过多阻断。

`Keep`：

- `vp install` 已通过，依赖处于同步状态。
- `vp run type-check`、`vp build` 和 Cargo check 已通过。

本章目标命令：

```
vp install
vp check
vp run type-check
vp build
cargo check --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
```

完成条件是上述命令稳定通过，且检查命令不修改生成文件。

实施结果：

- `vp check`、类型检查和生产构建通过。
- Cargo check、Rustfmt check 和严格 Clippy 通过。
- `.agents/**` 已退出产品格式检查；产品源码规则没有放宽。
- `components.d.ts` 已按当前组件扫描配置重新生成，移除不再由自动导入插件管理的旧 UI 声明。
- `animate_window_width` 改用具名 request 结构体，两个调用点的窗口、布局和版本字段保持原值。

## 8. 检查范围

`Confirmed`：当前格式检查范围包含不属于产品质量门禁的 `.agents/**`。

修复时按以下边界分类：

- 项目源码：必须格式化和 lint。
- 生成文件：不格式化，但验证是否同步。
- `.agents` 技能资料：不纳入产品检查。
- `dist`、`target`：忽略。
- shadcn-vue 生成源码：保留类型检查；是否执行全部格式规则以实际生成方式为准。

不扩大到编辑技能文档、依赖源码或生成产物内容。

## 9. 依赖治理

本节当前状态为 `Verify`，不是依赖替换任务。

只执行与当前变更直接相关的低成本验证：

- lockfile 能否稳定安装。
- manifest 中的依赖是否能在源码、构建配置或生成流程中找到明确用途。
- Cargo feature 是否对应当前实际调用的平台 API 或格式支持。
- 当前构建是否报告重复包、废弃 API 或已知安全问题。

处理规则：

- 找到明确未使用的直接依赖时，才创建独立删除任务并验证构建与运行行为。
- 只有存在安全公告、兼容性阻塞或可测量体积成本时，才评估升级、替换或关闭 feature。
- 不逐个调查所有依赖是否“有更轻方案”或是否停止维护。
- 不因为源码目录中存在未导入的 shadcn-vue 组件就认定其进入 bundle。
- 验证未发现问题时记录“不建议修改”，本节结束。

本次验证结果：lockfile 安装、前端构建和 Cargo 构建均正常，没有产生值得删除、升级或替换依赖的
证据，因此本章不修改依赖集合或 feature。

## 10. 版本一致性

本节当前状态为 `Verify / Keep`。

当前 `package.json`、`Cargo.toml` 和 `tauri.conf.json` 的应用版本均为 `0.1.0`，保持不变。
仅在准备发布时校验：

- 安装包
- Git tag

没有发布任务时不为版本同步增加脚本或抽象。

## 11. CI

本节当前状态为 `Conditional`。当前没有证据表明缺少 CI 正在造成回归，也没有在本章提前建立
完整发布流水线。

触发条件：

- 项目开始多人协作或接受外部贡献。
- 建立远程仓库合并门禁。
- 开始稳定发布安装包。
- 手工检查遗漏已经造成实际回归。

触发后先建立最小 Windows CI，只运行当时已经稳定的安装、类型检查、构建、Cargo check 和
Clippy；bundle smoke build、产物大小趋势和发布自动化仍需单独证明收益。

本章结束时触发条件仍未满足，因此不新增 CI 配置。

---

# 三、安全

## 12. CSP

本节状态为 `Confirmed`。原配置将 `csp` 设为 `null`，全局 CSS 还会请求 Google Fonts。

实施结果：

- 生产 CSP 默认仅允许同源资源、Tauri IPC、data/blob 图片和本地字体。
- 开发 CSP 只额外允许固定的 `localhost:1420` HTTP/WebSocket 开发连接。
- 禁止 object、frame、base URL 和表单提交。
- 保留 `style-src 'unsafe-inline'`，因为现有组件和 Vite 开发模式需要运行时样式；未开放 inline script。
- 移除 Google Fonts 网络请求，继续使用现有系统字体栈。

## 13. Capability 最小权限

本节状态为 `Confirmed`。`core:default` 会同时授予 path、event、window、webview、app、image、
resources、menu 和 tray 的默认权限，超过当前前端实际需要。

实施结果：

- Bar 和 Settings 均只保留 `core:event:allow-listen` 与 `core:event:allow-unlisten`。
- 未开放 shell、filesystem、process、HTTP、clipboard 或插件权限。
- 自定义 command 的窗口级访问控制留到第五章统一处理，避免在本章提前改变 IPC 契约。

## 14. 路径安全

本节状态为 `Keep / Conditional`。当前前端没有向 Rust 传入任意文件路径；设置和日志路径均由
Tauri 应用目录 API 生成，因此本章不修改现有路径逻辑。

如果未来新增用户可选路径，再单独处理 `..`、UNC、symlink、绝对路径逃逸和规范化后复验。

## 15. 外部命令

本节状态为 `Keep`。当前只调用固定的 `cmd.exe /D /C ver` 和 `explorer.exe`，参数不接受前端
命令片段；日志目录也由 Tauri 应用目录 API 生成，没有发现命令注入路径。

改用另一套 Windows 版本 API 没有已证实收益，本章不替换现有实现。

## 16. 数据与日志泄漏

本节状态为 `Verify`。扫描未发现 API key、密码、私钥、完整封面数据或设置文件内容写入日志。
媒体事件 token 是 Windows 事件注销句柄，不是认证凭据。

诊断页仍会显示任务栏句柄、进程 ID 和应用日志目录等本机信息，但数据不发送到外部；若未来增加
诊断导出或上传，再建立脱敏规则和用户确认流程。

## 17. 更新供应链

本节状态为 `Conditional`。当前没有 updater 插件、更新端点或自动更新流程，因此不提前建设签名、
回滚或密钥管理。

添加 updater 时必须同时要求 HTTPS、签名验证、固定来源、失败回滚，并确保私钥不进入仓库或客户端。

---

# 四、TypeScript 与运行时数据验证

## 18. 完整 IPC 类型

状态：`Confirmed`，已完成。

`SettingsPayload = Record<string, unknown>` 已替换为与 Rust `AppSettings` 的 camelCase 序列化结果
逐字段对应的接口，覆盖配置版本、窗口、任务栏、外观、歌词和开机启动设置。设置页使用独立的
`SettingsPatch` 提交局部改动，并从可修改字段中排除只应由 Rust 迁移逻辑维护的
`schemaVersion`。

同步审计结果：

- `MediaSnapshot`、`CurrentTimeline`、`RuntimeInfo`、任务栏诊断和 Bar 宽度测量已有明确接口。
- 媒体控制错误与复杂事件载荷已有明确 union 或接口。
- 没有发现仍以 `Record<string, unknown>`、`unknown[]` 或 `any` 代替已知 IPC DTO 的同类问题。

## 19. 建立统一命令名称

状态：`Keep / Conditional`，当前不修改。

命令字符串已按领域集中在各 API 模块中，调用点没有重复散落。仅在 TypeScript 中增加
`IPC_COMMANDS` 并不能消除与 Rust 手工同步的问题，反而增加一次间接跳转。若后续引入跨语言
绑定生成器，或同一命令名开始在多个模块重复，再建立 Rust/TypeScript 共享的生成来源。

事件名同样已封装在对应 API 模块内；保持当前局部常量。

## 20. 运行时验证

状态：`Keep / Conditional`，当前不增加前端 schema。

当前数据边界不是“任意 JSON 直接进入 Vue”：设置文件先由 Rust `serde` 反序列化，再执行版本迁移、
数值范围归一化、显示器标识清理、颜色校验和损坏文件回退，IPC 只序列化已构造成功的
`AppSettings`。前端读取函数仍保留安全回退，用于兼容缺失或无法识别的值。

因此，再在前端手写一份完整 schema 会复制 Rust 规则并形成漂移风险。仅在以下条件之一出现时实施：

- IPC 开始接收网络、插件或其他不可信来源的数据。
- 某个复杂载荷出现实际的反序列化或版本漂移故障。
- 项目引入能从 Rust 类型生成 TypeScript 和运行时 schema 的单一来源工具。

## 21. 枚举和穷尽检查

状态：`Verify → Keep`。

已检查 `ControlAction`、`MediaPlayerKind`、`CurrentPlaybackStatus`、`ProgressStyle`、
`WindowMode` 和错误码的消费者。当前没有对这些 union 进行分支映射的 `switch`，主要是相等判断、
透传和声明式选项，不存在会因 `default` 分支而静默吞掉新成员的路径。当前不添加无用途的
`assertNever`；未来出现全量分支映射时再要求穷尽检查。

## 22. Readonly 契约

状态：`Confirmed`，设置边界已完成；其余保持现状。

`SettingsPayload` 的全部字段已标记为 `readonly`。设置 Store 不原地修改快照，只通过
`saveSettingsPatch` 合并新对象，并用 Rust 返回的规范化完整设置替换旧快照，符合当前数据流。

没有证据表明媒体事件数组或其他 Store 状态发生组件侧误改，因此不在本章机械扩散深层
`Readonly`、只读数组或额外 composable 包装；若未来出现共享状态被绕过 action 修改的实例，
再在对应边界收紧。

## 本章验收

2026-08-25 已完成累计验证：

- `vp install`：通过，依赖已是最新状态。
- `vp run type-check`：通过，完整 DTO 与全部调用点类型一致。
- `vp check`：通过，342 个文件格式正确，324 个文件无 lint 或类型错误。
- `vp build`：通过，前端生产构建成功。
- `cargo check --manifest-path src-tauri/Cargo.toml --locked`：通过。
- `cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets -- -D warnings`：通过。

本章没有启动开发服务。

---

---

# 五、IPC 与 Tauri Command

## 23. Command 薄边界

状态：`Confirmed`，已完成。

已逐一核对全部 Command、`generate_handler!` 注册项和前端 `invoke`：

- 媒体选择、缓存、Win32 枚举和设置文件读写均已位于领域模块或共享状态中。
- `report_bar_content_width` 原先直接编排任务栏读取、宽度策略和窗口动画，现已下沉至
  `bar_layout::apply_content_width`，Command 只负责接收 Tauri 参数并转发。
- `update_settings` 原先直接编排启动项同步、持久化、回滚、广播和跨显示器恢复，现已下沉至
  `settings_update::apply`，事务语义保持不变。
- 诊断 Command 保留 DTO 转换；它属于 IPC 边界职责，没有为了追求一行函数继续拆分。

清理后保留 18 个实际使用的 Command。AppManifest、`generate_handler!` 和前端调用名称完全一致。

## 24. IPC 调用频率

状态：`Verify → Keep`。

调用路径审计未发现每帧、循环内 N 次或逐字段 invoke：

- Bar 宽度变化由 `requestAnimationFrame` 合并，并忽略小于 0.5px 的重复值。
- 时间轴使用不含封面数据的轻量事件，统一快照只在媒体状态变化时更新。
- 设置页把局部补丁合并为一次完整设置更新，Rust 返回规范化结果。
- 诊断只在设置页启动或用户刷新时读取，并行合并相关请求。
- 媒体控制只在用户点击时调用，并阻止同一会话的并发操作。

当前没有性能故障或调用频率指标支持改用 Channel、额外节流或批处理，保持现状。

## 25. Event 体系整理

状态：`Confirmed`，已完成。

已删除没有任何前端监听者的 4 类历史广播：

- `media-sessions-changed`
- `current-media-metadata-changed`
- `current-playback-status-changed`
- `current-playback-capabilities-changed`

同时删除对应前端监听封装。会话身份事件仅供设置诊断使用，已从应用全局广播改为只发送给
`settings` 窗口。

保留的事件均有明确消费者：统一媒体快照、轻量时间轴、媒体会话身份、媒体活动记录和设置变化。
时间轴刻意独立于统一快照，避免进度变化时重复传输封面和完整元数据，不再合并。

## 26. IPC 错误模型

状态：`Verify → Keep / Conditional`。

媒体控制已经返回包含 `action`、稳定 `code` 和 `message` 的结构化错误，因为该操作确实存在
`noSession`、`unsupported`、`rejected` 和 `windowsApi` 等可区分语义。其余 Command 的错误目前
只用于向用户显示或写入诊断，没有依据错误类别执行重试、跳转或按钮状态决策。

因此不把所有底层 `String` 机械包装成字段内容相同的通用对象，也不额外引入 `thiserror` 层级。
触发条件是某个消费者需要根据错误类别采取不同恢复动作；届时在对应领域定义稳定错误 enum，
而不是建立无法表达领域语义的单一“大而全”错误码表。

## 27. Command 访问范围

状态：`Confirmed`，已完成。

Tauri 默认允许所有应用窗口调用 `invoke_handler` 注册的自定义 Command。现已在 `build.rs` 使用
`AppManifest::commands` 为 18 个应用命令生成独立的 `allow-*` 权限，并按窗口写入 capability：

- Bar 可调用宽度和显隐、媒体控制/快照、会话选择、读取设置以及打开设置窗口。
- Settings 可调用设置读写、运行信息、任务栏诊断、媒体诊断快照以及显示设置窗口。
- Bar 不再具备诊断、设置写入或日志目录权限；Settings 不具备媒体控制、Bar 窗口操作或打开新设置
  窗口的权限。

已生成的 Tauri ACL manifest 和 capability schema 能解析全部权限标识，没有未授权注册项。

## 本章验收

2026-08-25 已完成累计验证：

- `vp install`：通过，依赖已是最新状态。
- `vp run type-check`：通过，删除旧前端 IPC 封装后无调用缺口。
- `vp check`：通过，格式、lint 和 TypeScript 检查无错误。
- `vp build`：通过，前端生产构建成功。
- `cargo check --manifest-path src-tauri/Cargo.toml --locked`：通过。
- `cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets -- -D warnings`：通过。
- `vp exec tauri dev`：Bar 正常启动，无 ACL 拒绝、Command not found 或事件权限错误。
- 再次启动应用触发单实例设置窗口：第二实例以 0 退出，Settings 首次读取无权限错误。

运行验证后已停止本章启动的 Tauri、Vite 和 Muse Bar 进程；端口 1420 空闲。

---

---

# 六、可靠性和恢复能力

## 28. 设置持久化

状态：`Confirmed`，已完成。

保留同目录“临时文件 + `sync_all` + `rename`”提交方式。它避免直接覆盖正式文件时因崩溃、断电或
写入中断留下半份 JSON；临时文件使用时间、进程和单调序号生成唯一名称，并通过 `create_new`
防止意外覆盖。替换完成后重新读取、反序列化并与待保存值比较，只有持久化验证通过后调用方才更新
内存设置。

配置冲突规则已经明确：

1. 用户已持久化的字段优先于代码默认值，升级应用不会无条件重置用户选择。
2. 新增字段由 `serde(default)` 补齐，不再仅因增加字段或修改新用户默认值而提升 schema。
3. 只有字段含义、格式或约束发生真实不兼容变化时才提升 schema，并编写针对性迁移；当前兼容
   基线继续使用版本 12，不再为普通功能修改版本常量。
4. 旧 schema 加载后迁移到当前版本；写回失败会记录日志，本次运行仍使用已迁移的内存值。
5. 高于当前版本的 schema 会明确拒绝加载且不移动、不改写文件，防止旧版应用丢弃新字段。

实施结果：

- Windows 已有目标文件可以原子替换；目标文件被独占时保存返回错误，原文件保持不变。
- 保存成功、替换失败以及下次启动都会清理本应用命名范围内的临时文件。
- 无效 JSON 会移动为独立损坏备份，最多保留最近 3 份；备份失败时不再假装恢复成功。
- 设置持久化改用 `SettingsPersistenceError` 保留 I/O、JSON、路径、未来版本和校验错误类别。
- 迁移写回失败不再被静默忽略；诊断日志保持原有调试构建注册时序，避免扩大启动生命周期改动。
- 已有文件替换、文件占用、临时文件清理、缺失字段默认值、旧版本写回、未来版本保护、损坏文件
  备份和备份数量上限均由同一持久化边界集中处理，发布前按故障注入清单人工验收。

启动回归修复：设置持久化改造曾扩大日志插件的注册时序，暴露了 Bar 原生窗口先显示、媒体初始快照
暂时为空后再隐藏的竞态。现已恢复原有日志注册时序；任务栏挂载按服务端可见状态决定是否显示，位置
稳定化不再隐式重新显示窗口；前端先选择并绑定媒体会话再读取快照；已绑定会话的元数据缓存为空时会
主动重试，避免冷启动失败后永久保持空快照。

磁盘耗尽和真实用户 ACL 拒绝需要破坏性系统环境；两者与文件占用共用同一 I/O 错误传播、临时文件
清理以及“保存失败不替换内存设置”路径，发布前在隔离的 Windows 用户中执行故障注入。

## 29. Explorer 恢复

状态：`Confirmed`，已完成实现，破坏性系统场景保留发布前人工验证。

- `TaskbarCreated`、设置变化、宽度应用和健康检查统一进入容量为 1 的恢复通道，连续通知只保留一个
  待执行请求；单次恢复最多重试 30 次，每次间隔 100 ms。
- 旧 Child 句柄失效时先销毁 Tauri 窗口和 WebView 标签，再在后续重试中创建，避免复用 Explorer
  已销毁或系统已重新分配的 HWND。
- 任务栏布局线程每 500 ms 检查 Bar 是否缺失、失效或脱离当前目标任务栏；异常持续时最多每 2 秒
  补交一次恢复请求。自动隐藏任务栏重新展开后，不再依赖新的 `TaskbarCreated` 才恢复。
- 恢复前清空 Explorer XAML 定位缓存；多显示器、DPI、目标位置和任务栏范围变化仍通过完整身份与
  矩形重新计算，不复用旧 Explorer 布局。
- 恢复前后都读取服务端最终显隐状态。恢复期间切换 Bar、媒体消失或用户隐藏，不会被旧请求重新显示。
- 三个 Explorer 线程共享停止信号；退出时唤醒恢复通道、向隐藏消息窗口发送 `WM_CLOSE`，重试循环
  可以提前终止。线程退出等待上限为 1 秒，系统 API 卡住时由进程结束回收，避免退出永久挂起。

连续终止 Explorer、切换自动隐藏以及真实多屏 DPI 变化会影响桌面环境，不自动执行；发布前按原
验证矩阵人工故障注入，并同时确认恢复请求首次触发、冷却合并和冷却后重试行为。

## 30. 媒体服务恢复

状态：`Confirmed`，已完成。

- WinRT manager、活动跟踪器、两个全局事件订阅和当前会话绑定改为事务式运行时；初始化未完整成功
  就不会留下半套订阅。首次失败不阻止应用启动，后续媒体 IPC 按 2 秒退避自动重建。
- 运行时读取失败会丢弃旧管理器、注销事件并清空会话与元数据缓存；下一次调用重新初始化，避免一直
  沿用已经失效的 Windows 媒体服务对象。
- 播放器退出后刷新活动会话，只保留 Windows 最新列表中仍存在的记录；晚到事件找不到记录时直接
  忽略。切换会话先注销旧 token，并用会话身份和元数据版本拒绝过期结果。
- 元数据任务改为单槽“最新值邮箱”，连续快速切歌只保留最新读取，不再让请求无限积压；同一会话
  缓存为空时可主动重试。
- 封面压缩流最大 4 MiB，解码尺寸最大 `4096×4096`，解码分配预算 64 MiB；损坏或超限封面退回
  空封面，但保留同批标题和歌手。标题、歌手最多缓存 4096 个 Unicode 字符。
- 时间轴继续校验有效起止范围，前端以会话、标题、歌手和更新时间拒绝切歌后晚到的旧时间轴；同名
  歌曲在切换播放器时仍由会话标识区分。
- 播放器不支持或拒绝控制继续返回稳定错误类别；媒体读取位于后台线程。退出等待有上限，因此播放器
  或 WinRT 调用卡死不会无限阻止应用结束。

媒体运行时退避、空缓存重试、支持播放器选择、损坏封面和 Unicode 文本上限均保留明确边界。真实
播放器退出、拒绝控制和 WinRT 服务故障需要外部进程配合，发布前使用四个目标播放器执行人工矩阵。

## 31. 后台任务生命周期

状态：`Confirmed`，已完成。

- Explorer 恢复、任务栏布局和 Windows 消息线程由唯一 `ExplorerMonitor` 所有；`OnceLock` 阻止
  进程内重复启动，原子停止状态负责 cancel，`WM_CLOSE` 和通道唤醒负责 shutdown。
- 媒体元数据线程由 `MediaMetadataLoader` 所有，单槽邮箱同时承担合并、唤醒和 shutdown；版本号
  取消在途旧结果。
- 媒体活动线程由 `MediaActivityTracker` 所有，事件队列最大 64 项；跟踪器析构时先注销会话事件，
  再提交停止消息并释放全部发送端。
- XAML UI Automation 线程是同步有界任务，创建后立即 `join`；宽度动画和位置稳定线程分别由固定
  步数和固定延迟自然结束，并用动画版本号取消旧提交，不建设长期 worker 管理器。
- 应用 `RunEvent::Exit` 会先停止媒体运行时和 Explorer monitor。公共退出等待统一上限 1 秒；正常
  路径回收线程，外部 API 永久阻塞时记录日志并让进程退出兜底。
- Explorer 监听属于进程级基础设施，不在同一进程内重启；媒体运行时属于可恢复依赖，失败后按退避
  重新创建。两者的 restart 语义不再混为一谈。

后台线程的正常 join 和超时放弃等待统一由同一个有界等待函数处理。Vue 两个页面继续由 Pinia store
唯一持有监听器，页面卸载统一调用 `stop()`；异步晚到的监听器会立即自行注销，没有新增组件级全局订阅。

## 32. 缓存治理

状态：`Confirmed`，已完成。

- 任务栏定位缓存：最多 1 份布局；仅在任务栏 HWND 和矩形完全一致时使用，Explorer 恢复、应用退出
  或身份变化时失效。
- 媒体元数据缓存：最多 1 份完整快照，待处理任务最多 1 项；会话切换、媒体运行时重建和读取失败时
  清空。封面 data URL 受 4 MiB 输入上限约束，解码临时内存受 64 MiB 预算约束。
- 媒体活动缓存：条目数不超过 Windows 当前活动会话数，会话列表变化时 `retain` 删除退出播放器；
  队列最多 64 项，标题和歌手各不超过 4096 个字符。
- 应用设置、Bar 宽度测量和每个前端窗口的媒体快照都是单值状态；设置保存失败不替换内存值，窗口
  卸载释放前端快照和事件监听。

这些缓存均为“当前状态”或“最后一次可信布局”，不是按 key 长期累计的历史数据，因此 LRU 不会带来
额外收益，本章不引入缓存库。颜色桶是封面解码期间的固定 4096 项栈内临时数据，不属于长期缓存。

## 本章验收

2026-08-26 已完成：

- `vp check`：360 个文件格式通过，324 个文件无 lint、类型或格式错误。
- `cargo check --locked`：Rust 生产代码编译通过。
- `cargo clippy --locked --all-targets --all-features -- -D warnings`：通过。
- `vp build`：生产前端构建通过。
- 受控开发启动无运行时错误；无媒体时 Bar 已挂载但保持原生隐藏。验证结束后已关闭本次启动的应用、
  Vite+ 和 Tauri 进程，`muse-bar.exe` 与 1420 监听均无残留。

Explorer 连续终止、真实自动隐藏切换、多显示器 DPI 热变更、WinRT 服务故障和四家播放器拒绝控制属于
外部系统故障注入，不修改普通开发机状态；发布候选版本按第 29、30 节矩阵人工执行。

---

---

# 七、测试体系

状态：`Skipped`。

按项目约束，本项目不维护 Rust、Vue 或组件自动化测试代码，本章不实施。仓库中原有的 27 项 Rust
测试已删除，前端原本没有测试文件。质量门槛继续由类型检查、格式检查、严格 Clippy、生产构建以及
各功能章节和发布清单中的 Windows 人工验收共同承担；第八章直接以前六章约束为基础继续实施。

---

---

# 八、架构与 SOLID

状态：`Confirmed`，已完成。

本章按真实依赖和变化原因应用 SOLID，不以文件行数、模式数量或 trait 数量作为完成指标。Command
名称、事件名、ACL、IPC DTO 字段、设置持久化语义和 Windows 恢复策略保持不变。

## 1. 单一职责 SRP

已完成四处有证据的职责拆分：

| 原边界                          | 已提取职责                                                                                  | 保留职责                                |
| ------------------------------- | ------------------------------------------------------------------------------------------- | --------------------------------------- |
| `system_media.rs`               | `media_model` 的 DTO/播放器识别，`media_artwork` 的封面与颜色，`media_selection` 的选择策略 | WinRT 运行时、订阅、会话绑定和事件协调  |
| `media_activity.rs`             | 选择优先级只接收 `MediaSelectionCandidate`                                                  | Windows 活动采集、记录和事件发布        |
| `taskbar_occupancy.rs`          | `taskbar_layout` 的矩形与 Bar 布局决策                                                      | UI Automation、Win32 回退和可信布局缓存 |
| `AppearanceSettingsSection.vue` | 5 张设置卡和 1 个纯预览组件                                                                 | Store 连接与 `SettingsPatch` 转发       |

拆分后 `system_media.rs` 从 1438 行降为 1074 行，`taskbar_occupancy.rs` 从 658 行降为
409 行，外观入口组件从 496 行降为 52 行。行数只是拆分结果，边界依据分别是系统采集、策略决策、
数据契约和 UI 展示的不同变化原因。

`settings-store.ts` 状态为 `Verify → Keep`：它虽然同时读取多个设置页数据源，但当前唯一职责是
拥有设置窗口的启动、异步监听器注册和统一销毁。拆成多个 Store 会让监听器生命周期和页面激活状态
出现多个所有者；状态领域拆分留到第十章在完整状态所有权审计后决定。

## 2. 开闭原则 OCP

状态：`Confirmed / Keep / Conditional`。

- 播放器识别从运行时流程提取为 `PLAYER_IDENTIFICATION_RULES` 策略表。增加播放器时集中增加枚举
  映射和一条识别规则，不修改 WinRT 会话协调流程。
- 播放/暂停优先级从活动跟踪器提取到 `media_selection`，策略只消费普通候选 DTO；活动来源变化不
  影响选择算法，选择规则变化也不触碰事件订阅。
- 任务栏中央按钮识别、可用区域和 Bar 横坐标统一位于 `taskbar_layout`；更换采集来源时仍可复用
  相同布局策略。
- 设置迁移保持 `Conditional`：当前不存在多个真实迁移步骤，普通新增字段仍由 `serde(default)`
  处理。只有出现第二个不兼容结构迁移时才建立按版本顺序执行的迁移表。
- 进度颜色来源和窗口宿主模式目前都是有限产品枚举，没有运行时插件需求；保留显式 `match`，不为
  假设中的扩展点增加 trait 或注册器。

## 3. 里氏替换 LSP

状态：`Verify → Keep`。

当前项目没有承担同一契约的多实现 trait、继承层级或运行时可替换对象，因此没有可证实的替换违规。
本章不创建只拥有一个实现的 trait。未来确实出现第二种媒体源或任务栏采集器时，才要求各实现遵守
相同输入前置条件、错误语义和返回值保证，并在调用方需要运行时替换时选择动态分发。

## 4. 接口隔离 ISP

状态：`Confirmed`，已完成。

原 `media-api.ts` 同时暴露 DTO、查询、控制、事件和设置页诊断，现已删除并拆为：

| 接口面                     | 消费者与职责                             |
| -------------------------- | ---------------------------------------- |
| `media-types.ts`           | Bar、Settings 共享的只读 TypeScript 契约 |
| `media-query-api.ts`       | 当前快照和会话选择查询                   |
| `media-control-api.ts`     | 用户媒体控制动作                         |
| `media-event-api.ts`       | 快照、时间轴、会话和活动事件订阅         |
| `media-diagnostics-api.ts` | 设置页会话身份与活动记录诊断             |

Bar 控制组件不再导入诊断或事件能力，设置 Store 不再导入控制能力。外观子组件只接收所需 props，并
通过类型化 `change(SettingsPatch)` 事件请求保存；只有连接容器能访问完整 Settings Store。
Rust Command 的 18 个注册名和按窗口 capability 未变化，继续沿用第五章的最小权限接口面。

## 5. 依赖倒置 DIP

状态：`Confirmed`，已完成到当前需求需要的程度。

- `media_selection` 依赖 `MediaSelectionCandidate` 和 `MediaPlayerKind`，不依赖 WinRT session。
- `taskbar_layout` 依赖项目普通矩形、任务栏矩形和位置枚举，不依赖 `HWND`、UI Automation 或 COM。
- `media_model` 集中 Rust 到 TypeScript 的可序列化媒体 DTO，不引用 Tauri handle 或 Windows 类型。
- Windows adapter/采集模块负责把系统对象转换为普通数据；Command 继续只转发到应用逻辑。

项目当前没有数据库、远程服务或多后端实现，因此不引入 repository/service trait、`Box<dyn Trait>`
或依赖注入容器。模块边界和静态分发已经提供所需解耦。

## 6. 模块依赖规则

当前依赖方向已经写入 `docs/architecture.md`：

```text
Vue Component
  -> Store / typed feature API
  -> Tauri command
  -> runtime coordinator
  -> Windows acquisition adapter

plain DTO / selection / layout policy <- coordinator or adapter
```

静态审计结果：

- `src/pages/**` 没有直接导入 `invoke`；IPC 调用集中在 `src/lib/*-api.ts`。
- 旧 `media-api.ts` 已删除且没有残留引用。
- 外观设置只有连接容器导入 Settings Store，6 个子组件均使用 typed props/emits。
- `media_model`、`media_selection` 和 `taskbar_layout` 不含 WinRT、COM、`HWND` 或 Tauri handle。
- Command 注册、事件常量和 capability 文件没有因本章拆分变化。

## 本章验收

2026-08-26 已完成：

- `vp check`：前端格式、lint 和 TypeScript 检查通过。
- `cargo fmt --check` 与 `cargo check --locked`：Rust 格式和生产代码编译通过。
- `cargo clippy --locked --all-targets --all-features -- -D warnings`：严格 Clippy 通过。
- `vp build`：前端生产构建通过。
- `vp exec tauri dev`：Bar 正常启动；第二实例正常唤醒 Settings，未出现 Command not found、ACL
  拒绝、事件订阅或 Vue 运行时错误。
- 验证结束后已关闭本章启动的 Muse Bar、Tauri 和 Vite+ 进程；端口 1420 空闲。

本章没有新增或运行自动化测试代码。Explorer 故障注入、多显示器 DPI 和自动隐藏任务栏等系统行为
沿用第六章人工矩阵，本章未改变对应采集或恢复语义。

---

---

# 九、Rust 模块和代码质量

## 38. 拆分超大模块

优先级：

1. `system_media.rs`
2. `media_activity.rs`
3. `taskbar_occupancy.rs`
4. `child_host.rs`
5. `explorer_monitor.rs`

拆分时保持 public API 不变。

## 39. 错误体系

目前大量使用 `Result<T, String>`，短期可用，但长期会丢失错误类别。

建议：

- 模块内部使用 `thiserror` 枚举。
- IPC 边界转换为可序列化错误。
- 日志保留底层上下文。
- 用户消息避免直接暴露 Windows/HRESULT 技术细节。

## 40. `Box<dyn Error>` 收敛

状态：由第 28 节的直接依赖提前完成。

设置模块已用 `SettingsPersistenceError` 替换 `Box<dyn Error>`，区分 I/O、JSON、应用路径、缺少父目录、
未来 schema、无效 schema 和写后校验失败。读取路径据此只恢复真正损坏的配置；未来版本保持原文件并
停止加载，普通 I/O 与备份失败继续向上传播。第九章不再重复改造此模块，只复核其他 Rust 模块的
错误边界。

## 41. Clone 审计

重点检查大对象：

- 封面 base64 字符串。
- 完整媒体快照。
- 活动记录集合。
- 任务栏区域集合。
- Settings 快照。

允许保留：

- `AppHandle`
- sender
- WinRT session handle
- 小型配置
- 为跨线程所有权所需的 clone

优化必须建立在调用频率和对象大小证据上。

## 42. 锁和并发

检查：

- 锁持有期间是否调用 Windows API。
- 锁持有期间是否 emit。
- 是否存在读写锁中毒后的恢复策略。
- async command 是否持有同步锁跨越 await。
- 后台 channel 是否有界。
- worker 是否可以停止。
- Explorer 重启是否创建重复 worker。
- 应用退出是否等待必要清理。

## 43. Blocking 和后台线程

区分：

- WinRT 阻塞 `.get()`
- 图像解码
- 文件 I/O
- 任务栏 UI Automation
- Explorer 查询

确保它们不运行在 Tauri 主线程或 async executor 的关键线程上。

## 44. 算法复杂度

重点测量：

- 会话选择。
- 活动记录清理。
- 任务栏区域查找和去重。
- 窗口碰撞计算。
- 封面颜色统计。
- 高频集合的 `Vec::contains`。

当前封面颜色使用固定大小桶，方向合理，不建议机械重写。

## 45. 参数对象和语义类型

修复 `animate_window_width` 的 11 个参数。

另外检查：

- 多个布尔参数。
- 多个坐标整数。
- 毫秒和 Windows ticks。
- 逻辑像素和物理像素。
- HWND 数值。

可用小型类型减少单位混用，但避免过度包装。

---

---

# 十、状态管理与数据一致性

## 46. 状态分类

明确区分：

- Local UI State：面板展开、输入焦点。
- Form Draft：设置页未保存草稿。
- Persistent State：Rust 保存的设置。
- Runtime State：任务栏和媒体状态。
- Derived State：显示文字、按钮可用性。
- Diagnostic State：只供诊断页显示。

## 47. Pinia Store 边界

Bar Store 应负责：

- 当前媒体投影。
- 事件订阅。
- 页面生命周期。
- 控制错误。

不应负责：

- Windows 媒体选择算法。
- 原生窗口定位。
- 设置持久化实现。

Settings Store 应负责：

- 服务端设置快照。
- 表单草稿。
- dirty 状态。
- 保存流程。
- 保存错误。

## 48. 避免重复状态

重点排查：

- Store 同时保存原值和可计算显示值。
- 组件复制 Store 字段后无法同步。
- Bar 同时保存完整快照和重复的元数据字段。
- 设置保存成功后仍使用提交前的草稿对象。

## 49. 并发和乱序

继续强化现有优秀处理：

- listener 注册和首次读取的顺序。
- 快速媒体切换产生的旧事件。
- 连续保存设置时的返回乱序。
- 快速开关 Bar 导致窗口命令乱序。
- 连续打开 Settings 创建重复窗口。
- 页面卸载后异步结果回写。

---

---

# 十一、Vue 组件与前端结构

## 50. 保持页面组件为组合层

当前 Bar 页面较薄，应保留。

Settings 页面继续优化为：

```
SettingsPage
├── SettingsHeader
├── SettingsSidebar
├── GeneralSettingsSection
├── AppearanceSettingsSection
├── MediaSettingsSection
├── TaskbarSettingsSection
└── DiagnosticsSettingsSection
```

页面只负责导航、加载状态和保存协调。

## 51. 拆分超大设置组件

`AppearanceSettingsSection.vue` 接近 500 行，建议按功能拆分：

- `ColorModeField`
- `LayoutAlignmentField`
- `ProgressAppearanceFields`
- `TitleScrollFields`
- `LyricsAppearanceFields`

拆分条件是具有独立数据契约，而不是单纯按行数切割。

## 52. 收拢设置表单契约

子组件采用：

- Props：当前值、错误、disabled。
- Emits：字段更新或语义操作。
- 不直接访问 Store。
- 不直接调用 IPC。
- 不修改传入对象。

## 53. Vue 响应式模型

全面检查：

- 源状态保持最少。
- 可推导字段改用 `computed`。
- Watcher 仅用于副作用。
- Watcher 的异步操作支持失效或取消。
- 大型快照只在根引用替换时更新。
- 不在 template 中执行排序、过滤或复杂函数。
- 不在 computed 中写状态或调用 IPC。

## 54. Listener 和 Timer 生命周期

为以下资源建立统一检查：

- Tauri event listener
- `matchMedia` listener
- timeout
- animation frame
- ResizeObserver
- 原生窗口事件

所有初始化都应应对“异步注册完成前组件已经卸载”的情况。

## 55. Loading 状态细化

不要共用一个 `loading`：

- `isInitializing`
- `isSaving`
- `isRefreshingDiagnostics`
- `isApplyingWindowChanges`
- `isExecutingMediaControl`

避免一次后台刷新导致整个页面不可操作。

## 56. Error 和 Empty UX

区分：

- 当前没有媒体会话。
- 媒体读取失败。
- 当前播放器不支持控制。
- 设置保存失败。
- 权限或系统 API 不可用。
- Settings 窗口显示失败。
- Explorer 重启恢复中。

每种错误说明发生了什么、是否影响数据、能否重试。

## 57. 可访问性

全面检查：

- 图标按钮具备可访问名称。
- Slider 和 ToggleGroup 有标签。
- 错误使用 `aria-describedby`。
- 键盘焦点样式可见。
- Dialog 能正确恢复焦点。
- 不只依靠颜色表达状态。
- 动画遵循 `prefers-reduced-motion`。

## 58. 快捷键和跨平台按键

虽然当前产品是 Windows 专用，仍需检查：

- Escape 关闭临时 UI。
- Enter 提交设置。
- Tab 顺序。
- Space 触发媒体按钮。
- 快捷键不会与 WebView、输入框或系统媒体键冲突。

---

---

# 十二、性能优化

## 59. 启动流程

建立时间点：

```
Process start
Tauri setup complete
Bar WebView created
Vue mounted
Listeners ready
First media snapshot
Bar interactive
```

检查哪些初始化可以：

- 延迟到首次使用。
- 移到后台线程。
- 并行执行。
- 从缓存恢复。
- 等首屏后再执行。

## 60. Bar 首屏

优先目标不是所有后台服务完成，而是：

- 窗口尽快出现或明确保持隐藏。
- 基础布局可用。
- 媒体状态随后更新。
- 设置和诊断初始化不阻塞 Bar。

## 61. DOM 测量

检查 Bar 的：

- `Range.getBoundingClientRect`
- artwork/controls 宽度读取
- requestAnimationFrame
- retry timer

保证同一帧先集中读取，再集中写入，避免 layout thrashing。

## 62. 动画

检查：

- 宽度动画是否可以取消。
- 新动画是否彻底终止旧动画。
- Explorer 重启期间是否继续提交动画。
- `prefers-reduced-motion`。
- 不使用高频 IPC 驱动每一帧。

## 63. Bundle

当前主要输出：

- Settings：207.11 kB
- Bar：139.64 kB
- Settings API 共享块：118.68 kB
- 全局 CSS：98.38 kB

后续需要：

- 生成 bundle 可视化。
- 确认 Reka UI、Motion、图标和 i18n 的占比。
- 检查 Settings 是否导入不需要的组件。
- 建立 chunk 大小预算。
- 检查 Tailwind 是否扫描整个 UI 组件目录导致 CSS 膨胀。

## 64. 封面数据

当前封面以 base64 data URL 进入 WebView，需要评估：

- 最大封面大小。
- 编码带来的约 33% 体积增长。
- 快速切歌时旧封面驻留。
- 相同封面是否重复解码和传输。
- 是否需要有限大小的缓存。
- 是否值得使用 Tauri asset protocol 或 binary channel。

必须测量后再决定，不立即重写。

## 65. Rust Binary Size

已有良好 release 配置：

- LTO
- 单 codegen unit
- `opt-level = "s"`
- strip
- panic abort

继续检查：

- Tauri `unstable` feature 是否必要。
- Windows crate features 是否都实际使用。
- image codecs 是否全部需要。
- tray、autostart、single-instance 插件成本。
- 安装包与二进制大小基线。

---

---

# 十三、文档与可维护性

## 66. 更新架构文档

当前文档质量较好，但部分内容已经落后于实际实现，例如仍描述某些目录为“计划中”。

需要同步：

- 当前模块树。
- 实际媒体数据流。
- Settings 生命周期。
- Explorer 恢复流程。
- 后台 worker。
- 当前 IPC 命令和事件。

## 67. ADR

建议只为关键决策建立 ADR：

- 为什么采用 Child taskbar host。
- 为什么完整快照和时间轴分离。
- 为什么 Rust 是设置唯一所有者。
- 为什么使用 base64 封面或未来为何替换。
- IPC 类型生成方案。

## 68. 注释治理

保留解释：

- Win32/WinRT 限制。
- 生命周期。
- 并发。
- 回退策略。
- 兼容性原因。

删除或避免：

- 重复代码含义。
- 过时计划。
- 无追踪编号的 TODO。
- 对显而易见 getter 的长篇注释。

---
