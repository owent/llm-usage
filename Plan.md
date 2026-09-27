# 桌面用量客户端执行计划

## 2026-09-27 逐日热力图与主要语言（已完成本机验证）

按最新反馈，活跃热力图改为一个格子对应一个实际本地日期；调用与 token
来自每日汇总，明细保留期外仍有日层数据时继续显示。仅剩周/月/年归档且无法
按日还原的日期显示斜纹，不当作零调用；同日仅有部分来源的日数据时
保留已知值并显示虚线框。周分布复用同一批每日格子。
补充已知活跃天数、已知最长连续活跃和已知峰值日期，避免短范围热力图面板留白。

界面语言扩展为简体中文、繁体中文、英语、日语、韩语、西班牙语、法语、
德语、巴西葡萄牙语和俄语。10 份目录覆盖全部 292 个界面键，
保存后即时切换；测试检查键与占位符一致。依据与验证见
[本轮记录](docs/validation/desktop-usage/review-2026-09-27.md#逐日热力图与多语言扩展)。

- [x] 按时区逐日查询日层数据，标明不可用/部分覆盖日期，补本机清理与无截止线导入回归。
- [x] 调整日历热力图、周分布提示和活动摘要，验证深浅主题截图。
- [x] 接入 10 种语言、完整目录及一致性测试，验证切换与持久化。
- [x] 完成本轮全量检查、浏览器回归与 Windows release 构建，登记结果。

本轮完整检查通过：483 项 Rust、8 项前端纯逻辑、3 项脚本测试；浏览器
13 类检查、9 张截图且运行时错误 0；Windows release 与 NSIS 打包通过，
候选安装包 2,585,083 字节。浏览器使用合成 IPC；原生 GUI 和各语言
人工文字审校仍在桌面验收范围内。

## 2026-09-27 数据源状态与未知版本兼容（已完成本机验证）

用户反馈 Codex 和 ZCode 显示“需要检查”。本机只读核对与隔离库重扫发现：
Codex 的 38 个文件仍有用量快照与逐次记录不一致等证据，需继续提示；
ZCode 主数据库读取正常，但另有一个含用量的 JSONL 文件无法与数据库调用
可靠对应，不能直接清除告警或叠加统计。
未知版本原已默认尝试本 Agent 最新内置解析器；本轮补齐增量状态与部分坏记录判定，
保留有效调用，不能只因版本号未知显示异常，也不能用兼容标记掩盖实际漏数。
依据与回归见[本轮验证记录](docs/validation/desktop-usage/review-2026-09-27.md#数据源健康与未知版本)。

- [x] 核查真实 Codex/ZCode 文件状态和解析诊断，隔离应用库重扫对照。
- [x] 修复兼容解析的部分失败判定；ZCode 未核对的旁路文件仍保留告警。
- [x] 数据源显示降级和未识别文件数，兼容读取采用普通信息样式。
- [x] 完成本轮全量检查、浏览器验证和 Windows release 构建，登记结果。

本轮 `npm run verify` 通过：481 项 Rust、6 项前端纯逻辑、3 项脚本测试；
浏览器 12 类检查通过、运行时错误 0；Windows release/NSIS 构建通过，
候选安装包 2,568,281 字节。真实应用库保持只读；隔离新库与
合成旧状态回归验证了来源诊断规则，ZCode 文件的用量缺口仍待来源取证。

## 2026-09-27 归档与暗色热力图补充审查（已完成本机验证）

按用户反馈，已改为暗色热力图中性底格与可辨识的调用强度色带；总览和趋势的
默认布局均先展示 token 用量，再展示调用与会话，已保存的用户布局仍按用户选择恢复。
新增 Qwen Code 旧 tmp/新 projects 的活动和归档目录发现，以同来源原生请求身份去重；
Gemini CLI 的自动清理与手动 checkpoint 尚无可叠加的逐次用量证据，保持当前采集边界。
依据、回归与剩余验证见 [本轮记录](docs/validation/desktop-usage/review-2026-09-27.md#归档与界面补充核验)。

- [x] 核查 Qwen、Gemini 与已有 Agent 归档证据及载体重叠。
- [x] 修复 Qwen 归档遗漏，补只有归档、双副本、新旧布局及路径覆盖回归。
- [x] 调整热力图与两页默认顺序，浏览器验证深浅主题。
- [x] 完成本轮改动的全量检查和 Windows release 构建，登记结果。

本轮 `npm run verify` 通过：479 项 Rust、6 项前端纯逻辑、3 项脚本测试；
浏览器 11 类检查通过、8 张截图且运行时错误 0；Windows release/NSIS
构建通过，候选安装包 2,567,112 字节。浏览器使用合成 IPC，本机没有 Qwen
真实归档，原生 GUI 与真实 Qwen 数据仍按验证记录中的边界处理。

## 2026-09-27 全量审查与改进（已完成本轮修复与本机验证）

范围：包含 `4ef354d42dac2ca9f86ae101f70908ff4cb2f7eb` 至 `ff1e458` 的九个提交。
用户已授权修复、补测试、改进全部面板并核验本机归档数据；不提交、推送或部署。
审查证据与验证结果见 [本轮记录](docs/validation/desktop-usage/review-2026-09-27.md)。

- [x] 核对设计、版本、工作区和改动范围（初始工作区干净）。
- [x] 核查各适配器归档发现与跨载体身份，修复遗漏及重复统计（缺证来源仍明确受限）。
- [x] 修复历史保留、统计筛选、名称大小写、调度与界面状态问题，补回归测试。
- [x] 统一五页及图表、表格、设置等组件的布局、主题和文案，增加字段覆盖与活动天数。
- [x] 完成业务检查、本机只读对照、界面验证并同步受影响文档。

本轮结果：476 项 Rust、6 项前端纯逻辑、3 项脚本测试通过；浏览器 10 类检查通过；
Windows release/NSIS 构建通过，候选包约 2.45 MiB。真实 ZCode 快照 8,225 次调用、
2,398,950,011 token 与逐次数据库合计一致，重扫无新增。原生 GUI 全流程、系统任务、
安装和远端三平台 CI 仍属于下方待验项；浏览器验证使用合成 IPC，未替代这些验收。

状态：M0 基本完成（仅余 GitHub CI 首次运行待推送后登记；空闲内存 206.5 MB 超
180 MiB 目标，转 M6/M7 定位）。M1 基本完成（余 V20 性能初值、V15/V16 迁移前空间检查
与一致备份；[M0/M1 审查记录](docs/validation/desktop-usage/m0-m1-review.md)）。
M2 基本完成：六源适配器、适配器目录化迁移/版本注册表与未知版本兼容尝试均已验收
（[M2-A](docs/validation/desktop-usage/m2a-codex.md)、
[M2-B/C](docs/validation/desktop-usage/m2bc-resumed.md)、
[M2-D](docs/validation/desktop-usage/m2d-layout-versions.md)）；Codex 0.139–0.151
旧载体专用实现已完成并由本轮全套测试复验；余 claude/gemini/qwen 真实 fixture（本机无数据）。
M1a 已完成（[验证记录](docs/validation/desktop-usage/m1a-provenance.md)：主机身份/
来源分区/交换合同，V28 通过）。M6 主体功能已实现（界面/刷新/调度/设置/导出/
headless/i18n，[验证记录](docs/validation/desktop-usage/m6-desktop-core.md)），
余真实桌面逐操作验收（V13–V18/V23–V25）、系统任务真实验收（V24，注册命令已实现）、
逐源定时与监听等（见记录未完成项）。
F3 调研已完成（[i18n 合同](docs/design/desktop-usage/i18n.md)），实施已随 M6 落地，
现扩展为上述 10 种语言。F2 调研已完成（[价格合同](docs/design/desktop-usage/pricing.md)，
费用引擎实施待排期）。M3/M4 基本完成（七产品文档级实施、kilo/kimi-code/
kimi-work/zcode 真实核对，真实验收后置）；M5 待执行；M7 部分完成
（release 构建 + NSIS 包体 + WSL 编译/测试）。F1 未排期。
用户已允许实施阶段提取本机真实 Agent 数据验证。设计基线日期：2026-09-24。

M5 与 F2 仅为计划与合同，不表示已实现遥测接入或费用估算；
聚合层导入已按 M1a 判定实施（明细级导入与完整 Merge 另行排期）。

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
| M2 | **基本完成**（六源适配器 + 目录化迁移/版本注册表 + 未知版本兼容尝试 + Codex 逐版本 fixture 已验收：[M2-A](docs/validation/desktop-usage/m2a-codex.md)、[M2-B/C](docs/validation/desktop-usage/m2bc-resumed.md)、[M2-D](docs/validation/desktop-usage/m2d-layout-versions.md)；Codex 0.139–0.151 旧载体专用实现已完成并由本轮复验；余 claude/gemini/qwen 真实 fixture 待本机数据） | 首批本地适配器及[目录化](docs/design/desktop-usage/execution.md#m2-layout)：Codex、Claude Code、pi、oh-my-pi、Gemini CLI、Qwen Code | M1 | 每个 Agent 独立目录，历史版本在该目录内实现；已有单文件迁移与 V30 对应回归通过；每个已验证版本有固定格式样本；未知版本默认尝试最新内置解析器，通过校验的数据带兼容标记统计；重复扫描不增量；主调用、辅助调用、子 Agent 的覆盖情况可见 |
| M3 | **基本完成**（kilo 真实核对；cline/dsh/hermes/openclaw/opencode/mimo/zoo 七产品文档级证据实施、合成测试通过、真实验收后置：[M3/M4 记录](docs/validation/desktop-usage/m34-kilo-zcode-kimi.md)、[文档级记录](docs/validation/desktop-usage/m3-doclevel-cmdh.md)） | 扩展本地适配器：Cline、Kilo Code、OpenCode、MiMo Code、Zoo Code、DSH、OpenClaw、Hermes Agent | M1；可与 M2 独立排期 | 分产品/格式验收；流式更正、模型/辅助累计、历史迁移与聚合重叠通过 |
| M4 | **基本完成**（kimi-code/kimi-work/zcode 适配器已实施并真实核对，[M3/M4 记录](docs/validation/desktop-usage/m34-kilo-zcode-kimi.md)；WorkBuddy 本机已无数据（盘点 2026-09-25 全盘未检出，M0 三处残留目录已消失），无本地格式可核验——待重新出现数据再验） | 新版 Kimi Code、Kimi Work、ZCode、WorkBuddy 本地格式核验 | M0/M1 | 本阶段产品有本地提取尝试记录；交付适配器或字段/版本限制；F1 的 IDE 不实施，不以远端 API 替代 |
| M5 | 未完成（M0 已取证：Copilot CLI 1.0.73 assistant_usage_events 逐 turn 字段已证） | 本机文件遥测及可选 loopback 接入：Copilot CLI、VS Code、CodeBuddy 等 | M1 | 本地 OTLP/file 协议、采样/重传、来源归属、敏感字段过滤通过；拒绝远端/账号范围数据 |
| M6 | **主体功能已实现，部分验收**（[验证记录](docs/validation/desktop-usage/m6-desktop-core.md)：界面/图表/筛选/来源页/设置/导出导入/调度/headless/i18n/多用户齐备，headless 端到端三源 11,401 事件验证；对话框导出与聚合导入已实现；余真实桌面逐操作验收 V13–V18/V23–V25、V24 系统任务真实验收、逐源定时、监听等） | 桌面界面、真实刷新、定时任务、设置、保留和导出 | M1/M1a/M2；对接 M3–M5 | 图表与日周月联动；可供后续合并的数据导出保留来源身份、覆盖范围和修订；全局/逐源定时配置、手动触发去重、休眠补扫、Windows 关闭界面提取和重启恢复通过；按[资源合同](desktop/assets/README.md)接入导航、主题托盘与空状态，核验高 DPI、深浅主题和可访问名称 |
| M7 | **部分完成**（release 构建 + NSIS 包体 2.24 MiB 达标 + WSL 编译/测试：[M7 记录](docs/validation/desktop-usage/m7-build-partial.md)；余真实安装/资源/系统任务/三平台 CI 验收） | 轻量化、三平台持续构建、安装与发布候选验收 | M2–M6 | Windows 11 实机与三平台 CI 证据分列；适配器状态明确，性能/包体、离线、任务清理与升级回滚通过；确认 LFS 下载后的安装包、任务栏及 macOS/Linux 桌面图标 |
| F1 | 后续支持，未排期，未实施 | 后续 IDE 支持：JetBrains/TRAE、Zed 内置及其他缺证 IDE 变体 | 后续支持阶段；不属于 M0–M7 | 启动该阶段后逐项取得本地格式证据及验证结果；当前只保留计划，不探测/开发，不阻塞首版 |
| F2 | **调研已完成**（[价格合同](docs/design/desktop-usage/pricing.md)：渠道比较/四家计费维度核验/版本化快照合同/V29 样本设计；费用引擎与 UI 实施待排期） | [模型 API 按量价格获取与 token 费用估算方案](docs/design/desktop-usage/execution.md#f2) | 数据合同；调研不阻塞首版 | 比较价格获取渠道并记录可核验证据；明确模型映射、计费维度、版本/生效期、离线缓存及更新方式；交付 V29 验收样本设计和后续实施任务 |
| F3 | **调研与实施已完成**（[i18n 合同](docs/design/desktop-usage/i18n.md)：方案证据、10 种完整语言目录、协商/格式化与回归；V31 原生 GUI 验收随桌面验收） | [界面多语言（i18n）方案调研与设计](docs/design/desktop-usage/execution.md#f3) | M6 界面实现开始前完成调研设计；不阻塞 M0–M5 | 比较 Svelte/Tauri 生态 i18n 方案并记录可核验证据；确定语言集合/协商顺序/缺失键回退/切换行为、数字日期单位本地化口径及诊断消息语言策略；交付键名与语言包合同及 V31 验收要点，实施随 M6 |

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
