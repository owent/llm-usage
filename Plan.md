# 桌面用量客户端执行计划

状态：实施前设计准备已完成。M0 主体完成：根文档依赖声明已恢复并锁定、版本全部固定、
最小 release 试验与包体/内存测量完成、5 源脱敏 fixture 取得、CI 矩阵建立、WSL 构建与 WSLg 冒烟通过，
证据见 [验证记录](docs/validation/desktop-usage/)；剩余 GitHub CI 首次实际运行待推送后登记。
M1–M7 待执行。用户已允许实施阶段提取本机真实 Agent 数据验证。设计基线日期：2026-09-24。

目标：在本地按 Agent、模型和时间汇总 token、模型请求及缓存使用，
支持今日刷新、日/周/月图表、定时提取和界面配置，提供小体积桌面客户端。

已确认：Windows 11 x64 首发，GitHub CI 保留 Windows/macOS/Linux；本地可用 WSL 2
尝试 Linux 构建。Harness Agent 指 Nous Research 的 Hermes Agent。全部 Agent 保留本地支持计划，
JetBrains/TRAE 等缺少本地格式证据的 IDE 列入后续 F1，当前不实施；
只统计本机产生的数据，不接入云端账单、企业 API 或跨设备账号报表。

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

## 待办与验收

详情维护在对应设计文档。状态列逐阶段标注完成/未完成；M0 起各阶段证据见
[验证记录](docs/validation/desktop-usage/)。

| ID | 状态 | 任务 | 依赖 | 完成条件 |
| --- | --- | --- | --- | --- |
| M0 | **基本完成**（2026-09-24；仅余 GitHub CI 首次运行待推送后登记；空闲内存 206.5 MB 超 180 MiB 目标，转 M6/M7 定位） | 固定版本样本、开发合同和三平台 CI 基线 | 开始实施 | 按已确认平台方案锁定版本，使用已获允许的本机数据取得脱敏 fixture；三平台构建、WSL 可行性、包体报告；恢复文档依赖声明 |
| M1 | 未完成 | 统计合同、SQLite 存储、导入事务和迁移 | M0 | 已知/未知与完整性可区分；请求更新、去重、崩溃恢复、日周月数学用例通过 |
| M2 | 未完成 | 首批本地适配器：Codex、Claude Code、pi、oh-my-pi、Gemini CLI、Qwen Code | M1 | 每个发布版本有固定格式样本；重复扫描不增量；主调用、辅助调用、子 Agent 的覆盖情况可见 |
| M3 | 未完成 | 扩展本地适配器：Cline、Kilo Code、OpenCode、MiMo Code、Zoo Code、DSH、OpenClaw、Hermes Agent | M1；可与 M2 独立排期 | 分产品/格式验收；流式更正、模型/辅助累计、历史迁移与聚合重叠通过 |
| M4 | 未完成（M0 已取证：Kimi Code 1.0.3 与 ZCode 3.14.3 本机字段已证，Kimi Work/WorkBuddy 本机无数据） | 新版 Kimi Code、Kimi Work、ZCode、WorkBuddy 本地格式核验 | M0/M1 | 本阶段产品有本地提取尝试记录；交付适配器或字段/版本限制；F1 的 IDE 不实施，不以远端 API 替代 |
| M5 | 未完成（M0 已取证：Copilot CLI 1.0.73 assistant_usage_events 逐 turn 字段已证） | 本机文件遥测及可选 loopback 接入：Copilot CLI、VS Code、CodeBuddy 等 | M1 | 本地 OTLP/file 协议、采样/重传、来源归属、敏感字段过滤通过；拒绝远端/账号范围数据 |
| M6 | 未完成 | 桌面界面、真实刷新、定时任务、设置、保留和导出 | M1/M2；对接 M3–M5 | 图表与日周月联动；全局/逐源定时配置、手动触发去重、休眠补扫、Windows 关闭界面提取和重启恢复通过；按[资源合同](desktop/assets/README.md)接入导航、主题托盘与空状态，核验高 DPI、深浅主题和可访问名称 |
| M7 | 未完成 | 轻量化、三平台持续构建、安装与发布候选验收 | M2–M6 | Windows 11 实机与三平台 CI 证据分列；适配器状态明确，性能/包体、离线、任务清理与升级回滚通过；确认 LFS 下载后的安装包、任务栏及 macOS/Linux 桌面图标 |
| F1 | 后续支持，未排期，未实施 | 后续 IDE 支持：JetBrains/TRAE、Zed 内置及其他缺证 IDE 变体 | 后续支持阶段；不属于 M0–M7 | 启动该阶段后逐项取得本地格式证据及验证结果；当前只保留计划，不探测/开发，不阻塞首版 |

M4/M5 的本地格式证据不足不阻止 M1/M2/M6，但对应工具必须保持“待验证/受限”，
不能为了宣布计划完成而从能力矩阵删除。M7 可交付有明确支持范围的首个版本，
未解决的工具继续留在本计划。
F1 单独标为“后续支持，未实施”，不计入首版必验适配器；具体变体边界见接入矩阵。

## 已确认的实施边界与首阶段核验

- 实施阶段可只读提取本机真实 Agent 数据验证，无需重复确认这项已给出的允许；M0 已按合同完成 5 源脱敏 fixture 提取（记录见验证记录目录）。
- JetBrains/TRAE 等缺证 IDE 已后移 F1，保留支持计划，当前不实施。
- 已确认在 M0 固定 GitHub runner、工具链及 Linux 基线；WSL/WSLg 实际探测留在 M0，macOS/Linux 不承诺首发发行支持。
- 包体和性能数字均为拟定验收目标，M0 测量后才能确认；不是已有结果。

设计准备结论、数据验证步骤和未执行项见开工准备文档；以上技术核验属于实施任务，不再列为待用户决策。

## 执行约束

优先方案为 Tauri 2 + Rust + SQLite + Svelte/TypeScript + 按需 ECharts。
先实现有证据的核心，再扩展适配器；不打包 Python/Node 采集服务，不自动启动 Agent。
只读取用户启用的本地数据源；定时任务只运行本应用采集逻辑，不启动 Agent 或模型调用。
本地数据的来源边界见设计入口；仅将文件保存到本地不构成本机使用证据。
额度、会话计数、累计快照与逐次模型调用保持不同统计单位。
业务命令在 M0 从实际文件建立，不把本文列出的未来任务当作已运行记录。
