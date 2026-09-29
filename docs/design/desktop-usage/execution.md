# 执行详情与交付顺序

任务状态以 [Plan.md](../../../Plan.md) 为准；本文件描述各阶段的输出、依赖与验收。
已完成阶段的实施细节、命令与证据见 [验证记录](../../validation/desktop-usage/)，
此处只保留交付要点与仍然有效的合同。不以计划更新代替实现或验证记录。
实施阶段提取本机真实 Agent 数据验证已获用户允许，按
[准备结论](implementation-readiness.md)执行，无需重复确认同一许可。

## M0：固定实施边界和试验基线（基本完成）

已交付：三平台 OS/WebView/架构基线、根与 desktop 独立依赖声明（2026-09-29 起
`^` 浮动最新，锁文件保证可复现）、版本/许可证固定、最小 release 试验与包体报告、
目标 Agent 脱敏 fixture、真实命令与验证记录模板、三平台 CI 矩阵与 WSL 独立证据。
余项：GitHub CI 首次运行待推送后登记；空闲内存超标转 M6/M7 定位。

## M1：统计核心和 SQLite（基本完成）

已交付（[审查记录](../../validation/desktop-usage/m0-m1-review.md)）：领域类型与
[数据合同](data-contract.md)期望测试、标准化事件/累计/区间/去重别名/迁移、
原子提交与故障注入恢复、按模型/Agent/provider/日查询（周月由日派生）、
更新与撤销、容量/保留/封存/导入批次身份、本机归属依据与采集作业合同。
余项：V20 性能初值、V15/V16 迁移前空间检查与一致备份。

## M1a：历史来源身份与存储分区（已完成）

验证记录：[m1a](../../validation/desktop-usage/m1a-provenance.md)。
仍有效合同：稳定主机 ID/主机名/来源实例区分原始来源与采集位置（身份不随重启、
改名或已确认迁移变化）；来源进事件唯一约束与汇总分区；版本化迁移不猜归属；
版本化交换合同（原始来源、分区键、时间范围、修订、快照/增量性质、重复跳过/
同源修订替换/互斥新增/冲突保留）。完整导入/Merge 另行排期；
不将主机名作为本机归属证明。

## M2：首批本地读取（基本完成）

六源适配器（Codex、Claude Code、pi、oh-my-pi、Gemini CLI、Qwen Code）、
[目录化迁移与版本组织](#m2-layout)、未知版本兼容尝试均已验收
（[M2-A](../../validation/desktop-usage/m2a-codex.md)、
[M2-B/C](../../validation/desktop-usage/m2bc-resumed.md)、
[M2-D](../../validation/desktop-usage/m2d-layout-versions.md)）。
余项：claude/gemini/qwen 真实 fixture 待本机数据。

<a id="m2-layout"></a>

### 适配器目录与版本组织合同（已实施，持续有效）

按 [架构合同](architecture.md#adapter-layout)，所有 Agent 使用独立目录：
统一入口 + 版本探测/分派 + `versions/` 格式实现；产品特有映射归对应目录，
跨 Agent 仅共享已验证的框架、读取器和辅助逻辑。已知版本按映射分派，
未知或缺失版本默认尝试该 Agent 对应输入类型的最新内置解析器并带兼容标记
（[兼容合同](architecture.md#unknown-version)）。新增历史版本补分派和混合版本
fixtures；修改 Agent 内部共用逻辑回归其全部已支持版本，修改跨 Agent 共用逻辑
回归全部实际使用者。M3–M5、M8 与 F1 沿用同一要求。

### 逐适配器交付合同（持续有效）

每个适配器独立交付发现、探测、增量、映射、能力说明及 fixtures；
静态/fixture/真实应用三种证据分列；解析器 fixtures 通过后对本机真实数据
做只读核对（所需允许已给出）。环境覆盖与手工根按平台路径列表处理，
不沿用跨平台分号拼接假设。源历史不足所配保留期不等于应用保留配置失效。

## M3：扩展本地读取（基本完成）

kilo 真实核对完成；cline/dsh/hermes/openclaw/opencode/mimo/zoo 七产品
文档级证据实施、合成测试通过、真实验收后置
（[M3/M4 记录](../../validation/desktop-usage/m34-kilo-zcode-kimi.md)、
[文档级记录](../../validation/desktop-usage/m3-doclevel-cmdh.md)）。
分产品字段能力表、格式检测、重放幂等与来源交叉去重可复核；V30 已随各记录验收。

## M4：新版及缺证产品核验（基本完成）

kimi-code/kimi-work/zcode 适配器已实施并真实核对
（[M3/M4 记录](../../validation/desktop-usage/m34-kilo-zcode-kimi.md)）；
WorkBuddy 本机无数据（2026-09-25 全盘未检出），待重新出现数据再验。
交付只能是：有证据的本地适配器 + 受支持版本与回归样本；或仅本机来源可核验的
会话导出/本地遥测接入（明确字段缺口）；或具体待证项（界面显示“尚无可靠用量源”）。
找不到入口不证明技术上不可能，也不无限期逆向私有接口。

## M5：本机遥测和本地导入（未完成）

先完成 Copilot/VS Code 的文件导出，再增加按需启用的本地 OTLP 接收器及 CodeBuddy。
显式支持实际发送协议，覆盖 protobuf/JSON 差异、认证、压缩体上限、未知字段和重传。
采用字段白名单，拒绝正文持久化；配置向导只提出本地最小采集设置。
所有来源增加本机归属检查：只接受已启用本机实例的遥测或本机会话导出；
拒绝远端转发、账号/组织汇总，即使文件在本地或数据经 loopback 到达。
本地同一来源/日期重新导出按修订替换，批次取消/失败保留旧结果，不重复增加；
替换范围由 M1a 的原始来源与完整分区决定。
通过条件：V08/V12/V17/V22/V25；进程未运行的采集空白、源端写入延迟、
隐私开关、父子 span、双 instrument token 样本计数均能正确解释。

## M6：桌面 UI 和用户配置（主体功能已实现）

已实现并验证（[m6 记录](../../validation/desktop-usage/m6-desktop-core.md)）：
五页界面、图表与筛选、来源页、设置、导出导入、调度、headless、i18n（10 语言）、
多用户。余项：真实桌面逐操作验收 V13–V18/V23–V25、V24 系统任务真实验收、
逐源定时与监听。仍有效合同：实际 Windows 桌面逐操作验证不能仅以 Web DOM 测试
证明 IPC 正常；机器可读导出遵循 M1a 交换合同；用户任务不显示内部表名/游标/SQL。

## M7：首个发布候选（部分完成）

已交付 release 构建 + NSIS 包体 + WSL 编译/测试
（[M7 记录](../../validation/desktop-usage/m7-build-partial.md)）。
余项：V19–V27 真实安装、资源、调度、来源边界与三平台验收；逐工具支持矩阵。
Windows 11 实机与 GitHub runner 分开记；macOS/Linux CI 持续运行，
WSL 不代替原生 Linux 桌面或 macOS 发布验收。代码签名、自动更新和发布渠道
在获准发布时办理，普通实现授权不等于发布授权。

<a id="m8"></a>

## M8：扩展 Agent 覆盖（第二批适配器，文档级实施已完成）

2026-09-29 完成覆盖调研与同日文档级实施（[M8 验证记录]
(../../validation/desktop-usage/m8-second-batch.md)）：18 个适配器
（zed/aider/junie/xum/droid/amp/grok/roo/goose/crush/jcode/gajae-code/
commandcode/continue/atomcode/kiro/antigravity/qoder 探针）全部独立目录 +
版本注册表 + V30 + 合成 fixture 合同测试（566 项全绿、verify 全链通过）；
本机均无真实数据（Zed 空库），真实验收后置。Amazon Q/Codebuff（本地无逐次
token 载体，源码级取证）与 iFlow（2026-04-17 停服）不实施 token 适配器。
依赖 M1 框架与 M2 目录化/版本注册表合同；每个适配器沿用独立目录、
统一入口、未知版本兼容尝试与 V30 验收，不放宽本机来源与未知不补零合同。

分批建议（按证据强度与取证成本排序）：

1. 官方开源、可固定源码核验：Roo Code、Goose、Crush、Continue、jcode、
   gajae-code、Command Code、Amazon Q Developer CLI；
   Antigravity CLI 的 protobuf 布局按官方发行产物/固定版本反推核对。
2. 闭源但路径/字段经第三方解析器确证，需本机脱敏 fixture（许可已给）：
   Amp、Grok Build、Junie CLI、Kiro、Droid、Xum、iFlow CLI、Qoder CLI。
3. 需启用载体：Aider（`--analytics-log`/llm 历史日志），不回填历史。
4. 前置取证再定：AtomCode（先源码级核验存储与 usage 字段）；
   Warp/Cursor/TRAE 的远端 dashboard/usage API 路线按边界排除，不入 token 统计。

实施边界：

- 双载体对账（Amp ledger/消息 usage、Kiro execution/快照）必须按原生 ID 或
  逐桶一致对账，不能按时间接近或数值相同猜测同一请求。
- 会话级聚合来源（Goose、Xum、Droid）按区间语义展示，不展开成伪造逐次事件。
- 估算路径一律不采纳：Kiro Auto 补零、Grok 累计差额、Goose reasoning 差额、
  任何 tokenizer 近似；缺数如实标 unknown。
- 品牌迁移双根发现：goose（Block 旧根）、Xum（mux 旧根）、Qoder（灵码演化）、
  Antigravity（~/.gemini 同根异子目录）、Codebuff（manicode 通道根）。
- Junie CLI 与 Zed 内置自 F1 转入本阶段；JetBrains AI Assistant IDE 插件、
  TRAE、Cursor、Windsurf 及其余缺证 IDE 仍留 F1。
- 本机未安装的产品按 M3 先例交付文档级证据实现（固定源码/第三方解析器锚点 +
  合成 fixture，discover 在本机返回空），真实样本出现后升级验证。

通过条件：逐适配器静态/fixture/真实三种证据分列，V30 结构与迁移回归；
新增家族样本按矩阵“各家族必须补的样本”清单覆盖；
共享解析组件改动回归全部实际使用者；重复扫描不增量、对账不双计。

## F1：后续 IDE 支持

该阶段未排期，当前不实施，不是 M0–M7 的依赖。
范围为 JetBrains AI Assistant IDE 插件、TRAE/TraeCode、Cursor IDE、Windsurf、
京东 JoyCode、智谱 CodeGeeX 插件、百度文心快码 Comate、华为 InsCode/CodeArts Snap
等接入矩阵标为 F1 的缺证 IDE 变体；已证实本地载体证据的 Junie CLI 与 Zed 内置
已转入 M8。后续开展时依次核验本机版本、日志/存储/会话导出、字段与生命周期、
脱敏 fixture、只读真实数据和定时提取；交付有证据的适配器或具体限制。
若宿主只是运行已有适配器支持的外部 Agent，按底层来源计量并去重。

<a id="f2"></a>

## F2：模型 API 按量价格获取调研（调研已完成，实施待排期）

[价格合同](pricing.md)已交付；费用引擎与 UI 实施另拆排期。
仍有效边界：不上传用量/主机/来源身份/会话或账户密钥；不接入远端用量/账单 API；
历史估算按发生时价格与当前价格分开；缺证模型保留未计价。

<a id="f3"></a>

## F3：界面多语言（i18n）方案调研与设计（已完成）

[i18n 合同](i18n.md)已交付并随 M6 实施（10 种语言）；V31 原生 GUI 验收随桌面验收。
统计口径不随语言隐式改变；诊断 code 保持稳定英文标识。

## 变更与回退

每阶段保持可独立审阅的变更；不自动提交、推送或部署。
解析器失效先禁用该源并保留旧结果，可回退到已验证版本；不回滚 Agent 的安装。
数据库迁移失败恢复一致备份；新库版本不能强行用旧程序写。
误清理只可从用户保有的合法备份或来源恢复，界面必须在清理前说明不可逆部分。

阶段完成记录须包括命令、cwd、OS/运行时、锁定版本、退出码、测试数量、
实际结果、失败及未执行项。记录放 docs/validation/desktop-usage/ 下；
只有产生实际证据后才创建记录文件，不预填“通过”。
