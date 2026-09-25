# 桌面用量客户端执行计划

状态：M0 基本完成（仅余 GitHub CI 首次运行待推送后登记；空闲内存 206.5 MB 超
180 MiB 目标，转 M6/M7 定位）。M1 基本完成（余 V20 性能初值、V15/V16 迁移前空间检查
与一致备份；[M0/M1 审查记录](docs/validation/desktop-usage/m0-m1-review.md)）。
M2 基本完成：六源适配器、适配器目录化迁移/版本注册表与未知版本兼容尝试均已验收
（[M2-A](docs/validation/desktop-usage/m2a-codex.md)、
[M2-B/C](docs/validation/desktop-usage/m2bc-resumed.md)、
[M2-D](docs/validation/desktop-usage/m2d-layout-versions.md)）；余 Codex
0.139–0.151 旧载体专用实现（需先取证 token_count/last_token_usage 身份语义）
与 claude/gemini/qwen 真实 fixture（本机无数据）。
M1a 已完成（[验证记录](docs/validation/desktop-usage/m1a-provenance.md)：主机身份/
来源分区/交换合同，V28 通过）。M6 主体功能已实现（界面/刷新/调度/设置/导出/
headless/i18n，[验证记录](docs/validation/desktop-usage/m6-desktop-core.md)），
余真实桌面逐操作验收、系统任务注册（V24）、逐源定时与监听等（见记录未完成项）。
F3 调研已完成（[i18n 合同](docs/design/desktop-usage/i18n.md)），实施已随 M6 落地
zh-CN/en 双语。F2 调研已完成（[价格合同](docs/design/desktop-usage/pricing.md)，
费用引擎实施待排期）。M3–M5、M7 待执行（kilo/zcode/kimi 适配器进行中）；
F1 未排期。
用户已允许实施阶段提取本机真实 Agent 数据验证。设计基线日期：2026-09-24。

M3–M5 与 F2 仅为计划与合同，不表示已实现对应适配器、遥测接入或费用估算；
完整导入/Merge 功能未实施（M1a 交付格式与判定）。

目标：在本地按 Agent、模型和时间汇总 token、模型请求及缓存使用，
支持今日刷新、日/周/月图表、定时提取和界面配置，提供小体积桌面客户端。

已确认：Windows 11 x64 首发，GitHub CI 保留 Windows/macOS/Linux；本地可用 WSL 2
尝试 Linux 构建。Harness Agent 指 Nous Research 的 Hermes Agent。全部 Agent 保留本地支持计划，
JetBrains/TRAE 等缺少本地格式证据的 IDE 列入后续 F1，当前不实施；
只统计本机产生的数据，不接入云端账单、企业 API 或跨设备账号报表。

**请确保深度思考调研后再执行，禁止猜测。按需更新AI agent提示词、skills和各类文档。及时更新完成进度。**

## 设计入口

- [范围、决策与需求映射](docs/design/desktop-usage/README.md)
- [架构、数据库选择与资源目标](docs/design/desktop-usage/architecture.md)
- [统计、存储、去重与保留合同](docs/design/desktop-usage/data-contract.md)
- [Agent 接入调研与能力矩阵](docs/design/desktop-usage/adapters.md)
- [任务详情、依赖与交付顺序](docs/design/desktop-usage/execution.md)
- [定时提取与后台生命周期](docs/design/desktop-usage/scheduling.md)
- [三平台 CI、Windows 首发与 WSL 验证](docs/design/desktop-usage/platform-ci.md)
- [测试与验收计划](docs/design/desktop-usage/validation.md)
- [源码、官方来源与调研限制](docs/design/desktop-usage/research.md)
- [开工准备结论与真实数据验证范围](docs/design/desktop-usage/implementation-readiness.md)
- [模型价格获取与费用估算调研](docs/design/desktop-usage/pricing.md)

## 待办与验收

详情维护在对应设计文档。状态列逐阶段标注完成/未完成；各阶段证据见
[验证记录](docs/validation/desktop-usage/)。

| ID | 状态 | 任务 | 依赖 | 完成条件 |
| --- | --- | --- | --- | --- |
| M0 | **基本完成**（仅余 GitHub CI 首次运行待推送后登记；空闲内存 206.5 MB 超 180 MiB 目标，转 M6/M7 定位） | 固定版本样本、开发合同和三平台 CI 基线 | 开始实施 | 按已确认平台方案锁定版本，使用已获允许的本机数据取得脱敏 fixture；三平台构建、WSL 可行性、包体报告；恢复文档依赖声明 |
| M1 | **基本完成**（余 V20 性能初值、V15/V16 迁移前空间检查和一致备份；[审查记录](docs/validation/desktop-usage/m0-m1-review.md)） | 统计合同、SQLite 存储、导入事务和迁移 | M0 | 已知/未知与完整性可区分；请求更新、去重、崩溃恢复、日周月数学用例通过 |
| M1a | **已完成**（[验证记录](docs/validation/desktop-usage/m1a-provenance.md)：主机身份/来源分区/legacy_unknown 迁移/交换合同与合并判定，V28 通过；完整导入/Merge 另行排期） | [历史来源身份与存储分区](docs/design/desktop-usage/execution.md#m1a)，为后续导入/导出/Merge 保留依据 | M1；在 M6 数据导出前完成 | 稳定主机 ID、主机名和来源实例可追溯；事件唯一键、历史汇总分区及索引保留来源；旧数据迁移不猜测归属；V28 对应存储用例通过 |
| M2 | **基本完成**（六源适配器 + 目录化迁移/版本注册表 + 未知版本兼容尝试 + Codex 逐版本 fixture 已验收：[M2-A](docs/validation/desktop-usage/m2a-codex.md)、[M2-B/C](docs/validation/desktop-usage/m2bc-resumed.md)、[M2-D](docs/validation/desktop-usage/m2d-layout-versions.md)；余 Codex 0.139–0.151 旧载体专用实现待取证、claude/gemini/qwen 真实 fixture 待本机数据） | 首批本地适配器及[目录化](docs/design/desktop-usage/execution.md#m2-layout)：Codex、Claude Code、pi、oh-my-pi、Gemini CLI、Qwen Code | M1 | 每个 Agent 独立目录，历史版本在该目录内实现；已有单文件迁移与 V30 对应回归通过；每个已验证版本有固定格式样本；未知版本默认尝试最新内置解析器，通过校验的数据带兼容标记统计；重复扫描不增量；主调用、辅助调用、子 Agent 的覆盖情况可见 |
| M3 | **部分完成**（kilo 真实核对 + cline/dsh/hermes/openclaw 文档级证据实施：[M3/M4 记录](docs/validation/desktop-usage/m34-kilo-zcode-kimi.md)、[文档级记录](docs/validation/desktop-usage/m3-doclevel-cmdh.md)；余 opencode/mimo/zoo 待限额恢复后同模式实施） | 扩展本地适配器：Cline、Kilo Code、OpenCode、MiMo Code、Zoo Code、DSH、OpenClaw、Hermes Agent | M1；可与 M2 独立排期 | 分产品/格式验收；流式更正、模型/辅助累计、历史迁移与聚合重叠通过 |
| M4 | **基本完成**（kimi-code/kimi-work/zcode 适配器已实施并真实核对，[M3/M4 记录](docs/validation/desktop-usage/m34-kilo-zcode-kimi.md)；WorkBuddy 本机已无数据（盘点 2026-09-25 全盘未检出，M0 三处残留目录已消失），无本地格式可核验——待重新出现数据再验） | 新版 Kimi Code、Kimi Work、ZCode、WorkBuddy 本地格式核验 | M0/M1 | 本阶段产品有本地提取尝试记录；交付适配器或字段/版本限制；F1 的 IDE 不实施，不以远端 API 替代 |
| M5 | 未完成（M0 已取证：Copilot CLI 1.0.73 assistant_usage_events 逐 turn 字段已证） | 本机文件遥测及可选 loopback 接入：Copilot CLI、VS Code、CodeBuddy 等 | M1 | 本地 OTLP/file 协议、采样/重传、来源归属、敏感字段过滤通过；拒绝远端/账号范围数据 |
| M6 | **主体功能已实现，部分验收**（[验证记录](docs/validation/desktop-usage/m6-desktop-core.md)：界面/图表/筛选/来源页/设置/导出/调度/headless/i18n 齐备，headless 端到端三源 11,401 事件验证；余真实桌面逐操作验收 V13–V18/V23–V25、V24 系统任务、逐源定时、监听、对话框导出等） | 桌面界面、真实刷新、定时任务、设置、保留和导出 | M1/M1a/M2；对接 M3–M5 | 图表与日周月联动；可供后续合并的数据导出保留来源身份、覆盖范围和修订；全局/逐源定时配置、手动触发去重、休眠补扫、Windows 关闭界面提取和重启恢复通过；按[资源合同](desktop/assets/README.md)接入导航、主题托盘与空状态，核验高 DPI、深浅主题和可访问名称 |
| M7 | **部分完成**（release 构建 + NSIS 包体 2.24 MiB 达标 + WSL 编译/测试：[M7 记录](docs/validation/desktop-usage/m7-build-partial.md)；余真实安装/资源/系统任务/三平台 CI 验收） | 轻量化、三平台持续构建、安装与发布候选验收 | M2–M6 | Windows 11 实机与三平台 CI 证据分列；适配器状态明确，性能/包体、离线、任务清理与升级回滚通过；确认 LFS 下载后的安装包、任务栏及 macOS/Linux 桌面图标 |
| F1 | 后续支持，未排期，未实施 | 后续 IDE 支持：JetBrains/TRAE、Zed 内置及其他缺证 IDE 变体 | 后续支持阶段；不属于 M0–M7 | 启动该阶段后逐项取得本地格式证据及验证结果；当前只保留计划，不探测/开发，不阻塞首版 |
| F2 | **调研已完成**（[价格合同](docs/design/desktop-usage/pricing.md)：渠道比较/四家计费维度核验/版本化快照合同/V29 样本设计；费用引擎与 UI 实施待排期） | [模型 API 按量价格获取与 token 费用估算方案](docs/design/desktop-usage/execution.md#f2) | 数据合同；调研不阻塞首版 | 比较价格获取渠道并记录可核验证据；明确模型映射、计费维度、版本/生效期、离线缓存及更新方式；交付 V29 验收样本设计和后续实施任务 |
| F3 | **调研已完成**（[i18n 合同](docs/design/desktop-usage/i18n.md)：方案对比与选型证据、键名/回退/格式化口径；zh-CN/en 已随 M6 落地，V31 验收随桌面验收） | [界面多语言（i18n）方案调研与设计](docs/design/desktop-usage/execution.md#f3) | M6 界面实现开始前完成调研设计；不阻塞 M0–M5 | 比较 Svelte/Tauri 生态 i18n 方案并记录可核验证据；确定语言集合/协商顺序/缺失键回退/切换行为、数字日期单位本地化口径及诊断消息语言策略；交付键名与语言包合同及 V31 验收要点，实施随 M6 |

M4/M5 的本地格式证据不足不阻止 M1/M2/M6，但对应工具必须保持“待验证/受限”，
不能为了宣布计划完成而从能力矩阵删除。M7 可交付有明确支持范围的首个版本，
未解决的工具继续留在本计划。
F1 单独标为“后续支持，未实施”，不计入首版必验适配器；具体变体边界见接入矩阵。
M1a 先保存来源与交换合同，完整导入/Merge 功能另行排期；来源不同不代表用量覆盖必然互斥。
F2 只调研公开价格元数据及本地估算，不接入远端用量/账单 API；估算实现待调研后细化排期。

## 已确认的实施边界与首阶段核验

- 实施阶段可只读提取本机真实 Agent 数据验证，无需重复确认这项已给出的允许；脱敏 fixture 提取与核验记录见验证记录目录。
- JetBrains/TRAE 等缺证 IDE 已后移 F1，保留支持计划，当前不实施。
- GitHub runner、工具链及 Linux 基线已在 M0 固定；WSL/WSLg 实际探测已在 M0 完成，macOS/Linux 不承诺首发发行支持。
- 包体和性能数字为验收目标；M0 实测包体已出报告，空闲内存 206.5 MB 超 180 MiB 目标，转 M6/M7 定位。

设计准备结论、数据验证步骤和未执行项见开工准备文档；以上技术核验属于实施任务，不再列为待用户决策。

## 执行约束

优先方案为 Tauri 2 + Rust + SQLite + Svelte/TypeScript + 按需 ECharts。
先实现有证据的核心，再扩展适配器；不打包 Python/Node 采集服务，不自动启动 Agent。
M2–M5 及后续 F1 的 Agent 适配器统一遵守[独立目录与版本组织合同](docs/design/desktop-usage/architecture.md#adapter-layout)：
每个 Agent 使用自己的目录，当前及历史版本实现留在该目录内。
应用更新频率低于 Agent；[未知版本默认先尝试该 Agent 最新内置解析器](docs/design/desktop-usage/architecture.md#unknown-version)，
不得仅因版本号未收录就停止采集。兼容尝试成功与逐版本验证通过分别记录。
只读取用户启用的本地数据源；定时任务只运行本应用采集逻辑，不启动 Agent 或模型调用。
本地数据的来源边界见设计入口；仅将文件保存到本地不构成本机使用证据。
额度、会话计数、累计快照与逐次模型调用保持不同统计单位。
业务命令在 M0 从实际文件建立，不把本文列出的未来任务当作已运行记录。
