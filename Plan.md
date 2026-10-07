# 桌面用量客户端执行计划

当前版本 0.2.1，Windows 11 x64 首发，保留 Windows/Linux/macOS CI。
本文件维护本轮可执行工作；移出范围及首次失败见
[本轮记录](docs/validation/desktop-usage/plan-20261007.md)，移出不表示验证通过。
历史结果见 [最新验收](docs/validation/desktop-usage/current-acceptance.md)。
设计入口：[产品与架构](docs/design/desktop-usage/README.md)、[交付要求](docs/design/desktop-usage/execution.md)、
[数据规则](docs/design/desktop-usage/data-contract.md)、[接入矩阵](docs/design/desktop-usage/adapters.md)、
[验收要求](docs/design/desktop-usage/validation.md)。

## 当前进度

| 阶段 | 已实施范围与记录 |
| --- | --- |
| M0/M1/M1a | 工程基线、SQLite 事务/恢复/统计/备份、来源身份及聚合交换；本轮新增完整标准化明细导出、预览与事务 Merge，保留修订、冲突、未知、累计和归档；[明细合同](docs/design/desktop-usage/detail-merge.md) |
| M2–M5/M8/M9 | 已注册来源解析与本机遥测/接收认证；真实标签仅限受测版本与载体。M8 的 Zed 新增 1.22.0 内置 Agent 外部 Provider 两模型、缓存真实样本；Claude Code 2.1.197 已由国内镜像完成智谱两模型原生主循环及旧库回读，[专项记录](docs/validation/desktop-usage/claude-container-sample.md)；逐源边界见接入矩阵 |
| M6/F3 | 五页、选区、保留/导出、逐源计划、暂停/取消、两来源并行、Windows 托盘/节能/通知及十语言；[交互与调度](docs/validation/desktop-usage/plan-finalization.md)、[Linux Orca](docs/validation/desktop-usage/orca-multilang.md) |
| M7 | 既有 Windows NSIS、Linux 包/GTK/WebKit/FUSE/Orca、规模及资源验收；历史 CI 与制品核验不能认证本轮代码；[安装](docs/validation/desktop-usage/installation-lifecycle.md)、[规模](docs/validation/desktop-usage/plan-execution.md)、[既有 CI](docs/validation/desktop-usage/ci-plan-validation.md) |
| F2 | 费用、价格快照、在线缓存/失败回退及官方 API 参考；本轮新增默认关闭的日/月 token 或单币种发生时估算预算提醒、精确阈值和持久去重；[预算合同](docs/design/desktop-usage/budget-reminders.md) |
| F1 | 已按授权核查候选 IDE 安装及数据路径，当前没有可测试安装/本地用量载体，移出本轮，不认证产品不支持 |

## 本轮结果

新增功能的统一检查、浏览器、Windows release/真实 IPC、无界面及 Zed 原生重扫已完成，
实际结果、首次失败及范围条件见本轮记录。受影响合同、来源能力和最新验收索引已同步。
Claude 专项另完成国内下载完整性、真实主循环、默认零修正及新库/旧库成品回读；
本次未追加 GUI/IPC 或发行验收，范围见专项记录。
本轮可执行范围没有剩余活动待办；环境无法执行的项按用户要求移出，原核验限制保留。

更多 DPI、完整读屏、宿主登录/注销及 OS 唤醒测量按用户要求不再执行。
主分支合并、发行签名/公证及 Release 按用户说明已完成，从执行计划移除；
本轮不另行提交、推送或发布。缺少账户、协议、发行物、历史证据或当前权限的
来源/系统场景移出活动待办，条件见本轮记录；原始验收要求和历史证据保留。

## 执行边界

- 先核对源码、配置、测试和版本依据；只同步受影响的规则、Skills、设计和记录。
- 只统计本机来源，未知不补零，调用/消息/累计/额度分开；费用默认关闭，多币种不合并。
- 本机只读提取按 [准备规则](docs/design/desktop-usage/implementation-readiness.md) 白名单与脱敏执行。
- 本轮已授权 Podman、指定 Provider 最小真实请求、本机 Zed 配置/测试与 F1 核查；凭据只用于目标进程或系统存储，不进入记录或版本库。
- 指定端点已核验为 Coding Plan，不认证按量 API 实付，不对套餐用量套按量发生时价格；其他真实记录缺渠道依据时仍受限。
- 保留用户修改；临时产物只放根 build/；不登录其他产品账户或构造认证状态。
- 依据按记录所属版本保留；发现、旧处理位置、修订/冲突/封存规则以数据合同为准，只凭实际结果更新状态。
