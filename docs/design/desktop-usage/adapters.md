# Agent 接入调研与能力矩阵

初始调研日期：2026-09-24；2026-09-27 复核归档行为，见文末及
[本轮验证](../../validation/desktop-usage/review-2026-09-27.md)。
国内外流行 Agent 覆盖扩展调研（A25–A48）与 M8 文档级
实施均已完成（[M8 验证记录](../../validation/desktop-usage/m8-second-batch.md)）：
18 个适配器（17 个解析 + Qoder 探针）已注册；Amazon Q/Codebuff/iFlow 经源码核验
证实本地无逐次 token 载体（或已停服），按边界排除。
下表区分实施候选与核验状态。
实施阶段已按授权只读核对部分本机数据，未启动 Agent 或发起模型请求。
用户列出的 Codex 重复项合并，CLI/桌面/IDE 仍须分别标识产品表面。
Harness Agent 按用户提供官网更正为 Hermes Agent。所有工具均保留本地支持计划；
按用户最新决定，缺少本地格式依据的 IDE 后移 F1，当前不探测/实施
（Junie CLI 与 Zed 内置已取得本地载体依据，转 M8；见扩展覆盖）。
实施阶段可提取本机真实 Agent 数据验证，流程见 [开工准备](implementation-readiness.md)。
不以企业 API、账号报表或远程日志替代本机来源。

## 如何解释支持

- **本地候选**：官方资料或源码证实存在可用存储/字段，仍需版本绑定的脱敏 fixture。
- **需启用**：有官方遥测或导出能力；未启用之前通常不能回填历史。
- **有条件**：需要特定本地版本、日志开关或本机会话导出，不能保证默认安装可用。
- **待证实**：只有原型解析逻辑或产品说明，尚缺本地字段/格式的可核验依据。
- **后续 F1**：用户已确定后移的 IDE 产品/变体；不是“不支持”的技术结论，不参与首版必验。

实现后的状态另用 `supported / partial / unsupported_format / not_found / disabled / error`，
并展示 token、缓存读、缓存写、逐次请求、模型、时间、费用、延迟分别是否可用。
来源“有文件”与格式“支持”必须分开；无法读取时不得生成全零记录。
未知或缺失版本默认按 [最新解析器兼容策略](architecture.md#unknown-version) 尝试读取。
成功解析的数据可正常统计，并单独展示未验证兼容状态、所用解析器及覆盖缺口；
不能仅因版本未收录就设为 unsupported_format，也不能把兼容尝试成功写成逐版本验证通过。

## 用户指定工具

来源编号对应 [官方与源码索引](research.md#agents)。表中的字段为已观察到的上游能力或原型候选映射。

| 工具/变体 | 拟接入来源与可用信息 | 边界与下一步 | 阶段/依据 |
| --- | --- | --- | --- |
| Claude Code | projects 下会话/子 Agent JSONL；官方 OTel 的 input/output/cacheRead/cacheCreation、模型与请求事件 | 本地 JSONL 逐请求形态需 fixture；遥测需显式启用，防会话/辅助统计重叠。**2026-09-30 真实样本（2.1.197，WSL 未登录首跑）：queue-operation/attachment/last-prompt 元数据记录已放行、`<synthetic>` 占位 assistant 不计调用（[WSL 记录](../../validation/desktop-usage/wsl-agent-installs.md)）；真实用量样本仍待有凭据环境** | M2/M5；本地候选/需启用，A01 |
| Cline | 适配器已实现（M3，2026-09-25，源码依据：固定版本源码 dcf8c3c；本机未安装 not_found，真实数据验收后置） | say 载体四桶互斥 + request 键；compaction 估算不入账；deleted_api_reqs/subagent_usage 聚合处理；不能每条 say 算请求；模型逐请求归属需真实样本核验；**2026-09-30 WSL 发现 Cline CLI 新产品面 `~/.cline/data/db/*.db`（sessions/schedules 等表，0 行）——与已实施的 VS Code 扩展形态不同源，尚无用量样本，未实施（[WSL 记录](../../validation/desktop-usage/wsl-agent-installs.md)）**；**2026-09-29 Roo 核验发现 Roo 血统 tokensIn 已含缓存（b867ec9 三处核验依据），与本适配器四桶互斥口径存在血统分歧，待真实样本复核** | M3 已实施（待真实验收），A03 |
| CodeBuddy Code / IDE / 插件 | **扩展存储适配器已实施（本机真实核对 PASS，2026-09-30）**：`%LOCALAPPDATA%\CodeBuddyExtension\Data\...\history\<session>\<conversation>\index.json` 的 `requests[].usage` 请求级聚合（`inputTokens=cacheTokens+cachedMissTokens`），消息 `extra.modelId` 归属模型，见 [M4/M5 记录](../../validation/desktop-usage/m4-buddy-local.md)。CLI 载体：官方目录文档确认 `~/.codebuddy/projects` 会话 JSONL；第三方解析器与测试证实 `message.usage` / `providerData.rawUsage` 可读逐次 token，`input_tokens` 已含缓存读。官方 monitoring 文档另提供需启用的 OTLP/HTTP protobuf `model_stream` | 扩展 usage 为请求级聚合（非逐 LLM 调用）；全零请求无模型调用不入账；`credit` 积分与 `lastTokens` 语义尚未核验不映射；同请求跨 profile/workspace 树复制按请求 id 键幂并不双计；CLI JSONL 本机仍无真实样本（文档级）；OTLP 与本地载体同时启用可能重复计数，应择一使用 | 扩展存储 M5 已实施（真实核对）；CLI 本地文档级适配器已注册，真实样本待完成，A04 |
| Codex CLI / 桌面 / IDE | 原型读 CODEX_HOME 下 rollout 的 token_usage_record；官方 OTel 支持请求、响应完成 usage 和流事件 | 已验证版本（逐版本脱敏 fixture）：0.155.0-alpha.16.3（M0/M2-A）、0.154.0-alpha.6.1/6.2、0.153.0（M2-D），逐次/累计/turn_context 三类记录，compaction 重置累计快照；0.139–0.151 已有 rollout_legacy 专用实现及 21 个精确版本映射，按 token_count 增量记录处理，边界见 m2d 记录；未知新版本默认 latest_fallback 带兼容标记；原型固定默认路径需改为发现/配置；旧 token_count 与新记录必须分格式；模型从结构化上下文归属，缺 response ID 不碰撞 | M2/M5；本地候选/需启用，A02 + 原型 |
| GitHub Copilot CLI | **适配器已实现（真实数据核对 PASS，2026-09-29）**：session-store.db assistant_usage_events（schema_version=8，36 行真实 fixture+期望值）；OTel 路径：`COPILOT_OTEL_FILE_EXPORTER_PATH`（JSON-lines，行级 schema 未文档化）/OTLP（默认 http/json；chat span gen_ai.usage.* 含缓存细分、TTFT；invoke_agent 汇总 span 官方警告不得求和双计）。**2026-09-30 复核：最新 CLI 的 `~/.copilot/session-store.db` 已改为 chronicle/search 索引（sessions/turns/checkpoints/search_index，无 assistant_usage_events），逐次用量载体外移；discover 增补 `COPILOT_HOME` 覆盖，detect 命中 chronicle 指纹记 UnsupportedVersion（不误报非 Copilot 库）。VS Code 扩展面 globalStorage 的同族 chronicle 库亦无 usage（turns 为纯文本）——VS Code 面用量经 copilot-chat 适配器（A06 行）接入，与 CLI 载体互不重复** | assistant_usage_events 为已验证主载体（旧版）；**最新版本已移除该表：`session-state/<id>/events.jsonl` 事件日志经同族 copilot-agent 转录核验无逐次 token 字段，`copilot-user-cache.json` 仅账户额度（premium_interactions）非逐次——最新 CLI 本地无已验证逐次用量，待真实最新 CLI 样本或启用 OTel 后锚定**；OTel file exporter 行级 schema 待本机样本（otel 适配器同族容错）；premium 倍率/nano AIU 不入 token；2026-09-25 消失为升级迁移、已恢复 | 主载体 M5 已实施（真实核对，旧版 schema=8）；最新版布局变更已识别（fail-closed），真实样本/OTel 后置；OTel 载体 M5 已实施（文档级），A05 |
| JetBrains 内置 AI Assistant / Junie | IDE 插件本地 schema 尚未核验，保留在 F1；Junie CLI 已取得本地格式依据，转入 M8：`~/.junie/sessions/<id>/events.jsonl` 逐轮 modelUsage（input/output/cache/reasoning/cost/time/provider），timestampMs 为结束时刻 | A07 只证实企业远端分析，已排除；Junie CLI（A36）不再列为待核验；AI Assistant IDE 插件本地 token/cache schema 待核验，当前不探测/开发 | IDE 插件 F1；Junie CLI M8，A07/A36 |
| DeepSeek Harness（DSH） | 适配器已实现（M3，2026-09-25，文档依据：token-meter README 46a7f68；本机未安装 not_found） | final 替换流式、retry 新开计费 attempt、边界定稿末样本（V03 数学样本语义）；pressure 估算不计账；occurred_at 用观察时间（无逐事件时间）；落盘行形状为合成假设待真实样本 | M3 已实施（待真实验收），A08 |
| Hermes Agent（原需求 Harness Agent） | 适配器已实现（M3，2026-09-25，源码依据：固定版本源码 ef70b36；本机未安装 not_found） | session_model_usage 按合同映射区间汇总（不虚构逐请求/每日分桶）；api_call_count 不拆调用；跨日归属/辅助互斥/压缩继承按 A24；日志详单能力待真实样本 | M3 已实施（待真实验收），A24 |
| OpenClaw | 适配器已实现（M3，2026-09-25，文档依据；本机未安装 not_found） | 官方文档未给出表级/条目级 schema ⇒ 运行时库与旧归档均 fail closed（诊断注明待真实样本）；归档按迁移输入降级（doctor --fix 路径）；不沿用“只扫 JSONL”假设；远端 Gateway 不等于本机 | M3 已实施（fail-closed 占位，待真实样本扩展），A09 |
| Gemini CLI | 官方会话存储 ~/.gemini/tmp/<project_hash>/chats/ 含 token；OTel 提供 input/output/thought/cache/tool 和请求统计 | JSON/JSONL 具体版本探测；thought/cache/tool 字段的包含关系逐 provider 核验；Gemini 网页和 Code Assist 不冒充 CLI | M2/M5；本地候选/需启用，A10 |
| Kilo Code CLI / 桌面 / IDE 扩展 | 适配器已实现（M3，2026-09-25）：SQLite 只读 + 暂存副本合同；message.data.tokens 逐次五互斥；水位增量；session 行 tokens 列仅作会话对账（275/276 matched，列级语义随版本不稳定不映射事件）；真实核对 13,342 调用/1.65B token 与独立求和一致、幂等 | 已验证 7.4.8/7.4.9/7.8.1（fixture）；库内其余 7.3.42–7.7.12 等未收录走 latest_fallback；7.8.1 为本机实读脱敏（顶层 modelID/providerID 载体，与 7.4.x 同形共用 message_tokens_v1）；新 core 数据层（session_message，当前 0 行）切换后需专用实现核验；IDE 扩展后移 F1；不能拿 CLI 通过代表 IDE 通过 | CLI/桌面 M3 已实施；本地用量格式尚未核验的 IDE 变体 F1，A11 |
| 新版 Kimi Code | 适配器已实现（M4，2026-09-25）：家族共享 kimi_wire.rs；注册表锚点 protocol_version=1.5；真实核对 13 文件 965 调用 total 116,428,813 与 jq 逐字段相等、幂等；打断步回声缺失以 echo_subset 可见 | usage.record 无稳定 ID ⇒ provider/逐次延迟不入账（如实 unavailable）；旧会话 1.4 走 latest_fallback；其他版本仍需各自 fixture | M4 已实施；跨版本待核验，A12 |
| Kimi Work | 适配器已实现（M4，2026-09-25）：独立目录（A13 不与 Kimi Code 合并）；conv-*/ctitle-* 布局（daimon 宿主）；锚点 1.4；真实核对 69 文件 1,337 调用 total 125,225,933 逐字段相等、69/69 对账 matched；跨文件同毫秒（swarm）事件键含 session:agent 身份段 | 官方默认布局/env 未见文档（迁移需手工加根）；subagent.completed 与缓存写>0 无本机真实样本；与 Kimi Code 共享 wire 解析但统计分列 | M4 已实施，A13 |
| MiMo Code | 适配器已实现（M3，2026-09-25，源码依据：固定版本源码 456678b；本机未安装 not_found） | part/step-finish 与 OpenCode 同形；message.agent_id 为互斥指纹锚点；session 无累计列（无对账目标，如实标注）；MIMOCODE_HOME/data 路径解析；不从 fork 关系推兼容（双向 fail closed 已测） | M3 已实施（待真实验收），A14 |
| oh-my-pi | 官方 session 文档确认 ~/.omp/agent/sessions/...jsonl，assistant usage 与模型/provider | 本机 18.2.7 已取 3 个脱敏 fixture（2026-09-25，[M2-B/C 恢复记录](../../validation/desktop-usage/m2bc-resumed.md)）：assistant 全带 usage + duration/ttft 浮点毫秒；model_change 落盘为 `model`="provider/model" 组合字段（非 pi 分字段）；子 Agent 文件在 `<ts>_<父UUID>/` 目录按路径形状归父；~/.omp/logs 仅上下文估算 debug 行、无逐次用量，title-generator 尚无重叠依据；compaction/branch_summary 本机均无 usage（辅助载体合同已实现，待真实样本） | M2；本地候选，A15 |
| pi | 固定源码 Usage 含 input/output/cacheRead/cacheWrite/reasoning；session 有 message、独立 usage、compaction/branch summary usage | 推理已含于 output；仅 assistant message 会漏辅助用量；分支继承不代表新调用；目录由该版本配置函数解析；本机 0.87.1 已取真实 fixture 并通过幂等核对（2026-09-25，[M2-B/C 恢复记录](../../validation/desktop-usage/m2bc-resumed.md)）；fork 复制条目带 fork 会话身份，仲裁为 conflict 保留先扫者，净效果不双计 | M2；本地候选，A16 |
| OpenCode | 适配器已实现（M3，2026-09-25，源码依据：固定版本源码 0027387；本机未安装 not_found） | 逐次数据可得：part 表 step-finish 部件 tokens{input(未缓存),output,reasoning,cache{read,write}}+cost；session.tokens_* 仅对账（matched）；message.data turn 聚合不读防双计；发现层按数据目录名收敛（kilo 目录同形库碰撞已修，负向测试覆盖） | M3 已实施（待真实验收），A17 |
| Qwen Code | 固定 recording service 记录 usageMetadata/model；旧 tmp 与新 projects 的 chats/ 及 chats/archive/ 均需发现；官方 OTel | 不把 Goal 累计 histogram sum 当总消耗；活动/归档以同来源原生 uuid 去重；QWEN_RUNTIME_DIR 优先于 QWEN_HOME，手工根可补旧库；旧日志缺 spend 时零不等于实测零；显式禁用 prompt 日志 | M2/M5；本地候选/需启用，A18 |
| Zoo Code | 适配器已实现（M3，2026-09-26，源码依据：固定版本源码 f780647；本机未安装 not_found） | Roo 血统整写 ui_messages.json；LIFO 配对合并（finish 覆盖 start、无配对丢弃）；condense_context.cost 计上游 totalCost（辅助调用、token 未知）；tokensIn 含缓存（与 opencode 家族相反）；独立产品不能擅自改为 Roo Code；本地用量格式尚未核验的 JetBrains 变体后移 | M3 已实施（待真实验收）；JetBrains 变体 F1，A19 |
| TRAE / TraeCode 及插件 | 后续探测本地 IDE/CLI 日志、会话库、插件数据和本机会话导出 | A20 企业 API/控制台报表不纳入；2026-09-29 复核：tokscale 的 TRAE 路线源自其远端 usage API 落盘缓存（dollar_float/extra_info 形态），同样不符合本机来源边界；本地逐次格式仍缺少可核验的资料，整个产品家族继续后移 | F1；后续支持，A20 为范围排除依据 |
| VS Code | 原生 chatSessions v3：promptTokens 为末次调用输入下界、completionTokens 为整轮累计；modelTotals 提供整轮逐模型总量；toolCallRounds 带 ID/时间戳的主循环轮次计调用；仅接入请求 agent.id 为 github.copilot 命名空间的记录。workspaceStorage 与 globalStorage/emptyWindowChatSessions 共用安装来源；本机 10 turn/217 round，输入下界 3,408,279、输出 320,141，见 [审查记录](../../validation/desktop-usage/m9-copilot-review.md)。2026-10-02：一键配置 file 导出本机 30 个 CLIENT chat span 已验收，见 [补充记录](../../validation/desktop-usage/dashboard-repair.md) | copilot-chat 全量重放完整快照；turn/逐模型汇总计 usage_observation，round 计 model_call，逐轮 token 未知；modelTotals 替换旧贡献、模型换序/副本/流式更新不重复累计。末次输入和整轮输出不派生完整总 token；缓存默认未知。应用管理的 OTel file 自动发现，同原主机/用户/会话/本地日择 OTel，保留原生历史及开启前覆盖缺口；trace+span 同源副本去重 | M9 原生和 VS Code file 均有本机真实验收；CLI/JetBrains OTel 仍文档级需独立验收，A06 |
| GitHub Copilot for Visual Studio | **VS 自动遥测（2026-10-01 本机真实验收）**：`%TEMP%\VSGitHubCopilotLogs\traces\<hex>_VSGitHubCopilot_traces.jsonl`——OTLP JSON（行=resourceSpans 批次、service.name=vs-copilot），`chat <model>` span 每 LLM 请求一个：`gen_ai.usage.input_tokens/output_tokens/cache_read.input_tokens`（intValue 字符串形）、模型/会话 id/纳秒时间戳；无需任何配置，见 [M9 记录](../../validation/desktop-usage/m9-vs-copilot-local.md)（2 span input=17,470/output=219/cache_read=13,184 与独立提取一致、重扫幂等）。会话文件（`VSGitHubCopilot\copilot-chat\…\sessions`，MessagePack）无逐次 token（响应正文 EncryptedContent、模型目录 InputTokens=上下文上限、Quotas 无 Usage），不接入 | vs-copilot 适配器（M9 已实现）：invoke_agent 整 turn 汇总 span 跳过（官方 OTel 防双计同族规则）；TEMP 载体随清理/实例滚动——不承诺完整历史；与 otel 接收器捕获的同源 span 同 agent 维度（vs-copilot）择一；token 桶并列报告不派生 uncached（包含关系未由 VS 文档声明）；`gen_ai.input.messages` 提示正文只读白名单键不入库 | M9 已实施（本机真实验收），A06 同族扩展 |
| GitHub Copilot for JetBrains | **源码核验（2026-10-01 插件 1.18.0-261 解包，[M9 分析记录](../../validation/desktop-usage/m9-jb-copilot-analysis.md)）**：默认本地数据无逐次 token——会话库为嵌入式 Nitrite `copilot-agent-sessions-nitrite.db`（NtAgentSession：turns/模型/**turnCreditsJson=RestoredTurnCredits(messageId,credits)**，credit 非 token 不折算）、日志走 idea.log、App Insights 为远程（排除）；**逐次 token 载体=需启用的 OTel 导出**：插件设置 `otelEnabled`/`otelExporterType(file/otlp-http/otlp-grpc/console)`/`otelOutfile`/`otelServiceName`/`otelCaptureContent`（=官方文档 Agent debug File Logging），与 VS Code `github.copilot.chat.otel.*` 同族同词汇；插件自身 Debug Panel 即从 outfile 解析 `gen_ai.usage.{input,output,cache_read,cache_creation}*` 五桶（OTelSpanProvider 字节码核验依据） | 不新增默认载体适配器（默认仅 credit，违背逐次 token 诉求）；接入路径=既有 otel 适配器（M5）：用户在 Settings → Tools → Copilot → Chat 启用 file 导出后把 otelOutfile 加为手工根；行格式同族容错、本机无 JetBrains 待真实样本锚定；otelCaptureContent 含提示正文——本应用只读白名单键 | 经 otel 适配器接入（需启用载体，文档级），A06 同族扩展 |
| ZCode | 适配器已实现（M4，2026-09-25）：model-io JSONL 双口径——AI SDK 五键为主（inputTokens 含缓存读）、anthropic snake_case 对照互斥校验（矛盾进诊断）；db.sqlite model_usage 为自动采集主载体，turn_usage 只读对账（活库不一致轮可见）；真实核对幂等 | 已验证 3.14.3（真实 fixture）；未收录版本 latest_fallback；`~/.zcode/v2` 布局与 `%APPDATA%/zcode` 桌面存储未接入（待核验）；缺 requestId/traceId 时不得全部变成 zcode:None | M4 已实施；跨版本待核验，A21 |
| WorkBuddy | 第三方开源解析器证实 `~/.workbuddy/projects/**/*.jsonl` 会话逐次用量字段；本机 8 文件/420 事件真实只读核对、重扫幂等通过（[记录](../../validation/desktop-usage/m4-buddy-local.md)） | 独立来源，未套用 CodeBuddy CLI OTLP；`traces/` 的总量与会话明细重叠关系未核，暂不叠加；套餐/账号积分页不纳入 | M4 已实施并核对当前本机格式；跨版本/trace 待核验，A22 |
| Zed | 内置 hosted Agent 已取得本地载体依据（A38 补充 A23）：threads.db threads 表 data blob（json 或 zstd）含 request_token_usage 逐次（input/output/cache_read/cache_creation）与 cumulative_token_usage、created_at/updated_at、folder_paths | 仅统计 provider=zed.dev 的 hosted 调用；imported 线程跳过；zstd 解压与体积上限防护；外部 ACP Agent 仍按底层适配器读取原生日志，不计为 Zed 内置支持 | 内置 M8 已实施（文档级+本机 schema 核验，2026-09-29）；外部来源按原阶段，A23/A38 |

本轮没有将任何“未发现文档”写成“该产品不可能支持”。
若不能取得可靠用量，仍可展示该工具状态与限制，但不占据有数值的总计行。

<a id="扩展覆盖"></a>

## 扩展覆盖

<a id="扩展覆盖2026-09-29-第二批调研"></a>

下列产品为本轮补全的国内外流行 Agent，来源编号见 [调研索引](research.md#agents)
（A25–A48）。已完成 M8 文档级实施（[验证记录](../../validation/desktop-usage/m8-second-batch.md)）：
各适配器独立目录 + 版本注册表 + V30 结构检查 + 合成 fixture 合同测试
（tests/m8_contract.rs 19 项）；本机均无真实数据（Zed 空库），真实验收后置。
闭源产品的字段依据来自第三方解析器或逆向分析，真实脱敏 fixture（许可已给）
出现后升级验证；一切估算路径（Kiro Auto 补零、Grok 累计差额、Goose reasoning
差额等）不采纳，仅采信原生计数。

| 工具/变体 | 拟接入来源与可用信息 | 边界与下一步 | 阶段/依据 |
| --- | --- | --- | --- |
| Amp（Sourcegraph，闭源） | `~/.local/share/amp/threads/T-*.json`：messages[].usage（model、inputTokens/outputTokens、cacheRead/cacheCreation、credits）与 usageLedger.events（timestamp/model/credits/tokens 五桶）双载体 | ledger 与逐消息 usage 按 messageId+桶对账防双计；缺显式时间戳不得以 thread created+messageId 推造逐次时间；credits 是计费单位非美元；schema 随版本滚动需 fixture | M8 已实施（文档级 2026-09-29；ledger 为主、消息 usage 仅对账不推造时间；credits 不映射），A28 |
| Goose（Block；仓库已迁 aaif-goose） | `sessions.db` sessions 表：会话级 total/input/output（单次与 accumulated_* 双列）、model_config_json、provider_name、created_at；GOOSE_PATH_ROOT 与 macOS/旧 Block 多根 | 仅会话级聚合（按 Hermes 区间语义展示，不虚构逐请求）；无缓存列；reasoning 差额推算不采纳；单双列语义随版本核验 | M8 已实施（文档级；官方 usage_ledger 逐请求 + 旧库 accumulated 兜底；estimated/carried_forward cost 不映射；reasoning 差额不采），A26 |
| Crush（Charm） | `~/.local/share/crush/projects.json` 注册表映射每项目 data_dir 与 crush.db；sessions 表 prompt_tokens/completion_tokens/cost、messages 表 model/provider、父子会话树 | token 列与消息部件的关系按固定源码核验（第三方仅采信 cost）；只取根会话防父子双计；按项目库发现 | M8 已实施（cost-only：官方证实 token 列是上下文快照不采；根会话 cost 累计、子会话回卷已过滤），A27 |
| Roo Code | VS Code globalStorage `rooveterinaryinc.roo-cline/tasks/<uuid>/`（ui_messages.json、api_conversation_history）及 .vscode-server 变体 | Cline 血统同构，解析组件可复用但删除/子 Agent/压缩行为分版本复测；与 Cline/Kilo 扩展目录区分；CLI 形态另证 | M8 已实施（文档级 b867ec9：完整 say/ask 枚举、api_req_deleted 备忘不计、condense 辅助；与 cline 的 tokensIn 口径分歧已登记），A29 |
| Aider | 默认仅 .aider.chat.history.md/.aider.input.history 落盘（无逐次 usage）；`--analytics-log`/llm 历史日志可选本地 JSONL（message_send 含 token/cost） | 可选日志属需启用载体，不回填历史；字段与单位待 fixture；不走 tokenizer 估算 | M8 已实施（--analytics-log 需启用不回填；prompt 含 cache 写、无 cache 分项；cost=litellg 自算 Estimated；无默认路径仅手工根），A30 |
| Continue（CLI/VS Code/JetBrains） | `~/.continue`（sessions、logs、index）目录由官方文档证实 | 会话载体逐次 token 字段未核验，先核验再实施；hub 账号数据不接入；JetBrains 插件载体另验 | M8 已实施（仅 CLI 会话顶层 usage：会话级聚合；GUI 无字段、devdata 估算不采），A31 |
| Droid（Factory.ai，闭源） | `~/.factory/sessions/{uuid}.settings.json` tokenUsage（input/output/cacheRead/cacheCreation/thinking，累计）+ 同名 jsonl 转录 | 累计值保留区间语义，分摊到回合属估计不采纳为逐次；无费用字段；providerLock 与转录时间归属待样本 | M8 已实施（tokenUsage 累计快照→会话聚合；转录字节分摊不采），A32 |
| Amazon Q Developer CLI | `~/.aws/amazonq/history/` 按时间戳 JSON 会话（第三方资料）；官方仓库开源 | history 内 usage/token 字段未核验，先源码级核验再实现；SSO 重登录可能丢历史，保留缺口可见 | 不实施 token 适配器（2026-09-29 源码核验：API tokenUsage 在类型转换层被丢弃，conversations/history 均无 token 字段），A33 |
| Grok Build（xAI，闭源） | `~/.grok/sessions/<workspace>/<session>/`（updates.jsonl、signals.json、summary.json、events.jsonl）与 `~/.grok/logs/unified.jsonl` | 仅取显式 usage 块五桶；累计 totalTokens 增量与压缩差额补偿是推断不采纳；PID 复用/子代理模型归属复杂，归属依据冲突时保持 unknown；GROK_HOME 覆盖 | M8 已实施（仅 updates.jsonl 显式 usage 块；累计差额/补偿/PID 归因不采），A34 |
| Antigravity CLI/扩展（Google） | `~/.gemini/antigravity[-cli]/conversations/<uuid>.db`：gen_metadata protobuf 逐回合 usage（input=固定系统提示+新增、cacheRead、output、thinking、responseId） | protobuf 布局为逆向结论且 1.1.18 时间戳字段变更，须逐版本锚定；IDE 主体用量走 language server，另按需启用核验；与 Gemini CLI 目录同根不同子目录 | M8 已实施（逆向 protobuf：input=#1+#2、#9.#4 时间戳；1.1.18+ 无时间戳行 fail closed；IDE language server 载体尚未核验不实施），A35 |
| Junie CLI（JetBrains） | `~/.junie/sessions/<session-id>/events.jsonl`：LlmResponseMetadataEvent.modelUsage[] 逐轮（model、input/output、cache、reasoning、cost、time、provider） | timestampMs 是结束时刻，起始时间=结束−time 仅在两者齐备时计算；会话名时间戳仅作兜底；JetBrains AI Assistant IDE 插件仍 F1 | M8 已实施（modelUsage[] 多别名组；timestampMs=结束时刻、time→延迟；缺时间戳不入账），A36 |
| Kiro（AWS，CLI+IDE） | 三载体：CLI `~/.kiro/sessions/cli/*.json(+jsonl)`；kiro-cli `~/.local/share/kiro-cli/data.sqlite3` conversations_v2；IDE globalStorage `kiro.kiroagent`（.chat 快照、execution、promptLogs） | Auto agent 常记 0：估算路径不采纳，仅采显式计数；execution 与 .chat 快照按 executionId 抑制重复；metering credit 独立计价单位；三载体交叉去重 | M8 已实施（CLI turns 真实计数 + kiro-cli SQLite request_metadata；Auto 零计数/估算不采；IDE 估算载体不实施；双载体重叠待真实样本对账），A37 |
| Zed 内置（升级自 F1） | threads.db（见上表 Zed 行）：request_token_usage 逐次五桶 | 仅 zed.dev hosted；imported 线程跳过；zstd blob 解压上限；threads 表可选列容错 | M8 已实施（官方源码 bd74733 + 本机 schema 核验（0 行）；cumulative 权威、request 桶覆盖语义仅对账；仅 zed.dev；zstd 有界解压），A23/A38 |
| Codebuff（原 Manicode） | `~/.config/manicode*/projects/*/chats/<chatId>/chat-messages.json`；CODEBUFF_DATA_DIR 覆盖 | usage 字段与分通道根（manicode/-dev/-staging）待 fixture | 不实施 token 适配器（2026-09-29 官方源码 caec5fc 证实本地仅 credits、无 token 字段；CODEBUFF_DATA_DIR 非官方变量），A39 |
| Command Code | `~/.commandcode/projects/<slug>/*.jsonl` v3 树形（session/message/model_change；assistant usage 五桶+costUsd）；配置 config.json | rewind 孤儿分支不计；fork 复制按 id+时间戳去重；checkpoints 文件跳过；全零 usage 视为已报告零 | M8 已实施（npm 1.69.0 分发物核验依据：inputTokens 含 cache、当前路径口径、fork 按 id+时间戳跨文件去重、全零=已报告零），A40 |
| jcode（jcode.sh，开源 Rust） | `~/.jcode/sessions/session_*.json` 快照 + `.journal.jsonl` 追加（journal 值权威覆盖快照）；五桶 token 字段 | OpenAI/Anthropic 缓存口径差异（cache_read 是否 input 子集）逐版本归一；回合计数防 journal 重放双计；JCODE_HOME 覆盖 | M8 已实施（快照+journal 合并、journal meta 权威；openai/anthropic 缓存口径分列；崩溃窗口按消息 id 兜底），A41 |
| gajae-code（gjc） | `~/.gjc/agent/sessions/<slug>/*.jsonl`（pi 血统：session 头 + assistant model/provider/usage 五桶 + cost.total USD） | 深度 1/2 子代理重放按会话 id+消息 id 去重；GJC_CODING_AGENT_DIR/GJC_CONFIG_DIR/PI_CONFIG_DIR/XDG 多根；pi 家族复用须逐项测试 | M8 已实施（官方 7e54f9c：usage 已归一化互斥桶（pi 口径）；五桶齐全才统计；message.timestamp 毫秒优先），A42 |
| Xum（Coder；原 mux） | `~/.mux/sessions/<workspaceId>/session-usage.json`：byModel 会话级聚合（input/cached/cacheCreate/output/reasoning 各含 tokens+cost_usd）+ lastRequest.timestamp | 仅会话级（区间语义）；模型 key 带 provider 前缀需拆分；产品更名需新旧双根发现 | M8 已实施（byModel 会话聚合；cost_usd 不映射（聚合层无 cost 载体）；mux 旧根保留），A43 |
| iFlow CLI（阿里心流，闭源） | `~/.iflow`（settings.json、`tmp/<project_hash>/`）；`/chat save` 持久化 JSON 会话；OTel 事件含逐次 api_response 五桶 token（input/output/cached/thoughts/tool，与 Gemini CLI 同构） | chats 载体具体路径与 usageMetadata 字段待本机样本；Gemini 家族解析知识不得直接套用，须逐字段验证；IFLOW_* 环境层 | M8；本地候选（待样本），A46 |
| Qoder CLI（阿里，前通义灵码） | CLI 设备流 `~/.qoder/`；IDE 为 Electron `%APPDATA%\com.qoder.app.stable*`；另有 globalStorage/`~/.local/share/qoder` 线索 | CLI 会话格式与 usage 字段尚未核验：先本机核验再定实现；与灵码品牌演化记录在案；JetBrains 插件归 F1 | M8 探针级接入（路径已核验、字段尚未核验：只发现识别不解析，fail closed；待本机实测 `<session>.jsonl` 与 `state.json`），A47 |
| AtomCode（AtomGit 生态） | CLI 形态国产 Agent（联合华为 InsCode AI IDE）；开源可源码核验 | 本地存储格式尚未核验：先源码级核验（存储路径/usage 字段）再决定入 M8 或待核验；InsCode IDE 归 F1 | M8 已实施（官方 e4215f7：.meta turn_stats 按模型会话聚合、round_count 合计作调用数；旧版单文件同构），A48 |
| Warp | 本地仅账户级用量缓存（requestsUsed/spendCents/syncedAt，工作区级） | 无 token 明细且属账户额度数据：不入 token 统计，最多以状态行展示受限 | 暂缓（额度类），A44 |
| Cursor（CLI/IDE） | CLI 转录 `~/.cursor/projects/<slug>/agent-transcripts/<uuid>/*.jsonl`（有会话、无逐次 token/模型字段）；IDE state.vscdb 载体待核验 | 逐次用量仅存在于远端 dashboard（get-filtered-usage-events/CSV 导出），按本地边界排除；不以 tokenizer 从转录估算 | F1（本地用量格式待核验），A45 |
| Windsurf（IDE+CLI） | IDE Cascade 与 2026 年确认存在的 windsurf CLI；本地 usage 存储尚未核验 | 本地逐次 token 格式尚未核验时不探测/不实施；列入 F1 后续支持计划 | F1；后续支持 |

## Hermes Agent 本地合同

A24 固定源码 `ef70b3661cbfcf57e583008ad91dd04d8ba46070` 确认以下内容；
未与本机发行版对应，不能写成已经支持：

- 路径遵循 get_hermes_home：上下文覆盖、HERMES_HOME、平台默认值；当前 Windows 默认
  `%LOCALAPPDATA%/hermes`，macOS/Linux 默认 `~/.hermes`。命名 profile 有独立目录/state.db。
  采集器只探测/读取允许的根，不导入运行 Hermes 代码；不能只硬编码 Linux 默认目录。
- sessions 存会话累计；session_model_usage 按 session/model/provider/base_url/billing_mode/task
  组合键累计，含 input/output/cache_read/cache_write/reasoning、api_call_count、first_seen/last_seen。
  base_url 只在内存规范化成无凭据的 provider 标识或本机 HMAC，不持久化完整 URL/查询参数。
- update_token_counts 有增量与 absolute 两条路径；后者不能分解历史模型路由。
  record_auxiliary_usage 只写按任务累计，不写主 sessions 总量，不能简单二选一或两表相加。
  未解释残差保持未知模型/覆盖缺口，禁止强归给 session 当前模型。
- schema v20 将历史 sessions 汇总回填为模型行；v22 加 task 键，源码另有旧键修复。
  必须探测真实列与主键，不只看版本整数；旧回填行不证明历史全部调用使用该模型。
- first_seen/last_seen 是聚合写入边界，不是逐请求时间；跨日累计按区间保存，
  无详单不能生成精确每日趋势。api_call_count 先作为来源汇总，不能展开成伪造调用事件。
- agent/turn_usage.py 有响应 usage 和带 model/provider/id 等信息的日志路径；
  实际日志文件、轮转、时区、稳定关联 ID、失败覆盖和缓存缺省语义尚待 fixture。
  input/cache/reasoning 的最终包含关系还需 normalize_usage 完整路径验证，不凭列名推导。
- 父子/压缩继承、辅助任务、MoA 聚合及外部 Codex 镜像必须验证覆盖集合；
  不读取 messages/FTS 正文计数来补请求数。源 DB 只读，不能调用其可能迁移/修复的初始化 API。

首个可交付能力可为本机按模型/任务的原生区间统计；逐次请求和精确日统计独立标注能力。
计费金额字段只保留本机调用归属明确的记录，不读取 Hermes Portal 账号余额或调用 account_usage API。

## 分家族复用的范围

基础读取器可以复用 JSONL、可变 JSON、只读 SQLite、OTLP 和 CSV。
业务映射仅在字段与生命周期测试证明一致后复用：

- pi/oh-my-pi 可共享部分 Usage 类型知识，辅助事件及分支恢复仍各测；
  gajae-code（gjc）为 pi 血统变体，复用同样逐项验证。
- OpenCode/Kilo/MiMo 可共享结构探测工具，不共享未经验证的目录、表名或累计/增量假设。
  Kilo 的库内最高版本只用于格式探测；每条消息按所属 session.version 保留已验证/兼容
  依据。混合库存在未验证消息时保留兼容提示，后续只有已验证增量不能覆盖该状态；
  新版本空会话不能认证，也不降级已有消息的版本核验依据。支持依据/规则更新自动重评。
- Cline/Zoo/旧 Kilo 扩展的 API 记录可以共享解析组件，但子 Agent 汇总、删除与压缩记录逐项测试；
  Roo Code 扩展载体同构可复用组件，删除/子 Agent/压缩行为分版本复测。
- VS Code/Copilot/宿主嵌入的 Claude/Codex 不按应用品牌重复计费，使用 origin 归属关系。
- Kimi Work 和 Kimi Code 的数据根、实例身份和日志 revision 独立，重叠来源显式处理。
- Amp 的 usageLedger 与逐消息 usage、Kiro 的 execution 与 .chat 快照属于同源双载体，
  对账去重规则各自验证，不能按时间接近或数值相同猜测同一请求。
- Goose/Xum/Droid 等会话级聚合来源按 Hermes 区间语义处理，不展开成伪造逐次事件。
- Antigravity CLI 与 Gemini CLI 同根（~/.gemini）不同子目录，目录发现互不推断；
  iFlow CLI 与 Gemini CLI 格式相似仅作线索，字段仍逐个核验。

## 每个适配器的交付合同

代码布局遵守 [Agent 独立目录与版本组织](architecture.md#adapter-layout)。
每个 Agent 一个目录，历史版本实现在目录内分模块；该要求适用于本表全部阶段
（目录迁移与版本注册表已随 M2 实施，见 [架构合同](architecture.md#adapter-layout)）；
已验证支持与未知版本兼容尝试分别记录，不把尚待执行的结构调整记为已实现。

| 项目 | 必须交付的内容 |
| --- | --- |
| 目录与版本 | Agent 独立目录、统一入口、发布版本/来源格式到内部实现的映射、逐版本 fixture；新增版本保留历史实现及回归，按 V30 验收 |
| 发现 | 产品/表面/OS/版本范围、本机归属依据、候选路径、环境覆盖、profile、手工添加方法 |
| 探测 | 逐文件/数据库识别 Agent 与输入类型；已知版本按映射分派，未知版本先尝试该 Agent 最新内置解析器；结构/语义不兼容或匹配冲突时明确报错 |
| 字段映射 | 每个 token 字段包含关系、单位、时间基准、模型归属、调用 ID、调用确认依据 |
| 生命周期 | 单次/累计/流式/最终/更正/聚合、重试和分支/子 Agent/辅助调用的处理 |
| 增量 | 游标、重写/轮转检测、事务恢复、数据源自身保留和最早可回填日期 |
| 去重 | 本机副本、多个数据库、宿主镜像、本机遥测/会话导出之间的关联或主来源选择 |
| 完整性 | success-only、是否记录隐藏调用、采样/丢包/日志清理、无 timestamp 的限制 |
| 验证 | 原始版本说明、最小脱敏 fixture、人工期望值、单元/集成结果、真实应用核对结果；兼容尝试覆盖版本未收录但结构兼容、结构破坏及部分可用 |
| 维护 | 源码 commit/schema、最新默认解析器、选择依据、兼容状态、升级重试与不兼容诊断；不因未知版本号直接拒绝 |
| 定时 | 统一采集入口、增量/重扫开销、暂停/取消、无变化扫描和任务恢复；不启动 Agent |

每个样本仅保留事件类型、匿名 ID、时间、模型、数值与必要关系；正文替换为常量。
样本改变后重新计算摘要，记录脱敏方法；不得提交完整真实会话库。

## 各家族必须补的样本

| 家族 | 必需样本 |
| --- | --- |
| Claude/Codex | 一请求多个片段、无 usage 的 tool/user 消息、重复 final、旧累计与新逐次格式、模型中途切换、子 Agent |
| pi/oh-my-pi | message + 独立 usage、标题/压缩/分支总结、会话 fork 继承、错误/取消、日志辅助事件交叉去重 |
| Cline/Zoo | started/finished 合并、同记录更新、压缩、删除请求汇总、子 Agent 汇总与明细同时存在、模型缺失 |
| OpenCode/Kilo/MiMo | 活库更新旧行、新 core/旧库分辨、session 累计与 step/message 明细关系、provider 变更 |
| Gemini/Qwen | prompt/cache/thought/tool 包含关系、文件重写、provider 转换、累计遥测重置及 Goal 重复恢复 |
| DSH | 流式 usage→最终替换→同 step retry、新旧 stateVersion、context estimate 不进用量、累计值下修 |
| OpenClaw | SQLite 正在写入、旧 JSONL 与迁移库重叠、外部 CLI 镜像、嵌套会话、远端路径与非持久会话 |
| Hermes | Windows/profile 路径、模型中途切换、增量/absolute、task 辅助、v20 回填/v22 主键、跨日累计、压缩/子 Agent/MoA、缺 usage |
| Kimi Code/Work/ZCode | 真实产品版本、每次调用稳定 ID 或序号、model/provider 字段、毫秒时间、缺 ID、目录迁移 |
| Copilot/VS Code/CodeBuddy | chat/model_stream 与父 span 去重、file/OTLP 重传、cache 缺失、无 usage 的失败、SDK/宿主重叠、chatSessions 与 otel outfile 同面双计（同 agent 维度择一）、kv 采样快照与压缩重写（chatSessions 全量重放+同键幂等）、VS TEMP 遥测清理/实例滚动与 otel 接收器同维度择一 |
| JetBrains/TRAE/其他本地待核验源 | 本地日志/数据库真实样本、版本/插件差异、缺 usage、同区间重导入；远端同步/账号报表必须排除 |
| M8 双载体/对账类（Amp/Kiro） | ledger 全量与部分覆盖、execution 覆盖快照、重复事件、缺 messageId、跨载体同一请求 |
| M8 会话级聚合类（Goose/Xum/Droid） | accumulated 与单次列、跨日会话、快照+日志合并（jcode）、provider 前缀模型 key、累计下修 |
| M8 逆向/闭源类（Antigravity/Grok/Junie/Qoder/iFlow） | protobuf 布局版本变更、累计与压缩差额、缺时间戳、结束时刻语义、品牌迁移双根 |
| M8 开源同构类（Roo/gjc/Command/jcode/Codebuff/Continue/Crush/Aider/Amazon Q） | rewind/fork 分支、删除汇总、子代理重放、缓存口径差异、需启用日志、SSO 丢历史 |

## 会话归档与历史可回采性

<a id="会话归档与历史可回采性2026-09-27-复核"></a>

来源归档与本应用分级保留是两件事。发现来源时也检查已证实的归档载体；
同一来源的活动/归档副本共用原生调用身份。没有共同调用 ID 的载体必须明确主来源，
不能按时间接近、token 相同或会话 ID 相同猜测同一请求。

| 来源 | 已核验入口与行为 | 实现和核验范围 |
| --- | --- | --- |
| Codex | CODEX_HOME 下 sessions 与 archived_sessions；官方 thread/archive 移动 JSONL，恢复时移回 | 两目录合并发现，只有归档目录也可采集；活动/归档副本与反复重扫去重。当前本机 306 个活动文件、无真实归档，归档行为由官方合同及合成移动/副本测试验证 |
| Qwen Code | 旧 tmp 与新 projects 下各项目的 chats/archive 为独立归档目录；daemon 将活动 JSONL 移入或移出归档，异常情况下活动和归档副本可同时存在 | 扫描两种布局的 chats 与 archive，按同一来源内原生 uuid 去重；QWEN_RUNTIME_DIR、QWEN_HOME 与手工根按已证实路径选择，Agent 工作目录相对路径需配置绝对手工根；本机无真实 Qwen 数据，格式升级仍须逐版本核验 |
| ZCode | cli/db/db.sqlite 的 model_usage 保留已完成调用，JSONL 可能删除/压实 | DB 为同一来源的主载体，按原生 id 及来源/日快照替换；从未有 DB 时才读 JSONL。使用过 DB 后丢失或损坏即报错并保留旧统计，不回退造成双计。旧 JSONL 未识别记录若无法与 DB 调用可靠对应，保留健康告警，不直接相加或抹除 |
| Claude Code | projects 中主/子会话、orphaned JSONL 与 superseded 副本 | 现有递归发现覆盖这些路径，以请求/消息身份去重；已有合成 fixture，当前无真实本机归档可核对 |
| Kilo / OpenCode | SQLite session.time_archived 是归档标记，调用仍在 message/part | 查询不排除归档会话；补充先归档、恢复、再归档全链路测试。Kilo 使用真实脱敏 fixture，OpenCode 为固定源码合同合成 fixture |
| Gemini CLI / Kimi Code / Kimi Work / pi / oh-my-pi | 已知本地会话文件；Gemini 自动保存与手动 checkpoint 不同 | Gemini 自动清理默认约 30 天，是删除而非另一个归档目录；手动 checkpoint 可能复制会话，但没有已验证的独立逐次用量合同，不能叠加。其他来源现有目录按格式采集；omp usage_history 是额度窗口，不能当调用历史 |
| Cline / Zoo / Hermes / DSH | 按各自已验证格式处理压缩、删除汇总或区间记录 | 沿用能力矩阵，不能把汇总反推出逐次明细；无本机真实档案的来源仍待验收 |
| OpenClaw | 新运行时库与旧归档分离，缺少本项目已验证的条目 schema | 保持不计入和可见诊断，等待真实样本；不凭旧 JSONL 名称猜测兼容 |

ZCode 安装包的 recordModelUsage 会按 started_at 清理约 30 天前的记录。
DB 不是永久完整档案：边界日已有统计时不以可能不完整的快照覆盖；来源清掉的旧日保留
在应用汇总中。新发现来源只能统计当前仍存在的数据，不能恢复从未采集且已被删除的记录。
DB 没有逐行产品版本，结构校验通过后使用 latest_fallback 标记，不宣称逐版本验证。

最新真实核对：8,225 次调用、2,398,950,011 Token；再次扫描新增为 0、修订不变。
turn_usage 有 24 个轮级对账差异，仅作对照，不能与 model_usage 相加。
会话文件目前存在不能证明产品永远不清理；清空应用统计前的备份仍有必要。

依据：[Codex App Server](https://learn.chatgpt.com/docs/app-server)、
[OpenCode Session 源码](https://github.com/anomalyco/opencode/blob/dev/packages/opencode/src/session/session.ts)、
[Qwen Code 会话归档合同](https://github.com/QwenLM/qwen-code/issues/6057)、
[Qwen Code 活动/归档副本问题](https://github.com/QwenLM/qwen-code/issues/9688)、
[Qwen Code 环境路径](https://github.com/QwenLM/qwen-code/blob/main/docs/users/configuration/settings.md)、
[Gemini CLI 会话管理](https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/session-management.md)；
本机安装包指纹、只读 SQL、脱敏结果及测试入口见[审查记录](../../validation/desktop-usage/review-2026-09-27.md)。

## 暂未证实工具的推进方式

M4 保留新版 Kimi Code、Kimi Work、ZCode、WorkBuddy；JetBrains/TRAE、Zed 内置及
表中本地用量格式尚未核验的 IDE 变体列入 F1，首版不探测/实施。2026-09-29 第二批调研后：
Junie CLI 与 Zed 内置（threads.db）已取得本地载体依据，从 F1 转入 M8；
Cursor、Windsurf、京东 JoyCode、智谱 CodeGeeX 插件、百度文心快码 Comate、
华为 InsCode/CodeArts Snap 本地用量格式仍待核验，保留在 F1；Warp 仅账户级额度缓存、
Cursor/TRAE 的逐次用量在远端，均按本地边界排除且不以估算补齐。
各产品到其所属阶段后，按
“识别版本→本地存储/本机导出/本地遥测→最小脱敏样本→字段/生命周期测试”推进。
不能因同属 IDE 后移有明确本地字段依据的 Cline、Roo Code 或已文档化的 VS Code 遥测。
每个工具留下核验版本、已查的官方本地入口/候选格式、授权范围、读取结果、缺失字段、
失败原因和后续条件；没有本机版本/样本时记“未尝试/待样本”，不能写“已验证不支持”。
只有套餐/账号额度页时显示本地用量暂不可提取；只有文本无可靠 usage 时不默认使用 tokenizer 估算。

不通过 TLS 中间人、浏览器会话 cookie、进程注入或私有账单接口补齐缺口。
用户未来主动要求自定义 API 网关计量时另做设计；代理不能读取此前历史，
也不一定覆盖官方订阅通道，因此不是当前通用回退方案。

首批之后的准入合同评估与 M8 文档级实施均已完成（2026-09-29，[M8 验证记录]
（../../validation/desktop-usage/m8-second-batch.md)）：18 个适配器注册
（17 个解析 + Qoder 探针）；Amazon Q/Codebuff 经源码核验确认本地无逐次 token 载体、
iFlow 已停服（2026-04-17）且唯一载体是需启用的 OTel（归 M5），三者不实施
token 适配器；Cursor/Windsurf 维持 F1。全部文档级实现的真实验收后置，
不把文档级实施写成已验证支持。
