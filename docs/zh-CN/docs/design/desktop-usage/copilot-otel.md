# Copilot 补充采集与 OTel 配置设计

<a id="copilot-supplemental-collection-and-otel-configuration"></a>

状态提示按缺配置、已配置等待数据、样本已核验和配置冲突分开。
总览使用紧凑摘要；详情每 30 秒只读自动检查实际输出，多个无法区分的 Profile 共享
同一输出时不冒认各自生效，数据存在也不消除策略冲突或保证历史完整。
JSONC 合并不重复插入空行，保留用户原格式；见
[交互说明](dashboard-polish.md) 和 [验证记录](../../validation/desktop-usage/dashboard-polish.md)。

异步用户配置检查、总览提示和设置页合并配置入口已实施。
trace SQLite 适配尚未实施。配置入口的开发验收范围见
[实施记录](../../validation/desktop-usage/telemetry-setup-ui.md)。
已有原生文件验收见 [M9 审查记录](../../validation/desktop-usage/m9-copilot-review.md)。

本应用配置的 VS Code Copilot file 输出自动采集已实施并完成真实副本核验，
见 [看板修正规则](dashboard-repair.md)与[验证记录](../../validation/desktop-usage/dashboard-repair.md)。
`chat <model>` CLIENT span 逐调用入库；同主机/用户/会话/本地日以 OTel 替代原生贡献，
原生记录保留且重扫不重新叠加，封存分区保持原生并排除重叠新 span。
不按近似时间/token 相等猜调用 ID。开启当天可能缺少此前调用，不能把导出当完整历史。
CLI/JetBrains 新版本仍需独立真实验收，其他隔离导出不自动推广为统计来源。

<a id="supplemental-collection-options"></a>

## 补充路线与取舍

| 路线 | 可以补充什么 | 当前核验结果与限制 |
| --- | --- | --- |
| VS Code Copilot Chat OTel file | 逐调用输入、输出、缓存及推理桶；子 Agent 调用 | 固定源码与本机 30 个 CLIENT span 已验收；本应用输出自动发现，未返回字段仍未知 |
| VS Code Agent Host OTel file | 原生 runtime 的逐调用记录 | 独立配置命名空间；SDK 的 service.name 不能单独证明它是独立 CLI |
| VS Code 本地 trace SQLite | IDE 留存的 span，可导出已有历史 | 官方文档和表结构核验结果；当前无此数据库适配器；需启用，历史受 IDE 保留期限限制 |
| Copilot CLI OTel file | 最新 chronicle 库缺失的未来逐调用 token | 官方 CLI 配置明确；行级格式仍须真实样本核验，不能从旧库恢复已丢失 token |
| Visual Studio 自动 traces | 已核验 VS 18 文件接入逐调用和缓存读，未核验全部 VS 2022/旧扩展 | 同时检查 TMP/TEMP/用户默认临时目录；提高采集频率减少清理前漏采；[跨版本依据与核查工具](../../validation/desktop-usage/m9-vs-copilot-discovery.md)，不猜 exporter 配置 |
| JetBrains Copilot debug File Logging | 插件提供的 OTel outfile | 1.18.0-261 静态核验结果；手工配置，无真实 IDE 验收，不直接改插件 XML |

首选本地 file：IDE/CLI 可以在本应用退出时继续写入，之后扫描。
本机 OTLP/HTTP 适合实时接收，但本应用必须运行；支持 `/v1/traces`、`/v1/logs`
的 JSON/protobuf，及隔离补充输出的 `/v1/traces/supplemental`。
接收前须逐源认证；原生凭据存储可用时可自动配置 Claude/Codex logs，以及同路径 npm manifest
确认的 CodeBuddy CLI 2.98.0 隔离 traces。协议解析/限流测试不代表其他发送端或版本
已有可用认证配置，见 [认证要求](receiver-auth.md)。
不支持 gRPC 或 metrics，不能称为通用 Collector。

<a id="implemented-configuration-controls"></a>

## 已实施的用户配置入口

应用启动后在后台有界发现安装清单与 PATH 中的 Agent 启动文件。
用户目录只定位配置，不单独证明已安装；卸载后残留目录不显示。
检查只读；总览只保留简要提示、“一键开启全部”和“查看详情”。查看详情直接进入
设置 → 本机遥测面板；该面板仅列本机已安装且本功能支持检查的 Agent，提供批量配置、
单项预览/应用/撤销及重新检查，不展示未安装 Agent 的配置模板。
页面之间共享检查结果，不阻塞统计查询，不定时改用户文件，不执行 Agent 启动文件。
进入设置遥测面板时只读刷新安装与配置状态；CLI 当前仅认可 PATH 启动文件，
非标准安装位置且未在 PATH 中的 CLI 不自动列出，不能用用户目录猜测已安装。

批量按钮直接处理当前缺失且可自动配置的项目，逐项生成计划并立即合并应用；
不预先生成所有计划，以免共享设置文件在前一项写入后使后一项的并发检查失效。
已有配置和需手工核对项跳过；单项失败继续后续项目，分别报告成功、失败和手工项。
批量执行状态与撤销入口跨页面保留；总览不列出路径、配置键或逐项结果。
批量创建的 CLI launcher 仍需用户通过脚本启动，文件配置仍需重载 IDE/Agent；不自动运行。

| 客户端 | 一键配置范围 | 输出与核验边界 |
| --- | --- | --- |
| VS Code / Insiders Copilot | 默认用户与已有 profile；核对安装清单实际声明的 OTel 键；file 导出、内容和身份关闭 | 新 Windows 版本目录与常规扩展目录均检查；同步排除项合并，明确同步冲突降为手工 |
| VS Code Agent Host | 安装的 app manifest 为已核验 1.140.0 时提供独立 chat.agentHost.otel 入口 | 各 profile 独立文件；其他版本待核验，不能从扩展键推断宿主支持 |
| Copilot CLI | 创建本应用目录内专用 PowerShell/sh 启动脚本；运行脚本才给进程配置官方 file 变量 | 不修改 config.json、settings.json、全局环境或 shell profile，不自动运行 |
| Gemini CLI | 用户 settings.json：telemetry enabled/local/outfile，logPrompts=false | GEMINI_CLI_HOME 替换 home 后仍附加 .gemini；文件格式/新版本真实输出未验收 |
| Qwen Code | 用户 settings.json：telemetry enabled/outfile，logPrompts 与敏感 span 关闭 | QWEN_HOME 为配置目录，QWEN_RUNTIME_DIR 不替代设置路径；0.25.0 连续多行 SDK JSON 已接入逐调用与原生分区择一；应用管理输出自动发现、只读核验，其他版本隔离；见来源升级记录 |
| Claude Code | 用户 env：logs 专用 HTTP/JSON endpoint 与认证头，内容关闭 | 指定字段的日志隔离；逐源凭据进当前用户系统存储，真实导出仍待验收 |
| Codex | 用户 TOML：HTTP/JSON logs、独立认证头与 log_user_prompt=false | 保留其他表、供应商配置/headers 及注释；日志不自动叠加原生用量 |
| CodeBuddy | 首个 PATH launcher 同目录 npm manifest 为 2.98.0 时配置 generic headers，Bearer 空格编码 %20，内容关闭 | 仅写隔离 supplemental traces；其他版本/无 manifest/已有 traces-specific headers 为手工项，本地 JSONL 独立 |

Visual Studio 已核验的 VS 18 组件会自动导出 traces 文件；VS 2022 按组件和实际文件另验，
不编造用户 exporter 设置。JetBrains 仍仅手工步骤。
portable/custom user-data、其他 IDE 与远端宿主不自动写入，按官方步骤核对。
现有输出目标不替换：已启用的保留；有目标但未启用的提示手工核对。
已知进程环境覆盖、托管文件、关闭遥测、同步冲突或配置错误时不提供自动写入。
用户层状态不代表企业策略、工作区或另一个启动环境下的最终生效状态。

单项配置先显示目标路径、实际待改键和值、输出位置及接收器影响；再应用。
JSONC 按 AST 值区间合并，TOML 使用保留格式的编辑器；检查 BOM、注释、重复键、
大小上限、链接、只读和并发修改。先保存原始字节备份，再同目录临时文件替换。
若接收器端口不能绑定则不写 Agent 配置；配置失败时恢复本次接收器启用状态。
HTTP 应用时生成逐源令牌并回查系统存储，失败回收本次凭据；预览只显示占位符，
不回传现有 headers 中的凭据。撤销先吊销凭据再条件恢复用户键，其他来源继续工作。
profile 的同步排除项写在默认用户文件：预览列出两个文件，应用时均检查并发版本；
profile 写入失败后条件回滚默认文件，不覆盖回滚期间的用户编辑。
撤销仅恢复仍等于本功能写入值的键，保留后续用户编辑并报告冲突；
预览与撤销令牌仅在当前应用进程有效，重启后原始备份留在本机。
共享接收器不会因撤销一个客户端而自动关闭；已采集文件和历史统计不删除。

OTel 只能补充开启后且实际导出的记录；缓存、推理、失败、辅助调用、重试和 inline
逐项核验，不承诺所有 Copilot 请求都有 token。父 invoke_agent、子 chat、同调用
事件与 token histogram 不相加。premium 额度和 AIU 继续独立保存，不折算 token。

<a id="manual-configuration"></a>

## 手工配置

以下说明用于生成文件，不表示本应用已完成全链验收。先创建本机专用目录，
使用实际绝对路径；不假定 outfile 展开 `%LOCALAPPDATA%` 或 `~`。
先保留原生统计，验证新文件后选择统计来源，避免直接混合总计。

<a id="vs-code-extension-host"></a>

### VS Code 扩展宿主

打开用户 Settings JSON，合并以下键。替换用户名，先创建父目录：

```json
{
  "github.copilot.chat.otel.enabled": true,
  "github.copilot.chat.otel.exporterType": "file",
  "github.copilot.chat.otel.outfile": "C:/Users/<用户名>/AppData/Local/llm-usage/copilot-otel/vscode.jsonl",
  "github.copilot.chat.otel.captureContent": false,
  "github.copilot.chat.otel.captureIdentity": false
}
```

重载窗口后检查文件；下一次用户正常使用 Copilot 后核对 chat span 的身份、时间及
usage。验证配置不自动发送测试提示。自选文件通过本应用手工根接入；一键配置的 Copilot file 自动发现。
添加后先查看受限诊断与独立核对结果，统计重叠须按后文处理。

### VS Code Agent Host

使用 Agent Host 会话时采用该宿主自己的键及独立文件，不与上面的文件共用：

```json
{
  "chat.agentHost.otel.enabled": true,
  "chat.agentHost.otel.exporterType": "file",
  "chat.agentHost.otel.outfile": "C:/Users/<用户名>/AppData/Local/llm-usage/copilot-otel/agent-host.jsonl",
  "chat.agentHost.otel.captureContent": false
}
```

个人设置在宿主启动时绑定，需要重启宿主或窗口。仅运行于本机的 runtime 纳入统计；
不为远端 Agent Host、SSH 或云端会话提供接入；WSL/容器按显式实例边界单独处理。

### Copilot CLI

在准备运行 CLI 的 PowerShell 终端设置进程级变量：

```powershell
$copilotOtelDir = Join-Path $env:LOCALAPPDATA 'llm-usage/copilot-otel'
New-Item -ItemType Directory -Path $copilotOtelDir -Force | Out-Null
$env:COPILOT_OTEL_ENABLED = 'true'
$env:COPILOT_OTEL_EXPORTER_TYPE = 'file'
$env:COPILOT_OTEL_FILE_EXPORTER_PATH = Join-Path $copilotOtelDir 'cli.jsonl'
$env:OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT = 'false'
```

随后在这个终端正常使用 CLI；既有进程不会获得新变量。关闭终端结束进程级配置。
一键功能已提供专用启动脚本；脚本退出后恢复 PowerShell 的原进程变量。

<a id="http-receiver"></a>

### HTTP 接收方案

Claude/Codex 和已核验 CodeBuddy 2.98.0 配置向导会先检查原生凭据存储及绑定并即时启用，
生成本机实例独立认证；CodeBuddy 始终使用隔离补充输出。
现有后端键为 `otel_receiver_enabled` 和 `otel_receiver_port`，默认关闭、端口 4318。
这两个键不绕过认证；现有未认证的本应用 HTTP 配置需重新预览/确认，不能自动续用。
Copilot HTTP 的逐源配置入口仍待核验，当前按上方 file 方案接入；
CodeBuddy 未核验版本不自动配置。
不能只把 endpoint 指向本应用就认为接入成功；不配置远端 Collector。

环境覆盖、企业策略和 VS Code telemetry 开关可能使设置失效；向导展示冲突原因，
不覆盖策略。HTTP exporter 可能发送 metrics；当前不接收 metrics，logs 独立保存。

<a id="additional-integration-design"></a>

## 后续接入设计

1. 数据源页增加“补充 Copilot 采集”入口，按客户端、版本与宿主展示覆盖缺口、
   官方说明和可复制配置；支持未安装/不支持版本状态。优先 file，其次 HTTP。
2. 预览修改目标、键、输出文件和来源影响。用户选择安装/profile/user-data
   目录，核对 JSONC、环境覆盖与策略；不扫描或打印配置里的凭据。
3. 用户点击应用后备份原始字节，只修改所列键，保留注释与其他设置；写入前比对
   文件版本，并发修改时重新预览。失败恢复本次变更并报告实际结果。
4. 分别显示“配置已写入”“需要重载”“等待记录”“已收到可解析调用”；
   只读限定字段验证调用身份、时间、token 桶和重扫，不以文件存在代替验收。
5. 撤销只恢复本功能改过且仍等于写入值的键；保留用户后改的键并报告冲突。
   备份仅本机保存，不进日志/导出；不自动重启 IDE 或产生模型调用。

已实施范围以上表为准，首发验收 Windows；Linux 原生凭据/HTTP 已在 WSL 隔离验证，
macOS 凭据模块仅交叉类型检查通过，两平台真实桌面/exporter 验收仍独立保留。
只改用户配置，不写项目 `.vscode/settings.json`；合并同步排除项，不启用 Settings Sync。
JetBrains 保留手工步骤，Visual Studio 使用已核验且实际存在的文件。
已完成应用管理的 VS Code Copilot file 自动接入、会话范围来源选择与
本机已导出数据验证，见 [看板修正记录](../../validation/desktop-usage/dashboard-repair.md)。
CLI/JetBrains 的新版真实导出、trace SQLite 和更多真实撤销场景尚未核验；
缺当前安装/本机文件的扩展已移出本轮计划，限制保留，不用 session-store 代替 trace DB。

<a id="prerequisites-for-statistical-source-selection"></a>

## 统计选择前置条件

应用管理的 Copilot 遥测目录只传给 OTel，不广播为所有适配器的手工根。
原生 vscode-copilot-chat 仍读 chatSessions；两者是不同文件格式，统计按下述规则择一。
同一物理文件只保留一个有效归属；旧版错误登记的无用量来源退出数据源展示，
未识别且未提交游标/用量的文件可由已识别格式接管，不清库、不丢诊断或停用设置。
旧登记恢复须沿注册表的真实 discover 路径核对；有些适配器会提升到父目录，
不能只匹配传入目录。仅重建应用管理输入的发现路径，有用量/周期历史的来源不隐去。
文件首行允许 metrics/logs，仍只采集已核验调用 span，不能把 metrics 当逐次用量。

`set_source_enabled` 只停止扫描，`load_daily_rows` 不按 enabled 排除历史贡献。
“停用原生源，再加 OTel”不能撤回已入库重叠用量。VS Code file parser
使用 trace+span 联合身份；跨文件/接收器副本在相同原主机和用户范围只采一次贡献。
原生 turn 与 OTel 调用没有共同调用 ID，不能按时间接近或 token 相等配对。
接收器混合多个 Agent 时，也不能为切换 Copilot 而停掉整个 OTel 来源。

已核验的 VS Code file 采用同原主机、用户、会话及本地日的统计来源选择。
有合法 OTel 调用才建立选择，原生记录保留并从所选分区贡献中排除，后续重扫继续遵守；
其他会话/日期仍读原生，已封存原生分区不叠加新遥测。选择同步日/小时、图表和导出。
开启当天可能缺少导出前调用，界面明示覆盖范围，不承诺无损拼接。
会话身份缺失不替代原生，不按 turn 时间截断跨切换点的整轮消费。

本轮已补无 usage 的 chat 调用计数、trace+span 联合身份与 token 整数严格校验；
混合 logs/metrics 跳过，SDK CLIENT 数值与 OTLP 枚举区分。VS Code 嵌入宿主有本机核验结果，
子 Agent 及其他 runtime 的分类与真实文件关系仍须独立核验。
service.name 可配置，不是宿主身份证明。file、接收器与 SQLite 同源优先用明确
身份关联，缺少可核验的关联依据时仍选择一种统计来源。

<a id="acceptance-and-rollback"></a>

## 验收与回滚

配置回归已覆盖 JSONC 注释/BOM、已有 profiles、已知策略/环境冲突、
目录不存在、只读文件、并发编辑、重复应用、失败恢复及撤销时用户已改值。
接入对照独立预期、明细和汇总，覆盖失败无 token、混合 Agent、父子 span、重传、
file/HTTP/SQLite 同源、来源切换及清理后的归档贡献。
portable/custom user-data 自动发现未实施。本轮只读核验用户此前一键配置产生的
真实 VS Code file；没有再次写用户配置、重载宿主或发起模型调用，写入/重载操作不作为本轮核验结果。
真实验收应记录写入、重载与用户正常调用产生文件的结果；合成样本不冒充真实调用。

trace SQLite 后续只读连接或使用 IDE 自带导出，尊重 WAL；不复制裸 `.db` 丢失
未 checkpoint 数据。只查询必要字段，不导出正文或 span_events；未知 schema 保持
受限，不将 trace DB 与 chronicle/session-store 混为同一数据库。
撤销配置不删历史统计，停止本功能导出与后续读取，统计主来源可独立切回。

<a id="sources-and-this-rounds-checks"></a>

## 依据与本轮验证

共同字段：verified_at=2026-10-01，owner=仓库维护者；相关任务前、产品升级、
配置/导出格式或策略变化时复核。安装版本沿用 M9，不宣称本轮安装态复测。
C01 的 VS Code file 本机数据验证日期更新为 2026-10-02，其他产品面不扩大验收范围。

| ID / scope | source_url / source_version | method / status / impact |
| --- | --- | --- |
| C01 扩展逐调用、file 与 trace DB | [官方监控说明](https://code.visualstudio.com/docs/agents/guides/monitoring-agents)，2026-09-30 页面 | 官方正文；2026-10-02 VS Code file 30 个真实 chat 调用验收，trace DB 未实施 |
| C02 CLI 变量与 CLIENT chat | [CLI 官方参考](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-command-reference#opentelemetry-monitoring)，rolling | 官方正文；影响启动脚本与字段，真实新导出未验收 |
| C03 独立 Agent Host、file/DB、启动绑定 | [固定源码说明](https://github.com/microsoft/vscode/blob/dc546cc3c9979a19adafccd439889d7b64298def/src/vs/platform/agentHost/OTEL.md) | 固定提交正文；影响宿主选择，main 快照不等同安装版本 |
| C04 file 与 SQLite 形状 | [fileExporters](https://github.com/microsoft/vscode/blob/dc546cc3c9979a19adafccd439889d7b64298def/extensions/copilot/src/platform/otel/node/fileExporters.ts)、[otelSqliteStore](https://github.com/microsoft/vscode/blob/dc546cc3c9979a19adafccd439889d7b64298def/extensions/copilot/src/platform/otel/node/sqlite/otelSqliteStore.ts) | 固定源码；file 一行一 span，DB 有 schema_version/spans/span_attributes；后续适配依据 |
| C05 策略/环境与 telemetry 开关 | [otelConfig](https://github.com/microsoft/vscode/blob/dc546cc3c9979a19adafccd439889d7b64298def/extensions/copilot/src/platform/otel/common/otelConfig.ts)、[官方策略](https://code.visualstudio.com/docs/enterprise/manage-ai-settings#configure-telemetry-export-with-opentelemetry) | 优先级表述有差异，实现按安装版本判断；不强行覆盖 |
| C06 JetBrains 1.18.0-261 | [现有分析](../../validation/desktop-usage/m9-jb-copilot-analysis.md) | 静态核验结果；真实环境缺失，手工步骤限定版本 |

公开源码快照及 SHA256 在忽略目录 `build/copilot-otel-setup/`。
前一轮文档阶段从仓库根执行 `npm run lint:md`（162 文件）、
`python build/copilot-otel-setup/check-links.py`（4 文档、77 个相对链接）和
`git diff --check`，均退出码 0。未改业务代码，不重跑 Rust/浏览器测试；
未启用本机 OTel，未产生模型调用。本轮业务与新增来源验证见上方实施记录。
