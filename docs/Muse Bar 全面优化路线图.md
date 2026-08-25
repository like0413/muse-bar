# Muse Bar 全面优化路线图

---

# 一、实施优先级

本章是其余优化工作的控制面。后续改动以这里定义的顺序、阶段门槛和累计回归规则为准；
阶段编号表示依赖关系，不代表必须把某一阶段的所有长期事项一次完成。

## 1. 优先级判定规则

每个优化项必须同时记录优先级、收益、修改风险和成本：

| 优先级 | 判定标准                                       | 处理规则               |
| ------ | ---------------------------------------------- | ---------------------- |
| P0     | 数据丢失、任意代码执行、稳定复现的崩溃或死锁   | 停止当前阶段，立即处理 |
| P1     | 安全边界缺失、关键功能不可靠、质量门禁失效     | 当前阶段优先完成       |
| P2     | 明显维护性问题、测试缺口、可证实的中等性能问题 | 纳入对应阶段           |
| P3     | 命名、局部重复、轻微结构或格式问题             | 仅在不扩大改动时处理   |

同一优先级内按以下顺序排序：

1. 高收益、低风险、低成本。
2. 会阻塞后续阶段的基础设施。
3. 能为后续重构提供测试或类型保护的工作。
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
| `vp test`                                                   | 失败     | 尚无前端测试文件                                       |
| Rust Clippy `-D warnings`                                   | 通过     | 动画调用已改用具名 request 结构体                      |

每完成一章，都在对应实施记录中更新实际命令结果；不得把“命令未运行”写成“通过”。

## 3. 累计优化规则

1. 每批只处理一个可独立描述的问题或一组强相关问题。
2. 修改前记录问题、范围、预期收益、风险和验证方法。
3. 保持用户可见行为不变，除非当前任务明确要求改变行为。
4. 当前阶段不得破坏此前阶段已经建立的安全、类型、测试和架构约束。
5. 发现前序阶段回归时，暂停新增工作并先修复回归。
6. 性能修改必须记录指标、测试场景和 Before/After；没有证据时只建立基线。
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
            → 第七章 测试体系
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
| 可靠性保护网   | 第六章、第七章     | 恢复路径明确，核心逻辑拥有自动测试        |
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
|    6 | 第六章：可靠性与恢复能力        | 进行中 | 第五章完成   | 开始第 29 节 |
|    7 | 第七章：测试体系                | 未开始 | 第六章完成   | 等待第六章   |
|    8 | 第八章：架构与 SOLID            | 未开始 | 第七章完成   | 等待第七章   |
|    9 | 第九章：Rust 模块和代码质量     | 未开始 | 第八章完成   | 等待第八章   |
|   10 | 第十章：状态管理与数据一致性    | 未开始 | 第九章完成   | 等待第九章   |
|   11 | 第十一章：Vue 组件与前端结构    | 未开始 | 第十章完成   | 等待第十章   |
|   12 | 第十二章：性能优化              | 未开始 | 第十一章完成 | 等待第十一章 |
|   13 | 第十三章：文档与可维护性        | 未开始 | 第十二章完成 | 最终文档同步 |

---

# 二、工程化与依赖治理

## 7. 质量门禁

本节只处理已经由命令确认的问题，不在此章提前建设完整测试体系。

`Confirmed`：

- `vp check` 当前被 `.agents/**` 等非产品文件的格式问题阻断。
- 严格 Clippy 当前被 `animate_window_width` 参数过多阻断。

`Keep`：

- `vp install` 已通过，依赖处于同步状态。
- `vp run type-check`、`vp build` 和 Cargo check 已通过。

`Conditional`：

- `vp test` 当前因没有测试文件失败，但测试内容和入口统一在第七章建设；第二章不创建占位测试。

本章目标命令：

```
vp install
vp check
vp run type-check
vp build
cargo check --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
```

完成条件是上述命令稳定通过，且检查命令不修改生成文件。`vp test` 留在第七章验收。

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

触发后先建立最小 Windows CI，只运行当时已经稳定的安装、类型检查、构建、Cargo check、测试和
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

本章没有启动开发服务。前端测试仍按第七章建立测试体系后执行，不将当前“没有测试文件”误记为
第四章回归。

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

运行验证后已停止本章启动的 Tauri、Vite 和 Muse Bar 进程；端口 1420 空闲。前端测试仍按第七章
建立测试体系后执行。

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
- 新增 11 个 Rust 回归测试，覆盖已有文件替换、文件占用、临时文件清理、缺失字段默认值、旧版本
  写回、未来版本保护、损坏文件备份和备份数量上限。

启动回归修复：设置持久化改造曾扩大日志插件的注册时序，暴露了 Bar 原生窗口先显示、媒体初始快照
暂时为空后再隐藏的竞态。现已恢复原有日志注册时序；任务栏挂载按服务端可见状态决定是否显示，位置
稳定化不再隐式重新显示窗口；前端先选择并绑定媒体会话再读取快照；已绑定会话的元数据缓存为空时会
主动重试。新增 4 个 Rust 测试覆盖元数据重试判定，避免冷启动失败后永久保持空快照。

磁盘耗尽和真实用户 ACL 拒绝需要破坏性系统环境，不在普通单元测试中修改机器状态；两者与已测试的
文件占用共用同一 I/O 错误传播、临时文件清理以及“保存失败不替换内存设置”路径，发布前在隔离的
Windows 测试用户中执行故障注入。

## 29. Explorer 恢复

验证：

- Explorer 连续重启。
- 恢复期间退出应用。
- 恢复期间切换 Bar。
- 多个恢复请求合并。
- 旧窗口句柄失效。
- 自动隐藏任务栏。
- 多显示器和 DPI 变化。

## 30. 媒体服务恢复

验证：

- 播放器退出。
- 会话关闭但事件晚到。
- 播放器卡死。
- WinRT manager 初始化失败。
- 连续快速切歌。
- 同名歌曲。
- 封面损坏或超限。
- 时间轴回退。
- 播放器拒绝控制。

## 31. 后台任务生命周期

每个 worker 都需要明确：

- start
- running
- cancel
- shutdown
- restart
- error recovery

避免重复启动或应用退出后残留任务。

## 32. 缓存治理

所有缓存明确：

- 最大条目数。
- 最大字节数。
- 失效条件。
- Explorer 或媒体会话重置行为。
- 是否包含大字符串。
- 是否需要 LRU。

---

---

# 七、测试体系

## 33. Rust 单元测试

优先测试：

- 设置默认值、迁移和规范化。
- 颜色解析和主色算法。
- 时间单位转换。
- 媒体播放器识别。
- 媒体会话选择。
- 任务栏矩形计算。
- 碰撞和宽度限制。
- 错误映射。

## 34. Rust 集成测试

覆盖：

- 设置文件读写。
- 损坏配置备份。
- 临时文件替换。
- command DTO 序列化。
- IPC 错误结构。

## 35. Vue 单元测试

覆盖：

- Settings Store dirty/save/reset。
- Bar Store 旧时间轴丢弃。
- listener 卸载竞态。
- 控制错误展示。
- 设置 reader 的异常值回退。
- 组件 props/emits 契约。

## 36. 组件测试

重点测试：

- Settings 保存按钮状态。
- 字段错误提示。
- 媒体控制按钮能力禁用。
- 无媒体和失败状态。
- 键盘操作。
- reduced-motion。

## 37. Windows 人工测试矩阵

自动化无法替代：

- Windows 11 不同缩放。
- 多显示器。
- 任务栏自动隐藏。
- 全屏应用。
- Explorer 重启。
- 不同任务栏对齐。
- 四类目标播放器。
- 当前用户安装和卸载。
- 开机自启。

---

---

# 八、架构与 SOLID

### 1. 单一职责 SRP

重点拆分职责混合的模块：

- `system_media.rs`：媒体管理器、事件订阅、元数据、封面、颜色、时间轴混合。
- `media_activity.rs`：会话观察、活动记录、选择算法和事件发布混合。
- `taskbar_occupancy.rs`：采集、缓存、分类、回退策略混合。
- `AppearanceSettingsSection.vue`：多组设置 UI、校验和草稿同步混合。
- `settings-store.ts`：表单状态、IPC、错误、生命周期混合。

建议按真实职责拆分，但不建立空洞的多层目录。

### 2. 开闭原则 OCP

把容易扩展的策略从流程代码中提取：

- 播放器识别规则。
- 媒体会话选择规则。
- 任务栏占用区域识别规则。
- 设置迁移步骤。
- 进度颜色来源。
- 窗口宿主模式。

目标是增加播放器或迁移版本时，少修改核心协调流程。

### 3. 里氏替换 LSP

Rust 当前没有明显的复杂继承体系，LSP 不是主要风险。若未来为任务栏检测、媒体源建立 trait，应保证：

- 所有实现遵循相同错误语义。
- 不出现某个实现接受输入后偷偷忽略操作。
- 不用 trait 伪装本质上能力不同的平台接口。

现阶段不建议为了 SOLID 主动增加 trait。

### 4. 接口隔离 ISP

避免前端模块暴露过大的 API：

- 将 `media-api.ts` 分为查询、控制、事件三个接口面。
- Settings 页面不应接触媒体诊断接口。
- Bar 页面只导入渲染所需快照和控制接口。
- Rust command 按窗口用途检查暴露范围。
- Composable/Store 只暴露只读状态和明确 action。

### 5. 依赖倒置 DIP

优先在需要测试的边界应用：

- 媒体选择算法依赖普通 DTO，不直接依赖 WinRT session。
- 设置迁移依赖序列化数据，不依赖 Tauri `AppHandle`。
- 任务栏几何计算依赖矩形数据，不直接依赖 HWND。
- Windows API 获取负责采集，纯逻辑负责决策。

不建议在所有 Rust 模块上引入 repository/service trait。

### 6. 明确模块依赖规则

建立并检查以下方向：

```
Vue Component
  → Store / composable
  → Typed IPC API
  → Tauri command
  → Application logic
  → Windows adapter
```

禁止：

- Vue 组件散落直接 `invoke()`。
- Command 内实现复杂业务算法。
- Windows 类型泄漏到 IPC DTO。
- Rust 应用层反向依赖窗口 UI。
- Store 相互直接修改内部状态。

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
