# 桌面用量客户端执行计划

按本机 Agent、模型和时间汇总 token、已观测调用与缓存使用，提供今日刷新、
日/周/月图表、定时提取、保留、导出和设置。Windows 11 x64 首发；
macOS/Linux 保留 CI，WSL 构建不代替原生桌面验收。当前版本 0.2.1。
2026-10-05 用户调整验收范围：不要求 macOS 桌面或特定硬件；Linux 桌面与
安装生命周期可在 WSL/Debian 的独立 Podman 环境验证，Windows 本机安装验收已授权。
容器实际 GUI、软件包生命周期、宿主系统集成和真实来源样本分别记录。

只读本机来源，不接入远端用量/账单 API 或跨设备账号报表。未知用量不补零；
调用、消息、累计值和额度分开。依赖使用浮动版本范围，锁文件保证可复现。
本文件只保留当前进度、剩余工作和执行边界；行为以设计及源码为准。

**请确保深度思考调研后再执行，禁止猜测。按需更新 AI agent 提示词、Skills 和相关文档，及时更新完成进度。**

## 设计入口

| 主题 | 最新设计 |
| --- | --- |
| 产品与工程 | [范围](docs/design/desktop-usage/README.md)、[架构](docs/design/desktop-usage/architecture.md)、[交付要求](docs/design/desktop-usage/execution.md) |
| 采集与统计 | [数据规则](docs/design/desktop-usage/data-contract.md)、[接入矩阵](docs/design/desktop-usage/adapters.md)、[本机数据验证](docs/design/desktop-usage/implementation-readiness.md) |
| 自动提取 | [调度与后台](docs/design/desktop-usage/scheduling.md)、[Copilot 遥测配置](docs/design/desktop-usage/copilot-otel.md)、[本机接收认证](docs/design/desktop-usage/receiver-auth.md) |
| 看板与费用 | [交互](docs/design/desktop-usage/dashboard-polish.md)、[统计修正](docs/design/desktop-usage/dashboard-repair.md)、[价格](docs/design/desktop-usage/pricing.md)、[语言](docs/design/desktop-usage/i18n.md) |
| 验收与来源 | [验收清单](docs/design/desktop-usage/validation.md)、[平台与 CI](docs/design/desktop-usage/platform-ci.md)、[安装生命周期](docs/design/desktop-usage/installation-lifecycle.md)、[研究依据](docs/design/desktop-usage/research.md) |

## 当前进度

价格反馈修复已实施：统一明细/归档计价范围、汇总后舍入、补充有依据的型号匹配，
同来源 provider/模型合并一行，多档归档显示区间。真实脱敏日汇总复算一致；
当前根锁文件的完整验证及 Windows release/原生回归已通过。
用户确认 k28-agent-preview 为 K2.8 Preview 后，
已补明确授权的 K2.7 Code 当前价替代参考，保留型号身份与替代标记；本型号价目优先。
根因、官方来源和验证边界见 [价格修复记录](docs/validation/desktop-usage/pricing-archive-repair.md)。

本轮已补日查询及供应商筛选加速、模型/Agent 表达式索引、SVG 图表、Windows
托盘退出/关闭隐藏、节能暂停、文件通知、逐源单调计时、协作式时间/重试限制，
以及遥测精确白名单、真实 HTTP 压缩边界和并发/请求限流。
供应商跨日/跨来源会话、未知值、旧布局、旧写者、回滚及保留清理已有回归。
此前基线完整检查已通过（Rust 889、前端 20、脚本 3），Windows release/NSIS 和无界面
11 项通过，原生 17 项、浏览器及百万库规模复测通过。详细记录见
[本轮验收](docs/validation/desktop-usage/plan-execution.md) 和
[查询加速设计](docs/design/desktop-usage/query-acceleration.md)。

366 日、50 模型、20 Agent 的百万/千万事件库上，未命中缓存查询 P95 为
47.1 / 133.9 ms；模型、Agent、供应商及组合筛选也分别实测，最高为 196.6 ms，
当前开发机上达到 200 ms 目标。首次旧库补建约 8.6 / 49 秒，空间成本计入验收。
当前内核首次导入 100 万标准化事件用时 184.5 秒，全进程 private bytes 峰值
53.4 MiB；这项无 WebView 的导入测量不代替 GUI 导入峰值验收。
百万库 20 次首屏 P95 1.26 秒，20 轮新增 1,000 条至界面更新 P95 718 ms；
连续 601 秒空闲 CPU 为单核 0.244%，调度循环 1,202 次。全进程内存均值
299.9 / 峰值 363.3 MiB；按实测与 WebView2 依据将空闲预算调整为 10 分钟均值
≤350 MiB、采样峰值 ≤400 MiB，开发机达到新预算。GPU 均值 146.2 MiB、主程序 17.0 MiB，
暖 WebView 导航空页的完整 600 秒诊断仍为均值 270.6 MiB（GPU 148.1 MiB），
全新空页基线未测，特定硬件要求已取消；GUI 百万合成 Codex 首次回填到界面更新 668.6 秒，
全进程采样峰值 371.1 MiB，达到调整后的 ≤512 MiB 导入预算，重扫保持幂等。

看板选区已完成：自然日文案明确；趋势三图、总览历史和今日小时图支持横向拖选，
汇总、模型/Agent 分布及模型表按同范围联动，不联动的热力图与周分布置后。
Kilo 独立快照差异只作对账，逐条错误与未知版本兼容分开；旧水位自动重评、
真实统计库修复及幂等验证通过，用量和来源字节不变，见
[选区与健康验收](docs/validation/desktop-usage/trend-range-kilo.md)。
解析器升级须验证完整旧摘要，仅更新解析依据；真实冲突和历史保留，见
[元数据合同验收](docs/validation/desktop-usage/parser-conflict-fix.md)。

本地任务已支持持久化意图/期限、单写者、暂停、独立来源规则、Windows 分钟任务
及实际状态核对。定点时区独立，DST 缺失顺延、重复只执行首次。
Windows 托盘、文件监听和暂停回归已补；保存暂停意图在等待数据库锁前生效，
重试等待和再次读取均检查暂停，失败保留游标。完整结果集中到
[最新验收](docs/validation/desktop-usage/current-acceptance.md)。

继续执行已完成两个来源实例槽、载体内协作中断及保留/费用控制。不同根可并行，
同源合并、单写者保留；读取窗续读、中断/未访问的到期规则、备份分页和归档恢复
专项通过。文件指纹/代数已与事件/游标同事务，失败后的同大小替换可恢复重读。
完整验证与 Windows release/原生复测已通过。
Windows Claude/Codex logs 已加入逐源认证与当前用户凭据库；缺凭据拒绝接收，
预览不回传秘密，失败回收、撤销吊销和另一来源继续接收均通过真实 IPC/HTTP 验证。
已从官方发布记录核验 CodeBuddy CLI 2.98.0 的 generic headers；按首个 launcher
同路径 manifest 限定配置，其他版本不套用。完整检查、release 与真实原生接收验收
8 项已通过，三来源精确撤销后自有凭据残留 0；不代替真实 exporter 导出验收。
Linux Secret Service 与 macOS Keychain 已实施：Linux 默认持久集合、DH 加密、
3 秒操作期限和锁定/重复/临时集合（含默认别名指向 session）拒绝已在 WSL Debian 真实验证；macOS 禁用云同步与
认证 UI，交叉类型检查通过，原生验收保留。Windows/Linux 的跨进程读取和真实
HTTP 撤销/来源隔离通过，错误写入回复会精确回收已保存的自有项。
三平台 Rust CI 作业已补齐，制品报告/上传排除 Debian/AppImage 暂存文件。
此前 Windows 统一检查 Rust 904、前端 21、脚本 4 项通过，release/NSIS、无界面 11 项、
原生桌面 17 项、接收 8 项及浏览器通过；Linux 核心 808、应用 95、原生凭据 6 项通过，
release/deb/AppImage 及 ELF/提取 AppImage 各 11 项无界面回归通过。
凭据范围与剩余条件见 [跨平台执行记录](docs/validation/desktop-usage/platform-auth-continuation.md)。

Windows 本机 NSIS 生命周期 12 项通过；已修复卸载遗留自有分钟任务，并验收
运行中写者阻止卸载、其他任务/启动项保留及重装后不恢复后台意图。Debian rootless
Podman 实际 deb 安装/升级/回滚/卸载/重装/清除和 GTK/WebKit GUI 通过；AppImage
提取运行通过；FUSE 已定位为容器能力限制，显式添加 SYS_ADMIN 后仍以普通用户/
默认 seccomp 完成实际只读挂载、GUI/IPC 与退出卸载。GTK 窗口缩放 1/2 对照各 8 组
40 项通过，实际 WebKit 比例与截图、五页布局均核对；无需修改 WSL 全局配置或使用
rootful Podman。可复用命令、包摘要与边界见
[安装验收](docs/validation/desktop-usage/installation-lifecycle.md)。
Qwen Code 0.25.0 已在无网络容器内调用真实本地模型，主循环字段与 CLI/模型服务/
SQLite 一致，重扫幂等并保存脱敏回归。默认自动记忆提取的另一调用未进入会话载体，
该覆盖缺口保留；关闭后台记忆的对照样本完整一致。后续真实 file 导出已找到
两条逐次 API response/LLM span，含后台缓存读；连续多行 SDK JSON 与现有 JSONL
合同不同，尚未接入统计，见
[容器来源验收](docs/validation/desktop-usage/container-sources.md)。OpenCode 1.18.34 真实
主循环及缓存读已与 API/CLI/原生库/应用核对，明确标题对照为 1 次/299 token；
默认标题场景 API 2 次/848 token、原生载体与应用 1 次/299，覆盖缺口保留。
两种脱敏 fixture 的 Windows/Debian 合同各 8 项通过；逐记录版本与旧游标升级尚未
验收，仍用 latest_fallback，未直接注册整个库的最高版本。
最终统一检查 Rust 908（核心 810、应用 98）、前端 21、脚本 4 全部通过；
包含最新能力说明的 Windows release/NSIS、Linux deb/AppImage 已重建，Windows 12 项、
Linux 缩放 1/2 各 40 项生命周期复验通过；新 deb 回读保存的 Qwen/OpenCode 真实载体
仍保持原用量、重扫幂等及更新后的覆盖说明。

| ID | 当前交付 | 剩余工作 |
| --- | --- | --- |
| M0 | 开发合同、版本样本及三平台 CI 基线已完成 | 持续 CI 归 M7 |
| M1 | SQLite、事务/恢复、去重、统计、迁移前一致备份及空间检查已完成 | 无核心实现待办；性能/桌面验证结果见 M6/M7 |
| M1a | 主机/来源身份、分区与版本化交换已完成；聚合交换可导入 | 明细级导入和完整 Merge 后置 |
| M2 | Codex、Claude Code、pi、oh-my-pi、Gemini、Qwen 六源已实现；Qwen 0.25.0 本地主循环样本/重扫通过，真实 file 遥测已核对主循环与后台两条调用及后台缓存读 | Gemini 非空真实样本；Qwen 多行 SDK JSON 解析/权威分区、其他版本/云端/主循环缓存命中 |
| M3 | Cline、Kilo、OpenCode、MiMo、Zoo、DSH、OpenClaw、Hermes 已实现；OpenCode 1.18.34 本地主循环/缓存读的真实兼容样本及重扫通过 | OpenCode 标题载体/逐记录版本与旧游标升级；仍标文档级产品的真实样本 |
| M4 | Kimi Code/Work、ZCode、WorkBuddy 已实现并有本机核对 | trace/子 Agent 覆盖和跨版本核对 |
| M5 | 本机 OTel file、HTTP 协议/字段/压缩/限流及 Claude/Codex logs、CodeBuddy CLI 2.98.0 隔离 traces 逐源认证已实现；Qwen 0.25.0 真实 file 字段已核对；Windows/Linux 原生凭据往返通过，macOS Keychain 已实现并交叉类型检查 | Windows 一次并行存储失败及遗留自有项（已回收）的原因、macOS 原生凭据、其他发送端/版本认证依据、Copilot 新 CLI 等真实导出、Qwen 多行 SDK JSON 解析/权威分区、重传/采样/父子 span、跨载体关联及 V08/V22/V25 剩余场景 |
| M6 | 五页、逐源计划、保留/导出、取消、十语言及 Windows 托盘/节能/文件通知、并行 2 来源和载体内协作中断已验收；Windows 安装与 Debian GTK 缩放 1/2 GUI 通过 | Windows 其他原生 DPI/真实辅助技术、宿主注销/登录与系统集成 |
| M7 | Windows NSIS 12 项生命周期、Debian Podman 提取 32 项及最终 FUSE/GTK 缩放 1/2 各 40 项通过；FUSE 挂载与退出释放均核对；三平台 Rust CI 已配置；既有规模/资源测量达标 | 三平台远端持续验收；macOS 桌面和特定硬件已取消要求 |
| M8 | 第二批 18 适配器（17 解析 + Qoder 探针）已注册并有合成回归 | 非空真实样本；不扩展尚未核验的产品能力 |
| M9 | CLI、VS Code、Visual Studio 用量与独立 premium 额度已接入 | JetBrains file/new CLI 真实验收；trace SQLite 需固定 schema/属性正文与非空样本核对后实施，同面原生与遥测择一 |
| F1 | 本地用量格式尚未核验的 IDE/插件未排期 | 当前不探测/实施；范围以接入矩阵为准 |
| F2 | 默认关闭的费用引擎、价格快照、在线缓存/失败回退及官方 API 参考已实施 | 预算提醒后置 |
| F3 | 十语言保存、可访问名称、100%–200% 页面缩放及 Windows DPI 144 的 UI Automation 名称检查通过；Linux GTK 缩放 1/2 的真实 WebKit/截图/五页布局通过，统计不随语言变化 | Windows 其他 DPI 与 Narrator/NVDA、Linux 屏幕阅读器实测随 M6；测试辅助技术标志不作为成品默认 |

## 剩余任务与完成条件

| 优先级 / ID | 工作 | 完成条件与依赖 |
| --- | --- | --- |
| 后续 / M6/M7/V23/V24 | 宿主登录/注销与系统集成 | Windows 安装生命周期及 Debian 容器软件包/GUI、AppImage FUSE 已通过；注销不能通过退出应用替代，不自动注销宿主用户 |
| 后续 / M6/M7/V18/V20/V21/V31 | 资源与辅助技术 | 开发机既有规模/资源结果保留；特定硬件已取消要求；更多 DPI/真实辅助技术及 OS 全部唤醒单独记录 |
| 后续 / M5/M9/V08/V22/V25 | 遥测认证、载体与来源边界 | Windows/Linux 原生存储与 HTTP 通过，macOS Keychain 已实现/交叉类型检查；Windows 并行失败后回收可靠性、macOS 原生、其他 exporter/版本的认证依据、真实重传/采样/父子 span、跨载体及全进程出站审计仍未完成；未知输出继续隔离 |
| 后续 / M2–M4/M8 | 缺失真实样本 | 优先安全只读提取既有数据；本轮允许独立容器内官方客户端调用本地模型，先核验支持合同、版本和载体，再独立核对字段/汇总/重扫；不使用个人账户或收费请求补样本 |
| 后续 / M7/V19/V21/V26/V27 | 发布候选与平台 | 最新资源/包体、升级回滚及原生平台验证结果；CI 推送、发布/签名另需相应授权 |
| 后置 / M1a/F2/F1 | 明细 Merge、预算、本地用量格式尚未核验的 IDE | 不作为已完成主线的阻塞；仍须逐项依据和单独验收 |

## 执行边界

- 本机真实数据开发验证已获允许；按准备文档只读、白名单和脱敏，不再次询问同一许可。
- Tauri 2 + Rust + SQLite + Svelte/TypeScript + 按需 ECharts；不打包 Node/Python 服务。
- 本轮已授权 WSL/Debian Podman 测试环境、隔离本地模型样本与 Windows 本机安装生命周期；
  不修改真实 IDE 配置，不推断容器安装已产生非空用量；未授权个人账户登录或云端付费请求。
- 所有适配器独立目录及版本注册表；未知版本先兼容读取，依据按记录所属版本保留。
- 应用管理的根定向路由；同一物理文件不重复登记；旧游标重评保留修订、历史和封存。
- 费用默认关闭，多币种不合并；参考价不推断实付，缺价/型号/渠道仍保留未知。
- 保留用户已有修改；临时产物放根 build/。不自动提交、推送、部署或发布。
- 只在完成对应检查并记录结果后更新验收状态；无数据、未执行和通过分别记录。
