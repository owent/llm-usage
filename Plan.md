# 桌面用量客户端执行计划

目标：在本地按 Agent、模型和时间汇总 token、模型请求及缓存使用，
支持今日刷新、日/周/月图表、定时提取和界面配置，提供小体积桌面客户端。

Windows 11 x64 首发，GitHub CI 保留 Windows/macOS/Linux（WSL 构建不等于 Linux 桌面验收）。
Harness Agent 指 Nous Research 的 Hermes Agent。只统计本机产生的数据，
不接入云端账单、企业 API 或跨设备账号报表；落盘文件仍须核验来源。
依赖策略（2026-09-29）：npm/Cargo 使用 `^` 浮动最新范围，可复现性由锁文件保证。
设计基线日期：2026-09-24。

逐轮实施细节、命令与证据一律见 [验证记录](docs/validation/desktop-usage/)，
本文件只维护当前状态与待办，不保留历史叙述。

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

详情维护在对应设计文档；各阶段证据见 [验证记录](docs/validation/desktop-usage/)。

| ID | 状态 | 任务 | 依赖 | 完成条件 |
| --- | --- | --- | --- | --- |
| M0 | **已完成**（三平台 CI 首次成功运行已登记：[36444880100](https://github.com/owent/llm-usage/actions/runs/36444880100) 6/6 success，[m0-ci](docs/validation/desktop-usage/m0-ci.md)；空闲内存超标转 M6/M7） | 固定版本样本、开发合同和三平台 CI 基线 | 开始实施 | 按已确认平台方案固定版本（现为浮动最新），使用已获允许的本机数据取得脱敏 fixture；三平台构建、WSL 可行性、包体报告 |
| M1 | **已完成**（V20 初值已测（[第二轮记录](docs/validation/desktop-usage/m5-telemetry-remainders.md)）；迁移/重建/清理前一致备份+空间检查已实施） | 统计合同、SQLite 存储、导入事务和迁移 | M0 | 已知/未知与完整性可区分；请求更新、去重、崩溃恢复、日周月数学用例通过 |
| M1a | **已完成**（[验证记录](docs/validation/desktop-usage/m1a-provenance.md)；完整导入/Merge 另行排期） | [历史来源身份与存储分区](docs/design/desktop-usage/execution.md#m1a) | M1；在 M6 数据导出前完成 | 稳定主机 ID 与来源分区；V28 存储用例通过 |
| M2 | **基本完成**（[M2-A](docs/validation/desktop-usage/m2a-codex.md)、[M2-B/C](docs/validation/desktop-usage/m2bc-resumed.md)、[M2-D](docs/validation/desktop-usage/m2d-layout-versions.md)；claude 已凭 2026-09-30 WSL 真实转录扩展 2.1.197 新记录类型（queue-operation/attachment/last-prompt/synthetic，[WSL 记录](docs/validation/desktop-usage/wsl-agent-installs.md)）；余 gemini/qwen 真实用量 fixture 待本机凭据） | 首批本地适配器及[目录化](docs/design/desktop-usage/execution.md#m2-layout)：Codex、Claude Code、pi、oh-my-pi、Gemini CLI、Qwen Code | M1 | 每 Agent 独立目录 + 版本注册表 + 未知版本兼容尝试；重复扫描不增量 |
| M3 | **基本完成**（kilo 真实核对；2026-09-30 WSL kilo 7.6.2 真实库 latest_fallback 解析 1595 事件（[WSL 记录](docs/validation/desktop-usage/wsl-agent-installs.md)）；其余七产品文档级实施、合成测试通过、真实验收后置：[M3/M4 记录](docs/validation/desktop-usage/m34-kilo-zcode-kimi.md)、[文档级记录](docs/validation/desktop-usage/m3-doclevel-cmdh.md)） | 扩展本地适配器：Cline、Kilo Code、OpenCode、MiMo Code、Zoo Code、DSH、OpenClaw、Hermes Agent | M1 | 分产品/格式验收；流式更正、累计、迁移与聚合重叠通过 |
| M4 | **基本完成**（kimi-code/kimi-work/zcode 真实核对；WorkBuddy `.workbuddy/projects` 真实 8 文件/420 事件、重扫幂等通过，[记录](docs/validation/desktop-usage/m4-buddy-local.md)；余 trace/子 Agent 覆盖与跨版本核对） | 新版 Kimi Code、Kimi Work、ZCode、WorkBuddy 本地格式核验 | M0/M1 | 有本地提取尝试记录；交付适配器或字段/版本限制 |
| M5 | **主体已实施**（Copilot CLI 真实核对；CodeBuddy 扩展存储本机真实核对 PASS、CLI `.codebuddy/projects` 本地适配器已注册，[记录](docs/validation/desktop-usage/m4-buddy-local.md)；OTel spans + 本地 OTLP 接收器 E2E；余本地/OTLP 重叠消解、CLI 真实样本与 V08/V22/V25 场景验收） | 本机文件遥测及可选 loopback 接入：Copilot CLI、VS Code、CodeBuddy 等 | M1 | 本地 OTLP/file 协议、采样/重传、来源归属、敏感字段过滤通过；拒绝远端/账号范围数据 |
| M6 | **主体功能已实现**（逐源定时已接线：核心规则+调度+命令+UI×10 语言，[记录](docs/validation/desktop-usage/m5-telemetry-remainders.md)；余真实桌面逐操作验收 V13–V18/V23–V25、V24 系统任务真实验收、文件监听（优化项）） | 桌面界面、真实刷新、定时任务、设置、保留和导出 | M1/M1a/M2；对接 M3–M5 | 图表联动、定时/休眠补扫/重启恢复、资源合同与可访问名称验收 |
| M7 | **部分完成**（[M7 记录](docs/validation/desktop-usage/m7-build-partial.md)；空闲内存复测 171 MB ≤ 180 目标、V20 双档初值已测（[第二轮记录](docs/validation/desktop-usage/m5-telemetry-remainders.md)）；2026-09-30 安装包修正：移除 sample-data.txt、NSIS 安装器改嵌项目图标（installerIcon）；按用户指示跳过安装流程实机验收；余首屏 P95/离线/系统任务/三平台持续验收） | 轻量化、三平台持续构建、安装与发布候选验收 | M2–M6 | 实机与三平台 CI 证据分列；性能/包体、离线、升级回滚通过 |
| M8 | **文档级实施与 f6ede26 工作区复核已完成**（[验证记录](docs/validation/desktop-usage/m8-second-batch.md)；18 适配器（17 解析+Qoder 探针）注册与回归通过；真实样本验收后置；Amazon Q/Codebuff/iFlow 按边界排除；2026-09-30 WSL 安装级取证：opencode 1.18.33 真实库探测通过、jcode/goose/crush/aider 官方渠道安装与目录骨架、npm 抢注名排除，[WSL 记录](docs/validation/desktop-usage/wsl-agent-installs.md)） | 第二批本地适配器：Amp、Goose、Crush、Roo Code、Aider、Continue、Droid、Amazon Q CLI、Grok Build、Antigravity CLI、Junie CLI、Kiro、Zed 内置、Codebuff、Command Code、jcode、gajae-code、Xum、iFlow CLI、Qoder CLI、AtomCode | M1 框架与 M2 目录/版本合同；不在 M0–M7 关键路径 | 按矩阵分批交付；每适配器独立目录+版本注册表+V30；双载体对账不双计、会话级聚合不虚构逐次、估算路径一律不采信；Cursor/Warp/TRAE/Windsurf 按边界排除或留 F1 |
| M9 | **本轮审查与修复已完成**（2026-10-01：VS Code 217 条已观测主循环调用、10 条用量记录；VS 2 条调用，明细与日汇总均通过真实载体独立对照、重扫零新增。修复快照替换、来源过滤、失败调用与额度时间/小数/清理；新增 13 项 Rust 回归。完整 verify（716 项 Rust 测试）与浏览器回归通过，见 [审查记录](docs/validation/desktop-usage/m9-copilot-review.md)。JetBrains 仍文档级；同面原生与 OTel 择一） | Copilot 本地用量、请求统计与独立额度展示 | M1/M5 框架 | 真实载体独立对照；重扫、更正与副本不双计；未知字段/覆盖保留未知；同面 OTel 载体择一 |
| F1 | 未排期，未实施（缺证 IDE/插件：JetBrains AI Assistant、TRAE、Cursor、Windsurf、JoyCode、CodeGeeX、Comate、InsCode/CodeArts Snap；CodeBuddy IDE/插件已凭本机 CodeBuddyExtension 载体证据于 2026-09-30 实施并真实核对，移出 F1，见 [A04](docs/design/desktop-usage/adapters.md)） | 后续 IDE 支持 | 后续支持阶段 | 逐项取得本地格式证据；当前只保留计划，不探测/开发 |
| F2 | **主体已实施**（费用引擎+V29 合同测试完成，[验证记录](docs/validation/desktop-usage/f2-cost-engine.md)：schema v8 价格表、种子快照、估算引擎（渠道不明不套价/档位/TTL/覆盖标记）、日成本回填与汇总、命令+界面（默认关闭）+i18n ×10；余可选在线刷新（任务 5）、预算提醒、真实数据端到端估算（待用户配置渠道默认）后置） | 模型 API 按量价格获取与 token 费用估算 | 数据合同 | 渠道比较、版本化价格合同、V29 样本设计交付；费用引擎已实施（除上述后置项） |
| F3 | **已完成**（[i18n 合同](docs/design/desktop-usage/i18n.md)，10 语言已随 M6 落地；V31 原生 GUI 验收随桌面验收） | 界面多语言方案调研与设计 | M6 前 | 已交付并实施 |

M4/M5 的本地格式证据不足不阻止 M1/M2/M6，但对应工具必须保持“待验证/受限”，
不能为了宣布计划完成而从能力矩阵删除。F1 不计入首版必验适配器。
F2 只调研公开价格元数据及本地估算，不接入远端用量/账单 API。

## 已确认的实施边界

2026-10-01：Copilot 补充采集与 OTel 配置调研及配置入口已实施，见
[配置合同](docs/design/desktop-usage/copilot-otel.md)与
[验收记录](docs/validation/desktop-usage/telemetry-setup-ui.md)。
已完成全年活动热力图、Copilot 已知输入/未知总量及零值悬浮修复；
总览/设置提供异步用户层检查、修改预览、按字段合并、备份与撤销。
VS Code profile 同步排除项写入默认用户层；Codex 保留已有 exporter 选项及内联表。
已核对扩展/Agent Host/CLI 的 file 设置及本地 trace SQLite 路线；
SQLite 适配和跨载体统计主来源选择仍待实施。新输出隔离供核验，不自动叠加原生用量。
暂停采集不会撤回历史贡献，不能靠停用来源解决跨载体双计；开发未修改真实 IDE/Agent 配置。
本轮 Windows 验收：`npm run verify`、Edge 浏览器回归、只读配置审计、98 个相对链接及
`git diff --check` 通过；Rust 752/UI 10/脚本 3 通过，真实写入及新导出验收后置。
后续 UI 调整已完成：总览只保留简要提示与批量/详情按钮，详情直达设置遥测面板；
批量配置逐项合并并报告部分失败，跨页保留执行和撤销状态；安装检测排除仅有用户目录的候选。
最终 `verify`（Rust 754/UI 13/脚本 3）、两次独立 Edge 回归、5 项本机只读配置候选、
98 个相对链接及 diff 检查通过；新增导出、非 PATH CLI 与跨平台桌面验收缺口保留。

- 实施阶段可只读提取本机真实 Agent 数据验证（已授权，无需重复确认）；脱敏 fixture 与核验记录见验证记录目录。
- JetBrains/TRAE 等缺证 IDE 已后移 F1，当前不实施；Junie CLI 与 Zed 内置凭本地载体证据在 M8。
  JetBrains 的 GitHub Copilot 已源码级取证（M9）：默认本地仅 credit 无逐次 token，
  逐次载体为需启用的 OTel file 导出，经既有 otel 适配器手工根接入，
  见 [分析记录](docs/validation/desktop-usage/m9-jb-copilot-analysis.md)；JetBrains 自家 AI Assistant 仍 F1。
- GitHub runner、工具链及 Linux 基线已在 M0 固定；macOS/Linux 不承诺首发发行支持。
- 包体和性能数字为验收目标；空闲内存超标项转 M6/M7 定位。

## 执行约束

优先方案为 Tauri 2 + Rust + SQLite + Svelte/TypeScript + 按需 ECharts。
先实现有证据的核心，再扩展适配器；不打包 Python/Node 采集服务，不自动启动 Agent。
M2–M5、M8 及后续 F1 的 Agent 适配器统一遵守[独立目录与版本组织合同](docs/design/desktop-usage/architecture.md#adapter-layout)。
应用更新频率低于 Agent；[未知版本默认先尝试该 Agent 最新内置解析器](docs/design/desktop-usage/architecture.md#unknown-version)，
不得仅因版本号未收录就停止采集。
只读取用户启用的本地数据源；定时任务只运行本应用采集逻辑，不启动 Agent 或模型调用。
额度、会话计数、累计快照与逐次模型调用保持不同统计单位；未知用量不补零。
业务命令从实际文件建立，不把本文列出的未来任务当作已运行记录。
