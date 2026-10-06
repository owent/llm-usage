# 最新实施与验收

最近业务验收：2026-10-06，版本 0.2.1。本文为结果索引；
当前未完成任务及执行条件统一见 [Plan.md](../../../Plan.md)，验收条件见
[V01–V31](../../design/desktop-usage/validation.md)。专项记录保留完整命令、数量、环境、
首次失败和恢复过程，本页不重复逐轮实施历史。

本机受测环境为 Windows 11 Pro x64 10.0.26300、Ryzen 9 9950X3D、约 125 GiB RAM；
Node 24.21.0、Rust 1.98、Tauri CLI 2.12.0、WebView2 154.0.4258.53，依赖以锁文件为准。
Linux 使用 WSL/Debian 独立 rootless Podman；CI runner 与受测提交另见 CI 记录。

## 当前结果

| 检查 | 已核对结果 | 记录与适用范围 |
| --- | --- | --- |
| 本机统一检查及浏览器 | verify 退出 0：Rust 1,007、前端 21、脚本 4，类型无错误/告警；Edge 浏览器回归通过 | [三源阶段](m3-container-samples.md)及 [M8 成品](m8-container-samples.md)；浏览器使用模拟 IPC |
| Windows 成品 | 无界面 11 项；原生 WebView2/IPC 17 项及 20 次首屏检查；接收器 8 项通过，自有凭据残留 0 | [三源阶段](m3-container-samples.md)；真实 IPC/SQLite/HTTP 与来源环境隔离，exporter 配置载体为合成数据 |
| Windows 安装 | NSIS 12 项升级/回滚/卸载/重装与失败中止通过 | [生命周期](installation-lifecycle.md)；仅自有任务/启动项，其他值保留 |
| Linux 成品与安装 | Debian workspace 1,004 项；deb 生命周期、实际 AppImage FUSE/GTK/Orca 共 9 组/47 项通过；此前 GTK 缩放 1/2 各 40 项独立保留 | [三源阶段](m3-container-samples.md)、[生命周期](installation-lifecycle.md)、[Orca 十语言](orca-multilang.md)；真实只读挂载及退出释放已核对 |
| 原生凭据 | Windows 处理确认后的两组各 100 轮并行、Linux 独立 D-Bus/keyring 六项及 macOS CI Keychain/HTTP 两项通过 | [跨平台凭据](platform-auth-continuation.md)、[CI](ci-plan-validation.md)；旧 Windows 撤销异常原因仍未确认，首次失败及残留检查保留 |
| 本批远端 CI/下载包 | 两轮各八作业成功；三份归档 API 摘要/CRC、四个包 SHA-256/大小及报告 revision 相符 | [CI 37486581699](https://github.com/owent/llm-usage/actions/runs/37486581699)，受测源码 61223e591815a4369a85a00fa23ff1ab2819d6d5；完整分项见 [本批 CI](ci-plan-validation.md) |
| 规模与资源 | 百万/千万未命中查询 P95 47.05/133.86 ms；百万 GUI 导入峰值 371.09 MiB；十分钟空闲均值/峰值 299.90/363.30 MiB，达到调整后的预算 | [规模测量](plan-execution.md)、[增量与原生检查](plan-finalization.md)；开发机、合成来源、全进程 private bytes，原测量与预算调整保留 |

本机 Windows/Linux 包与 CI 下载包分别核对；本机安装结果不由 CI 归档摘要推导。
文档同步提交不等于另有一次远端 CI。制品未执行发行签名、公证或 Release 发布。

## 来源与功能记录

| 范围 | 当前记录 |
| --- | --- |
| 核心存储、来源身份/聚合交换 | [M1](m1-core.md)、[M1a](m1a-provenance.md) |
| Codex、pi/oh-my-pi、Claude/Gemini/Qwen 初始读取及版本分派 | [Codex](m2a-codex.md)、[M2 恢复](m2bc-resumed.md)、[版本目录](m2d-layout-versions.md)、[CLI 安装边界](wsl-agent-installs.md)；初始记录保留当时样本状态 |
| Qwen 0.25.0 原生/SDK、OpenCode 1.18.34 | [容器来源](container-sources.md)；逐次/原生/封存分区择一，标题与后台覆盖分列 |
| Cline SDK、Hermes、OpenClaw schema 24、MiMo/Zoo/DSH | [Cline](cline-container-sample.md)、[Hermes](hermes-container-sample.md)、[OpenClaw](openclaw-container-sample.md)、[三源](m3-container-samples.md)；真实非空载体与当前成品升级/重扫已核对 |
| Kilo、Kimi、ZCode、CodeBuddy/WorkBuddy | [M3/M4](m34-kilo-zcode-kimi.md)、[Buddy 本机](m4-buddy-local.md)、[Kilo 健康/选区](trend-range-kilo.md)；CLI/扩展、独立对账与 trace 覆盖分列 |
| M8 第二批 | [18 个适配器](m8-second-batch.md)、[十源真实本地模型及成品回读](m8-container-samples.md)；其余八源条件见采样路线表，Qoder 仅探针 |
| Copilot 四面、额度与遥测 | [规则审查](m9-copilot-review.md)、[VS Code](m9-copilot-chat-local.md)、[Visual Studio](m9-vs-copilot-local.md)、[JetBrains 静态分析](m9-jb-copilot-analysis.md)、[配置入口](telemetry-setup-ui.md) |
| 看板、调度、取消、并行及旧规则恢复 | [原生与规模](plan-finalization.md)、[查询与后台](plan-execution.md)、[完整旧摘要](parser-conflict-fix.md)、[来源规则升级](source-policy-upgrades.md) |
| 费用引擎、在线刷新、当前 API 参考及归档修正 | [引擎](f2-cost-engine.md)、[刷新](f2-online-refresh.md)、[当前参考](dashboard-reference.md)、[归档](pricing-archive-repair.md)；真实数据发生时估算仍有渠道前置条件 |

## 验收边界

- 真实来源结论限于受测版本、原生载体和非空场景；未知字段、未保存调用与累计区间保留，不补零或扩展产品能力。完整支持范围见 [接入矩阵](../../design/desktop-usage/adapters.md)。
- 原生 DPI 144/UI Automation 名称、GTK 缩放 1/2 与 Orca 十语言五页导航各自有效；ALSA null 不认证物理音频或全部控件，控件名称不代替 Narrator/NVDA 完整使用。
- Linux 容器为普通应用用户、默认 seccomp、无网络/宿主挂载；FUSE 仅对自有容器显式加 SYS_ADMIN。结果不认证宿主登录/注销、完整桌面或其他发行版。
- macOS 原生凭据与构建 CI 已通过；macOS 桌面、特定硬件不在本轮要求内。
- 合成规模库与百万 GUI 导入不认证其他真实来源；应用调度循环不等于 OS 唤醒，暖空页不等于全新 WebView 最低开销，协作式中断不能强制取消 OS 阻塞读取。
- 已观测 WebView 请求无外部 HTTP 不等于全进程出站审计；接收器/凭据测试不认证其他真实 exporter。

临时日志、核对库和原始载体均位于仓库根已忽略的 build/ 各任务目录；
公开记录保留最小脱敏结果，自有凭据、keyring 与守护进程按各次验收回收。
