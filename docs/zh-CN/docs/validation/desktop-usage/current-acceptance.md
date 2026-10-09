# 最新实施与验收

<a id="latest-implementation-and-acceptance-results"></a>

最近完整业务验收：2026-10-07，版本 0.2.1；portable 草稿发布：2026-10-08，版本 0.2.2。
本文为结果索引；
当前范围及移出条件统一见 [Plan.md](../../../Plan.md)，验收条件见
[V01–V31](../../design/desktop-usage/validation.md)。专项记录保留完整命令、数量、环境、
首次失败和恢复过程，本页不重复逐轮实施历史。

本机受测环境为 Windows 11 Pro x64 10.0.26300、Ryzen 9 9950X3D、约 125 GiB RAM；
Node 24.21.0、Rust 1.98、Tauri CLI 2.12.0、WebView2 154.0.4258.53，依赖以锁文件为准。
Linux 使用 WSL/Debian 独立 rootless Podman；CI runner 与受测提交另见 CI 记录。

<a id="current-results"></a>

## 当前结果

| 检查 | 已核对结果 | 记录与适用范围 |
| --- | --- | --- |
| 检查软件更新 | 开发模式检查不再要求安装类型；GitHub 请求失败后可使用有效期受限的站点快照，沿用相同包校验 | [检查更新修复](update-check-repair.md)；独立原生开发模式 IPC 与本地回退测试，部署观察分列 |
| 0.3.0 本地修复 | 费用卡片无溢出，设置使用自定义控件，登记八个 Codex 原生版本并修正压缩携带记录为空的处理；旧库副本的 332 个 Codex 文件恢复正常且原有用量完整保留 | [0.3.0 检查](release-030.md)；Windows 本地构建、合成更新替换和原生数据副本检查与远程发布/平台验收分列 |
| 软件更新 | 本机实现每次启动/每天/每周/手动检查、可选自动下载、共用进度及显式安装；合成更新已验证 Windows 原生便携替换 | [更新验证](application-updates.md)，最终检查见该记录；真实公开新版本、NSIS 更新器执行及 Linux/macOS GUI 更新验收分列 |
| 本机统一检查及浏览器 | verify 退出 0：Rust 1,032、前端 22、脚本 5，8 项平台条件测试忽略，类型无错误/告警；另 Claude 专项六项通过（与统一检查重叠）；此前 Edge 提醒显示/去重回归保留 | [VS 发现修复](m9-vs-copilot-discovery.md)、[Codex 修复](codex-today-recovery.md)、[本轮](plan-20261007.md)、[Claude](claude-container-sample.md)；本次未重跑浏览器，既有模拟 IPC、[三源](m3-container-samples.md)/[M8](m8-container-samples.md) 结果保留 |
| Windows 成品 | 0.2.1 业务 release/无界面 11 项、VS 本机 4 调用允许字段核对及重扫、Codex 本机旧库补采与重扫、Claude 新库/旧库自动纠正通过；此前原生 WebView2/IPC 19 项（含明细 Merge/提醒）、20 次首屏及接收器 8 项保留 | [VS](m9-vs-copilot-discovery.md)、[Codex](codex-today-recovery.md)、[Claude](claude-container-sample.md)、[本轮](plan-20261007.md)、[三源](m3-container-samples.md)；该修复未重跑 GUI/IPC；合成检查隔离 SQLite 与来源环境，真实样本另列 |
| Windows 安装 | NSIS 12 项升级/回滚/卸载/重装与失败中止通过 | [生命周期](installation-lifecycle.md)；仅自有任务/启动项，其他值保留 |
| Linux 成品与安装 | Debian workspace 1,004 项；deb 生命周期、实际 AppImage FUSE/GTK/Orca 共 9 组/47 项通过；此前 GTK 缩放 1/2 各 40 项独立保留 | [三源阶段](m3-container-samples.md)、[生命周期](installation-lifecycle.md)、[Orca 十语言](orca-multilang.md)；真实只读挂载及退出释放已核对 |
| 原生凭据 | Windows 处理确认后的两组各 100 轮并行、Linux 独立 D-Bus/keyring 六项及 macOS CI Keychain/HTTP 两项通过 | [跨平台凭据](platform-auth-continuation.md)、[CI](ci-plan-validation.md)；旧 Windows 撤销异常原因仍未确认，首次失败及残留检查保留 |
| 本批远端 CI/下载包 | 两轮各八作业成功；三份归档 API 摘要/CRC、四个包 SHA-256/大小及报告 revision 相符 | [CI 37486581699](https://github.com/owent/llm-usage/actions/runs/37486581699)，受测源码 61223e591815a4369a85a00fa23ff1ab2819d6d5；完整分项见 [本批 CI](ci-plan-validation.md) |
| Portable 版本 0.2.2 | 六种原生 Windows/Linux/macOS x64/arm64 .tar.zst 归档；恢复制品上传后全部 12 个 tag 作业通过。六种解压成品无界面、两个 Linux GUI 及本机 Windows x64 19 项原生 GUI 检查通过；13 个发布附件大小/摘要一致且实际覆盖成功 | [tag CI 37761919575](https://github.com/owent/llm-usage/actions/runs/37761919575)，受测源码 39e74d09fa2b4e300dca3f7d0ba2edf4b6e95ba2；[CI 记录](ci-plan-validation.md#portable-版本-v022)。未建立新安装生命周期、Windows arm64/macOS GUI 或签名/公证验收结论 |
| 规模与资源 | 百万/千万未命中查询 P95 47.05/133.86 ms；百万 GUI 导入峰值 371.09 MiB；十分钟空闲均值/峰值 299.90/363.30 MiB，达到调整后的上限 | [规模测量](plan-execution.md)、[增量与原生检查](plan-finalization.md)；开发机、合成来源、全进程 private bytes，原测量与上限调整保留 |

本机 Windows/Linux 包与 CI 下载包分别核对；本机安装结果不由 CI 归档摘要推导。
2026-10-07 业务轮次保留用户报告的主分支合并、发行签名/公证及 Release 完成状态，
未独立重新核验这些外部结果。2026-10-08 的 portable 草稿发布另按上表实际核对，
未执行新的签名/公证；后续仅记录提交不等于再次执行源码 CI。

<a id="source-and-feature-records"></a>

## 来源与功能记录

| 范围 | 当前记录 |
| --- | --- |
| 核心存储、来源身份/聚合交换 | [M1](m1-core.md)、[M1a](m1a-provenance.md) |
| 完整标准化明细 Merge、用量及费用提醒、Zed 外部 Provider | [本轮](plan-20261007.md)；事务内冲突处理/重扫、单币种/未知/去重及 Zed 1.22.0 两模型非空实样分列；指定端点为 Coding Plan |
| Codex、pi/oh-my-pi、Claude/Gemini/Qwen 初始读取及版本分派 | [Codex](m2a-codex.md)、[M2 恢复](m2bc-resumed.md)、[版本目录](m2d-layout-versions.md)、[CLI 安装边界](wsl-agent-installs.md)；初始记录保留当时样本状态 |
| Codex 当天漏采集 | [读取窗轮转与旧库恢复](codex-today-recovery.md)；历史大文件不再持续推迟未访问文件，本机逐条核对及重扫；新版本仍保留兼容状态 |
| Claude Code 2.1.197 | [国内下载与原生实样](claude-container-sample.md)；智谱两模型主循环、逐条版本、多块去重、默认零未知及旧库回读；Anthropic 自家模型和其他场景另验 |
| Qwen 0.25.0 原生/SDK、OpenCode 1.18.34 | [容器来源](container-sources.md)；逐次/原生/封存分区择一，标题与后台覆盖分列 |
| Cline SDK、Hermes、OpenClaw schema 24、MiMo/Zoo/DSH | [Cline](cline-container-sample.md)、[Hermes](hermes-container-sample.md)、[OpenClaw](openclaw-container-sample.md)、[三源](m3-container-samples.md)；真实非空记录与当前成品升级/重扫已核对 |
| Kilo、Kimi、ZCode、CodeBuddy/WorkBuddy | [M3/M4](m34-kilo-zcode-kimi.md)、[Buddy 本机](m4-buddy-local.md)、[Kilo 健康/选区](trend-range-kilo.md)；CLI/扩展、独立对账与 trace 覆盖分列 |
| M8 第二批 | [18 个适配器](m8-second-batch.md)、[历史十源](m8-container-samples.md)、[Zed 实样及当前容器限制](plan-20261007.md)；没有账户/协议/发行物的样本项移出当前计划，Qoder 仅探针 |
| Copilot 四面、额度与遥测 | [规则审查](m9-copilot-review.md)、[VS Code](m9-copilot-chat-local.md)、[Visual Studio 实样](m9-vs-copilot-local.md)、[VS 跨版本发现修复](m9-vs-copilot-discovery.md)、[JetBrains 静态分析](m9-jb-copilot-analysis.md)、[配置入口](telemetry-setup-ui.md) |
| 看板、调度、取消、并行及旧规则恢复 | [原生与规模](plan-finalization.md)、[查询与后台](plan-execution.md)、[完整旧摘要](parser-conflict-fix.md)、[来源规则升级](source-policy-upgrades.md) |
| 费用引擎、在线刷新、当前 API 参考及归档修正 | [引擎](f2-cost-engine.md)、[刷新](f2-online-refresh.md)、[当前参考](dashboard-reference.md)、[归档](pricing-archive-repair.md)；真实数据发生时估算仍有渠道前置条件 |

<a id="acceptance-scope"></a>

## 验收边界

- 真实来源结论限于受测版本、原生记录和非空场景；未知字段、未保存调用与累计区间保留，不补零或扩展产品能力。完整支持范围见 [接入矩阵](../../design/desktop-usage/adapters.md)。
- 原生 DPI 144/UI Automation 名称、GTK 缩放 1/2 与 Orca 十语言五页导航各自有效；ALSA null 不能据此确认物理音频或全部控件，控件名称不代替 Narrator/NVDA 完整使用。
- Linux 容器为普通应用用户、默认 seccomp、无网络/宿主挂载；FUSE 仅对自有容器显式加 SYS_ADMIN。结果不能据此确认宿主登录/注销、完整桌面或其他发行版。
- macOS 原生凭据与构建 CI 已通过；macOS 桌面、特定硬件不在本轮要求内。
- 合成规模库与百万 GUI 导入不能据此确认其他真实来源；应用调度循环不等于 OS 唤醒，暖空页不等于全新 WebView 最低开销，协作式中断不能强制取消 OS 阻塞读取。
- 已观测 WebView 请求无外部 HTTP 不等于全进程出站审计；接收器/凭据测试不能据此确认其他真实 exporter。
- 更多 DPI、完整读屏、宿主登录/注销及 OS 唤醒按用户要求移出；当前环境不能执行的来源扩展、F1 与系统调查条件见本轮记录，移出不等于通过。

临时日志、核对库和原始记录均位于仓库根已忽略的 build/ 各任务目录；
公开记录保留最小脱敏结果，接收器自有凭据、一次性 keyring 与守护进程按各次验收回收；
用户要求配置的 Zed Provider 密钥留在 Windows 凭据库，普通 settings.json 不含密钥。
