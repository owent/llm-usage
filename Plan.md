# 桌面用量客户端执行计划

当前版本 0.2.1，Windows 11 x64 首发，保留 Windows/Linux/macOS CI。
本轮不要求 macOS 桌面或特定硬件；Linux 实际 GUI/包生命周期接受 WSL/Debian
独立 Podman 验收，容器结果不扩大为宿主系统集成。

本文件维护当前状态、未完成任务和执行条件。已完成的命令、数字与首次失败保留在
[最新验收](docs/validation/desktop-usage/current-acceptance.md)及专项记录；行为以设计和源码为准。
设计入口：[产品与架构](docs/design/desktop-usage/README.md)、[交付要求](docs/design/desktop-usage/execution.md)、
[数据规则](docs/design/desktop-usage/data-contract.md)、[接入矩阵](docs/design/desktop-usage/adapters.md)、
[验收清单 V01–V31](docs/design/desktop-usage/validation.md)。

## 当前进度

| 阶段 | 已完成范围与记录 |
| --- | --- |
| M0/M1/M1a | 工程基线、SQLite 事务/恢复/统计/备份、来源身份及聚合交换；[核心](docs/validation/desktop-usage/m1-core.md)、[来源交换](docs/validation/desktop-usage/m1a-provenance.md) |
| M2–M5/M8/M9 | 已注册来源解析与本机遥测/接收认证；真实格式标签限于受测版本和载体。M8 注册 18 个适配器（17 解析 + Qoder 探针），其中十源已取得真实本地模型用量；逐源状态见接入矩阵及最新验收 |
| M6/F3 | 五页、图表选区、保留/导出、逐源计划、暂停/取消、两来源并行、Windows 托盘/节能/通知及十语言；[交互与调度](docs/validation/desktop-usage/plan-finalization.md)、[Linux Orca](docs/validation/desktop-usage/orca-multilang.md) |
| M7 | Windows NSIS 12 项、Linux deb/AppImage/FUSE/GTK/Orca 验收通过；既有规模与资源测量达标；本批两轮三平台 CI 各八作业成功，三份下载归档及四个包校验通过，macOS 原生凭据两项通过；[安装](docs/validation/desktop-usage/installation-lifecycle.md)、[规模](docs/validation/desktop-usage/plan-execution.md)、[本批 CI](docs/validation/desktop-usage/ci-plan-validation.md) |
| F2 | 默认关闭的费用引擎、价格快照、在线缓存/失败回退及官方 API 参考；[费用](docs/validation/desktop-usage/f2-cost-engine.md)、[在线刷新](docs/validation/desktop-usage/f2-online-refresh.md)、[参考与归档修正](docs/validation/desktop-usage/pricing-archive-repair.md) |

## 剩余任务与完成条件

优先核对已授权的本机既有数据和可离线运行的官方客户端。取得真实样本后，再核对
原始字段、来源归属、独立汇总、旧库升级和重复读取；缺少账户、分发物或格式依据的项
保留具体条件。完整用例要求见验收清单，不以安装成功、空会话或合成数据结束真实验收。

### 来源与真实样本

| ID | 未完成范围 | 下一步与条件 |
| --- | --- | --- |
| M2 | Claude Code、Gemini 非空真实用量；Qwen 其他版本/供应商/云端及主循环缓存命中 | 先只读寻找原生载体。Claude 未登录元数据不认证用量，Qwen 0.25.0 原生/SDK 已完成；[M2 记录](docs/validation/desktop-usage/m2bc-resumed.md)、[CLI 安装边界](docs/validation/desktop-usage/wsl-agent-installs.md)、[容器来源](docs/validation/desktop-usage/container-sources.md) |
| M3 | Cline 旧 UI/迁移/其他 SDK/CLI；MiMo 其他版本/协议/非截断任务；Zoo CLI/删除/压缩；DSH 真实 seed/retry/fail/其他协议；Hermes gateway/辅助/混合历史；OpenClaw 冷归档/gateway/其他协议 | 按各源原生 writer 与公开入口取得对应样本，分别验收；OpenCode/DSH 标题未落盘属于覆盖缺口，须先核验是否有原生补充载体，不能补造调用；[三源](docs/validation/desktop-usage/m3-container-samples.md)、[最新来源记录](docs/validation/desktop-usage/current-acceptance.md) |
| M4/M5 | Kimi Code/Work、ZCode、WorkBuddy 的 trace/子 Agent/跨版本/历史保留；CodeBuddy CLI 原生会话及扩展更多字段 | CodeBuddy 扩展真实用量已核对，CLI JSONL 尚无真实用量；会话/遥测重叠须择一或取得可靠关联，不能相加；[Buddy 记录](docs/validation/desktop-usage/m4-buddy-local.md) |
| M8 | Amp、Qoder、Antigravity、Zed、Kiro、Command Code、Droid、Grok Build 八个产品的非空真实样本 | 逐面核验账户、本地协议与官方分发物；Qoder 仍仅探针，字段核验后才实施解析。Command Code 已实际无账户拒绝，Droid Airgap 包未公开；Zed 本地模型不认证当前 hosted 载体，Grok 官方入口读取失败不证明不支持；[采样限制](docs/validation/desktop-usage/m8-container-samples.md#剩余本地采样路线核查) |
| M2–M4/M8/V17/V30 | 已采样来源的其他版本、工具循环/模型切换/失败/取消/辅助请求，以及尚未核验的历史格式 | 按 [逐源样本要求](docs/design/desktop-usage/adapters.md) 分别登记，不能从一个版本推广；Codex 旧版本及 OMP_PROFILE 等原有条件见 [M2 版本](docs/validation/desktop-usage/m2d-layout-versions.md)、M2 记录；WSL UNC SQLite 快照限制按 CLI 安装记录单列 |

### 遥测与安全

| ID | 未完成范围 | 下一步与条件 |
| --- | --- | --- |
| M5/M9/V08/V10 | 其他真实 exporter/版本的认证配置；Copilot 新 CLI、JetBrains OTel file；真实重传/采样/父子 span 与跨载体关联 | 核验实际导出端及非空原生载体，逐字段比较并回归来源择一；接收器合成载荷/凭据往返不认证真实 exporter；[遥测配置](docs/design/desktop-usage/copilot-otel.md)、[Copilot 审查](docs/validation/desktop-usage/m9-copilot-review.md) |
| M5/V22 | Windows 旧跨进程撤销异常的原因 | 保留首次失败，核对跨进程时序及凭据 API；写后回读/删除确认已修正、处理确认后 200 轮并行通过、自有残留 0，仍未确认旧异常原因；[凭据记录](docs/validation/desktop-usage/platform-auth-continuation.md) |
| M5/V22/V25 | 全进程出站网络审计及安全/归属剩余场景 | 覆盖运行、手动、定时、导入路径和应用子进程，分开记录构建下载与统计运行；已观测 WebView 请求不足以结束该验收 |
| M9 | Copilot trace SQLite 解析 | 固定 schema、属性正文及非空本机样本核对后才能实施；同一产品面的原生与遥测择一，不按时间/token 相等猜调用身份 |

### 桌面与系统

| ID | 未完成范围 | 下一步与条件 |
| --- | --- | --- |
| M6/F3/V18/V31 | Windows 其他原生 DPI、Narrator/NVDA 完整使用；Linux 其余控件、语言发音及完整辅助技术使用 | DPI 144/UI Automation 名称和 GTK 缩放 1/2、Orca 十语言五页导航已通过；继续用真实辅助技术逐操作检查，ALSA null 不认证物理音频 |
| M6/M7/V23/V24 | 宿主登录/注销及完整系统集成 | 不自动注销或重启宿主；需独立会话环境。Linux 容器包/GUI 不认证宿主托盘/通知、登录会话或其他发行版 |
| M7/V20/V21 | OS 全部唤醒测量；全新最小 WebView 页面开销诊断 | 应用调度循环不等于 OS 唤醒；成品规模/内存验收已通过，暖空页不认证全新页面最低开销。最小页诊断单列，不恢复已取消的特定硬件要求；协作式中断不能强制取消 OS 阻塞读取 |

## 后置功能与发布条件

| ID | 状态与条件 |
| --- | --- |
| M1a | 明细级导入和完整 Merge 未排期；现有聚合交换已完成 |
| F2 | 预算提醒后置；真实数据的发生时估算须先核验/配置供应商渠道，再只读比较。当前 API 参考核对已完成，不推断实付；[价格设计](docs/design/desktop-usage/pricing.md) |
| F1 | 本地用量格式尚未核验的 IDE/插件未排期，范围以接入矩阵为准；Amazon Q/Codebuff 无已核验逐次载体、iFlow 停服及远端账单路线为排除项 |
| M7/V26/V27 | 本批测试分支 CI 与制品核验已完成；合并主分支、发行签名/公证和 Release 尚未执行，需相应授权。后续代码变更持续验收，不把文档提交当作新的远端 CI |

## 执行边界

- 深度核对源码、配置、测试及版本依据后执行；按需同步 AI 提示词、Skills、受影响设计和计划。
- 只统计本机 Agent 来源；未知不补零，调用/消息/累计/额度分开，费用默认关闭、多币种不合并。
- 既有本机真实数据只读提取已授权，按 [准备规则](docs/design/desktop-usage/implementation-readiness.md) 白名单与脱敏执行，无需重复询问。
- 独立 WSL/Debian Podman、真实本地模型样本及 Windows 安装生命周期已授权；不修改真实 IDE 配置，不自动登录个人账户或产生云端付费请求。
- 保留用户修改，临时产物放根 build/；独立测试分支提交/推送/CI 已授权，不自动合并、部署或发布。
- 适配器按记录所属版本保留依据；定向路由和物理文件去重、旧处理位置重评、修订/历史/封存规则以数据设计为准。只在实际检查并记录结果后更新状态。
