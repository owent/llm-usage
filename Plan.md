# 桌面用量客户端执行计划

按本机 Agent、模型和时间汇总 token、已观测调用与缓存使用，提供今日刷新、
日/周/月图表、定时提取、保留、导出和设置。Windows 11 x64 首发；
macOS/Linux 保留 CI，WSL 构建不代替原生桌面验收。当前版本 0.2.0。

只读本机来源，不接入远端用量/账单 API 或跨设备账号报表。未知用量不补零；
调用、消息、累计值和额度分开。依赖使用浮动版本范围，锁文件保证可复现。
本文件只保留当前进度、剩余工作和执行边界；行为以设计及源码为准。

**请确保深度思考调研后再执行，禁止猜测。按需更新 AI agent 提示词、Skills 和相关文档，及时更新完成进度。**

## 设计入口

| 主题 | 最新设计 |
| --- | --- |
| 产品与工程 | [范围](docs/design/desktop-usage/README.md)、[架构](docs/design/desktop-usage/architecture.md)、[交付要求](docs/design/desktop-usage/execution.md) |
| 采集与统计 | [数据规则](docs/design/desktop-usage/data-contract.md)、[接入矩阵](docs/design/desktop-usage/adapters.md)、[本机数据验证](docs/design/desktop-usage/implementation-readiness.md) |
| 自动提取 | [调度与后台](docs/design/desktop-usage/scheduling.md)、[Copilot 遥测配置](docs/design/desktop-usage/copilot-otel.md) |
| 看板与费用 | [交互](docs/design/desktop-usage/dashboard-polish.md)、[统计修正](docs/design/desktop-usage/dashboard-repair.md)、[价格](docs/design/desktop-usage/pricing.md)、[语言](docs/design/desktop-usage/i18n.md) |
| 验收与来源 | [验收清单](docs/design/desktop-usage/validation.md)、[平台与 CI](docs/design/desktop-usage/platform-ci.md)、[研究依据](docs/design/desktop-usage/research.md) |

## 当前进度

流式日汇总/图表、明细覆盖索引、有容量上限的连接缓存及稳定归档跳过已通过完整检查
（Rust 852）。百万事件库上 20 轮新增 1,000 条至界面更新 P95 1.39 秒，预算通过；
最终 release 的原生资源、取消及交互回归已完成。

清理事务取消、系统任务注册/删除失败路径、无 GUI 的分钟采集，以及十语言、
键盘选区和 100%–200% CSS 页面缩放已完成实现及原生核对。百万/千万事件已测量；
366 日未命中缓存查询 P95 分别为 965 ms / 4,178 ms，命中为 0.033 / 0.049 ms；
未命中查询仍未达到 200 ms 目标。
百万事件 20 次原生首屏 P95 1.22 秒；连续 601 秒空闲 CPU 为单核 0.31%，
全进程 private bytes 均值 334 MiB / 峰值 484 MiB，超过 180 MiB 目标。

看板选区已完成：自然日文案明确；趋势三图、总览历史和今日小时图支持横向拖选，
汇总、模型/Agent 分布及模型表按同范围联动，不联动的热力图与周分布置后。
Kilo 独立快照差异只作对账，逐条错误与未知版本兼容分开；旧水位自动重评、
真实统计库修复及幂等验证通过，用量和来源字节不变，见
[选区与健康验收](docs/validation/desktop-usage/trend-range-kilo.md)。
解析器升级须验证完整旧摘要，仅更新解析依据；真实冲突和历史保留，见
[元数据合同验收](docs/validation/desktop-usage/parser-conflict-fix.md)。

本地任务已支持持久化意图/期限、单写者、暂停、独立来源规则、Windows 分钟任务
及实际状态核对。定点时区独立，DST 缺失顺延、重复只执行首次。
当前完整检查通过（Rust 852、前端 20、脚本 3）；Windows NSIS 3.63 MiB，
真实无界面 11 项、原生 IPC/鼠标与键盘选区、十语言和后台触发共 13 项通过；
20 次合成小数据首屏 P95 724 ms。详细结果集中到
[最新验收](docs/validation/desktop-usage/current-acceptance.md)。

| ID | 当前交付 | 剩余工作 |
| --- | --- | --- |
| M0 | 开发合同、版本样本及三平台 CI 基线已完成 | 持续 CI 归 M7 |
| M1 | SQLite、事务/恢复、去重、统计、迁移前一致备份及空间检查已完成 | 无核心实现待办；性能/桌面证据见 M6/M7 |
| M1a | 主机/来源身份、分区与版本化交换已完成；聚合交换可导入 | 明细级导入和完整 Merge 后置 |
| M2 | Codex、Claude Code、pi、oh-my-pi、Gemini、Qwen 六源已实现 | Gemini/Qwen 非空真实用量样本 |
| M3 | Cline、Kilo、OpenCode、MiMo、Zoo、DSH、OpenClaw、Hermes 已实现 | 接入矩阵中仍标文档级的产品补真实样本 |
| M4 | Kimi Code/Work、ZCode、WorkBuddy 已实现并有本机核对 | trace/子 Agent 覆盖和跨版本核对 |
| M5 | Copilot CLI、CodeBuddy、本机 OTel file 与 loopback traces/logs 接收已实现 | 新版 CLI 真实导出、其他隔离输出的统计关联、本地/OTLP 重叠及 V08/V22/V25 剩余场景 |
| M6 | 五页、查询/图表、逐源计划、保留/导出、事务取消及十语言已实现；原生 IPC/离线/重启/键盘/页面缩放、无 GUI 分钟采集及失败注入通过 | OS DPI/辅助技术与完整生命周期；其余差异见下表 |
| M7 | 最新 Windows release/NSIS、小数据/百万事件各 20 次首屏、完整 10 分钟全进程资源、百万/千万查询及 20 轮增量刷新测量；WSL 编译和三平台 CI 基线已有证据 | 大规模查询与内存未达标；当前索引下完整导入、升级回滚、拟定基准和三平台持续验收，安装按既有指示跳过 |
| M8 | 第二批 18 适配器（17 解析 + Qoder 探针）已注册并有合成回归 | 非空真实样本；不扩展缺证产品能力 |
| M9 | CLI、VS Code、Visual Studio 用量与独立 premium 额度已接入 | JetBrains file/new CLI 真实验收、trace SQLite；同面原生与遥测择一 |
| F1 | 缺证 IDE/插件未排期 | 当前不探测/实施；范围以接入矩阵为准 |
| F2 | 默认关闭的费用引擎、价格快照、在线缓存/失败回退及官方 API 参考已实施 | 预算提醒后置 |
| F3 | 十语言实现及真实 IPC 保存、可访问名称、100%–200% 页面缩放通过，统计不随语言变化 | 原生 OS DPI、其他平台与辅助技术实测随 M6 |

## 剩余任务与完成条件

| 优先级 / ID | 工作 | 完成条件与依赖 |
| --- | --- | --- |
| 后续 / M6/M7/V23/V24 | 注销/升级/卸载生命周期 | 注册/删除失败注入和无 GUI 的真实分钟采集已补齐；安装生命周期仍缺证 |
| 当前 / M6/M7/V18/V20/V21/V31 | 查询/内存优化、OS DPI 与辅助技术 | 百万库新增 1,000 条至 UI 更新 P95 1.39 秒通过；查询和内存仍未达标，拟定基准/导入峰值/唤醒次数仍缺证 |
| 后续 / M5/M9/V08/V22/V25 | 遥测载体与来源边界 | 独立版本/本机依据；重传、采样、压缩上限、敏感字段、父子 span 和跨载体不双计；未核验新输出继续隔离 |
| 后续 / M2–M4/M8 | 缺失真实样本 | 数据已存在且能安全只读时提取白名单，独立核对明细/汇总和重扫；不启动 Agent 制造样本 |
| 后续 / M6 | 托盘退出、节能暂停、文件监听、单源时间/重试限制 | 先形成与现实现状一致的可审阅设计再实施；监听为优化项，轮询可先行 |
| 后续 / M7/V19/V21/V26/V27 | 发布候选与平台 | 最新资源/包体、升级回滚及原生平台证据；CI 推送、发布/签名另需相应授权 |
| 后置 / M1a/F2/F1 | 明细 Merge、预算、缺证 IDE | 不作为已完成主线的阻塞；仍须逐项依据和单独验收 |

## 执行边界

- 本机真实数据开发验证已获允许；按准备文档只读、白名单和脱敏，不再次询问同一许可。
- Tauri 2 + Rust + SQLite + Svelte/TypeScript + 按需 ECharts；不打包 Node/Python 服务。
- 不自动启动 Agent、WSL/容器或模型调用，不修改真实 IDE 配置来制造样本。
- 所有适配器独立目录及版本注册表；未知版本先兼容读取，依据按记录所属版本保留。
- 应用管理的根定向路由；同一物理文件不重复登记；旧游标重评保留修订、历史和封存。
- 费用默认关闭，多币种不合并；参考价不推断实付，缺价/型号/渠道仍保留未知。
- 保留用户已有修改；临时产物放根 build/。不自动提交、推送、部署或发布。
- 只在实际证据取得后更新验收状态；无数据、未执行和通过分别记录。
