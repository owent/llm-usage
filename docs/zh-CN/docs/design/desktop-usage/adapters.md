# Agent 接入调研与能力矩阵

<a id="agent-research-and-capability-matrix"></a>

初始调研日期：2026-09-24；2026-09-27 复核归档行为，见文末及
[本轮验证](../../validation/desktop-usage/review-2026-09-27.md)。
国内外流行 Agent 覆盖扩展调研（A25–A48）与 M8 文档级
实施均已完成（[M8 验证记录](../../validation/desktop-usage/m8-second-batch.md)）：
18 个适配器（17 个解析 + Qoder 探针）已注册；Amazon Q/Codebuff/iFlow 经源码核验
证实本地无逐次 token 记录（或已停服），按边界排除。
下表区分实施候选与核验状态。
初始实施阶段只读核对本机数据；此后真实请求及容器验收按各专项授权执行。
用户列出的 Codex 重复项合并，CLI/桌面/IDE 仍须分别标识产品表面。
Harness Agent 按用户提供官网更正为 Hermes Agent。所有工具均保留本地支持计划；
缺少本地格式依据的 IDE 保留 F1 标签；2026-10-07 已按授权核查候选本机安装，
没有可测试安装/记录的项移出本轮。Junie CLI 与 Zed 内置已转 M8；
Zed 外部 Provider 实样见 [本轮记录](../../validation/desktop-usage/plan-20261007.md)。
实施阶段可提取本机真实 Agent 数据验证，流程见 [开工准备](implementation-readiness.md)。
不以企业 API、账号报表或远程日志替代本机来源。

<a id="interpreting-support"></a>

## 如何解释支持

- **本地候选**：官方资料或源码证实存在可用存储/字段，仍需版本绑定的脱敏测试文件。
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

0.3.0 补充八个逐一核对的 Codex 原生版本，精确注册表现有 26 个旧格式和七个新格式版本。
下表原有的 21 个旧格式映射仍是历史依据，详见[新增样本与恢复](../../validation/desktop-usage/release-030.md)。
其他版本继续进行兼容检查，不继承这些核验结论。

<a id="user-specified-tools"></a>

## 用户指定工具

来源编号对应 [官方与源码索引](research.md#agents)。表中的字段为已观察到的上游能力或原型候选映射。

| 工具/变体 | 拟接入来源与可用信息 | 边界与下一步 | 阶段/依据 |
| --- | --- | --- | --- |
| Claude Code | projects 原生 JSONL：2.1.197 国内镜像/官方 integrity 核对，Debian/Podman 使用智谱两模型的两次真实主循环，API/CLI/原生正输入/输出相符；[真实记录](../../validation/desktop-usage/claude-container-sample.md) | 四内容块按 message.id 去重为两调用；逐行 version 绑定，默认零缓存/完整总量未知；记录未保存渠道，provider/费用不推断。旧完整摘要与未变化处理位置自动纠正。Anthropic 自家模型、正缓存、辅助/子 Agent/重试与其他版本仍另验；OTel 不叠加 | M2/M5 已实施，2.1.197 兼容端点实样通过，A01 |
| Cline | 旧 UI 文档级适配器 + 独立 VS Code SDK schema 1；4.1.22 官方 VSIX/真实 GUI 与本地 API 三条原生 metrics 已核对，成品复验见 [记录](../../validation/desktop-usage/cline-container-sample.md) | SDK inputTokens 含缓存、正桶 reported/默认零未知、消息自身 modelInfo/ts；metrics 可合并 run/重试，记 observation，调用数未知；origin.version 可重写，仍 latest_fallback；只读原生 messages，不叠加 manifest/DB。旧 UI 四桶/删除/子 Agent/compaction 规则独立保留，尚无真实样本；CLI/desktop SDK 面、迁移、其他 provider 待验，空 registry DB 不能确认用量格式 | M3 已实施；SDK 实样通过、其他面另验，A03 |
| CodeBuddy Code / IDE / 插件 | **扩展存储适配器已实施（本机真实核对 PASS，2026-09-30）**：`%LOCALAPPDATA%\CodeBuddyExtension\Data\...\history\<session>\<conversation>\index.json` 的 `requests[].usage` 请求级聚合（`inputTokens=cacheTokens+cachedMissTokens`），消息 `extra.modelId` 归属模型，见 [M4/M5 记录](../../validation/desktop-usage/m4-buddy-local.md)。CLI 记录：官方目录文档确认 `~/.codebuddy/projects` 会话 JSONL；第三方解析器与测试证实 `message.usage` / `providerData.rawUsage` 可读逐次 token，`input_tokens` 已含缓存读。官方 monitoring 文档另提供需启用的 OTLP/HTTP protobuf `model_stream` | 扩展 usage 为请求级聚合（非逐 LLM 调用）；全零请求无模型调用不入账；`credit` 积分与 `lastTokens` 语义尚未核验不映射；同请求跨 profile/workspace 树复制按请求 ID 键去重，不重复计数；CLI JSONL 本机仍无真实样本（文档级）；OTLP 与本地记录同时启用可能重复计数，应择一使用 | 扩展存储 M5 已实施（真实核对）；CLI 本地文档级适配器已注册，真实样本待完成，A04 |
| Codex CLI / 桌面 / IDE | 原型读 CODEX_HOME 下 rollout 的 token_usage_record；官方 OTel 支持请求、响应完成 usage 和流事件 | 已验证版本（逐版本脱敏测试文件）：0.155.0-alpha.16.3（M0/M2-A）、0.154.0-alpha.6.1/6.2、0.153.0（M2-D），逐次/累计/turn_context 三类记录，compaction 重置累计快照；0.139–0.151 已有 rollout_legacy 专用实现及 21 个精确版本映射，按 token_count 增量记录处理，边界见 m2d 记录；未知新版本默认 latest_fallback 带兼容标记；原型固定默认路径需改为发现/配置；旧 token_count 与新记录必须分格式；模型从结构化上下文归属，缺 response ID 不碰撞 | M2/M5；本地候选/需启用，A02 + 原型 |
| GitHub Copilot CLI | **适配器已实现（真实数据核对 PASS，2026-09-29）**：session-store.db assistant_usage_events（schema_version=8，36 行真实测试文件+期望值）；OTel 路径：`COPILOT_OTEL_FILE_EXPORTER_PATH`（JSON-lines，行级 schema 未文档化）/OTLP（默认 http/json；chat span gen_ai.usage.* 含缓存细分、TTFT；invoke_agent 汇总 span 官方警告不得求和双计）。**2026-09-30 复核：最新 CLI 的 `~/.copilot/session-store.db` 已改为 chronicle/search 索引（sessions/turns/checkpoints/search_index，无 assistant_usage_events），逐次用量记录外移；discover 增补 `COPILOT_HOME` 覆盖，detect 命中 chronicle 指纹记 UnsupportedVersion（不误报非 Copilot 库）。VS Code 扩展面 globalStorage 的同族 chronicle 库亦无 usage（turns 为纯文本）——VS Code 面用量经 copilot-chat 适配器（A06 行）接入，与 CLI 记录互不重复** | assistant_usage_events 为已验证主要记录格式（旧版）；**最新版本已移除该表：`session-state/<id>/events.jsonl` 事件日志经同族 copilot-agent 转录核验无逐次 token 字段，`copilot-user-cache.json` 仅账户额度（premium_interactions）非逐次——受查最新 CLI 本地无已验证逐次用量，待真实最新 CLI 样本或启用 OTel 后锚定**；OTel file exporter 行级 schema 待本机样本（otel 适配器同族容错）；premium 倍率/nano AIU 不入 token；2026-09-25 消失为升级迁移、已恢复 | 主要记录格式 M5 已实施（真实核对，旧版 schema=8）；最新版布局变更已识别（fail-closed），真实样本/OTel 后置；OTel 记录 M5 已实施（文档级），A05 |
| JetBrains 内置 AI Assistant / Junie | IDE 插件本地 schema 尚未核验，保留在 F1；Junie CLI 已取得本地格式依据，转入 M8：`~/.junie/sessions/<id>/events.jsonl` 逐轮 modelUsage（input/output/cache/reasoning/cost/time/provider），timestampMs 为结束时刻 | A07 只证实企业远端分析，已排除；Junie CLI（A36）不再列为待核验；AI Assistant IDE 插件本地 token/cache schema 待核验，当前不探测/开发 | IDE 插件 F1；Junie CLI M8，A07/A36 |
| DeepSeek Harness（DSH） | 官方 npm 0.2.0-rc.2 两轮本地 CLI/续会话、原生 v4 JSONL/zstd 与 API 已核对，Windows/Linux 新包回读通过 | 正非缓存输入；仅本行 pi-ai openai-completions/version 2 正输入反变换含缓存总输入；output 已含推理，自算 total 不作源总量；settlement/stream 择一、retry 新身份、继承前缀排除；时间用原生行时间并标观察依据；标题 205 token 未写入文件不补造，旧 doc1 独立保留 | M3 当前主循环完成；其他协议/真实 seed/retry/fail 另验，[规则](m3-runtime-samples.md)，A08 |
| Hermes Agent（原需求 Harness Agent） | M3 已实现；0.21.5/v2026.9.24 官方镜像的真实本地请求/续会话已核对，schema 30 不能核验其他历史版本 | session_model_usage 为非缓存输入/缓存读写/输出/推理区间累计；默认零未知，api_call_count 不拆调用；完整旧摘要/旧处理位置重评；模型/费用维度与日志详单另验 | M3 本地样本已核对，A24 |
| OpenClaw | schema 24 只读读取已实现（M3，2026-10-06；官方 npm 2026.9.8 CLI/续会话真实本地模型样本） | transcript_events TEXT/zstd 正文；本地已核验 provenance，外部/迁移输入隔离；input 为非缓存，默认零、计算总量/费用及底层调用未知，整库版本不能核验历史行；冷归档显式覆盖缺口，其他协议/schema 不套用 | M3 真实 CLI 专项 11 项、原库升级与 Windows 成品双读通过，A09；[读取规则](openclaw-runtime.md)、[真实验收](../../validation/desktop-usage/openclaw-container-sample.md) |
| Gemini CLI | 官方会话存储 ~/.gemini/tmp/<project_hash>/chats/ 含 token；OTel 提供 input/output/thought/cache/tool 和请求统计 | JSON/JSONL 具体版本探测；thought/cache/tool 字段的包含关系逐 provider 核验；Gemini 网页和 Code Assist 不冒充 CLI | M2/M5；本地候选/需启用，A10 |
| Kilo Code CLI / 桌面 / IDE 扩展 | 适配器已实现（M3，2026-09-25）：SQLite 只读 + 暂存副本规则；message.data.tokens 逐次五互斥；按处理位置增量；session 行 tokens 列仅作会话对账（275/276 matched，列级语义随版本不稳定不映射事件）；真实核对 13,342 调用/1.65B token 与独立求和一致，重复读取不新增用量 | 已验证 7.4.8/7.4.9/7.8.1（测试文件）；库内其余 7.3.42–7.7.12 等未收录走 latest_fallback；7.8.1 为本机实读脱敏（顶层 modelID/providerID 记录，与 7.4.x 同形共用 message_tokens_v1）；新 core 数据层（session_message，当前 0 行）切换后需专用实现核验；IDE 扩展后移 F1；不能拿 CLI 通过代表 IDE 通过 | CLI/桌面 M3 已实施；本地用量格式尚未核验的 IDE 变体 F1，A11 |
| 新版 Kimi Code | 适配器已实现（M4，2026-09-25）：家族共享 kimi_wire.rs；注册表参考版本 protocol_version=1.5；真实核对 13 文件 965 调用 total 116,428,813 与 jq 逐字段相等，重复读取不新增用量；打断步回声缺失以 echo_subset 可见 | usage.record 无稳定 ID ⇒ provider/逐次延迟不入账（如实 unavailable）；旧会话 1.4 走 latest_fallback；其他版本仍需各自 fixture | M4 已实施；跨版本待核验，A12 |
| Kimi Work | 适配器已实现（M4，2026-09-25）：独立目录（A13 不与 Kimi Code 合并）；conv-*/ctitle-* 布局（daimon 宿主）；参考版本 1.4；真实核对 69 文件 1,337 调用 total 125,225,933 逐字段相等、69/69 对账 matched；跨文件同毫秒（swarm）事件键含 session:agent 身份段 | 官方默认布局/env 未见文档（迁移需手工加根）；subagent.completed 与缓存写>0 无本机真实样本；与 Kimi Code 共享 wire 解析但统计分列 | M4 已实施，A13 |
| MiMo Code | 官方 0.1.15 CLI/续会话八条 step-finish 与 API 已逐条核对，Windows/Linux 原始 SQLite/WAL 新包升级通过 | 自身 SDK 归一 input/output 与缓存/推理反变换；零缓存/推理/费用未知，不叠加 message 副本；所属会话版本逐记录 latest_fallback；MIMOCODE_HOME/data 与 MIMOCODE_DB 优先级独立核验；八次 length 不能核验任务成功 | M3 当前主循环完成；其他版本/协议/非截断任务另验，[规则](m3-runtime-samples.md)，A14 |
| oh-my-pi | 官方 session 文档确认 ~/.omp/agent/sessions/...jsonl，assistant usage 与模型/provider | 本机 18.2.7 已取 3 个脱敏测试文件（2026-09-25，[M2-B/C 恢复记录](../../validation/desktop-usage/m2bc-resumed.md)）：assistant 全带 usage + duration/ttft 浮点毫秒；model_change 写入文件为 `model`="provider/model" 组合字段（非 pi 分字段）；子 Agent 文件在 `<ts>_<父UUID>/` 目录按路径形状归父；~/.omp/logs 仅上下文估算 debug 行、无逐次用量，title-generator 尚无重叠依据；compaction/branch_summary 本机均无 usage（辅助记录规则已实现，待真实样本） | M2；本地候选，A15 |
| pi | 固定源码 Usage 含 input/output/cacheRead/cacheWrite/reasoning；session 有 message、独立 usage、compaction/branch summary usage | 推理已含于 output；仅 assistant message 会漏辅助用量；分支继承不代表新调用；目录由该版本配置函数解析；本机 0.87.1 已取真实测试文件 并通过重复读取核对（2026-09-25，[M2-B/C 恢复记录](../../validation/desktop-usage/m2bc-resumed.md)）；fork 复制条目带 fork 会话身份，冲突处理为 conflict 保留先扫者，净效果不双计 | M2；本地候选，A16 |
| OpenCode | 适配器已实现；1.18.34（固定源码 aec0b9a6）在独立 Podman 真实主循环及缓存读与 API/CLI/原生库/应用一致，重扫不新增用量；1.18.34 已逐记录核验，混合版本与旧处理位置升级回归通过 | part 的 input 为未缓存桶；session.tokens_* 仅对账、message 聚合不叠加；默认标题另有 549 token API 调用，未进入主循环 part/累计，matched 不证明全覆盖；明确标题对照 1 次/299 token。一条真实会话不能确认同库其他版本、空会话或旧游标的格式 | M3 主循环真实兼容验收；其他版本/云端/未写入文件标题记录另验；[容器记录](../../validation/desktop-usage/container-sources.md)，A17 |
| Qwen Code | recording service 记录 usageMetadata/model；0.25.0 独立 Podman 真实主循环与 CLI/模型服务/SQLite 一致、重扫不新增用量；真实 file 导出已核对主循环与后台两条 API 日志/LLM span，含后台缓存读 | 默认自动记忆调用未进入会话逐次记录，不从累计量补造事件；file 为连续多行 SDK JSON，须使用独立读取器，不能作为通用 OTel JSONL；2026-10-06 已实施按已核验主机/用户/会话/本地日选择 SDK、保留原生历史并排除封存重叠；Goal 不叠加，活动/归档按 uuid 去重；未知本地模型名不套价；云端、其他版本/主循环缓存命中待验 | M2 主循环真实验收，M5 0.25.0 SDK 读取与范围选择已实施；[容器记录](../../validation/desktop-usage/container-sources.md)、[数据规则](data-contract.md)，A18 |
| Zoo Code | 官方 VSIX 3.86.0 在独立 VS Code GUI/公开 API 调用本地模型，原生数组/回调/API 与 Windows/Linux 新包回读一致 | 完整 ask/say 枚举；正 tokensIn 含缓存，默认零桶/费用未知；缓存未齐不派生未缓存；模型/客户端版本不从全局配置补造；旧 finished LIFO/condense 文档路径保留；真实任务主动取消，不声明完成 | M3 当前扩展主调用完成；CLI/删除/压缩/其他版本另验，JetBrains 变体 F1，[规则](m3-runtime-samples.md)，A19 |
| TRAE / TraeCode 及插件 | 后续探测本地 IDE/CLI 日志、会话库、插件数据和本机会话导出 | A20 企业 API/控制台报表不纳入；2026-09-29 复核：tokscale 的 TRAE 路线源自其远端 usage API 写入文件缓存（dollar_float/extra_info 形态），同样不符合本机来源边界；本地逐次格式仍缺少可核验的资料，整个产品家族继续后移 | F1；后续支持，A20 为范围排除依据 |
| VS Code | 原生 chatSessions v3：promptTokens 为末次调用输入下界、completionTokens 为整轮累计；modelTotals 提供整轮逐模型总量；toolCallRounds 带 ID/时间戳的主循环轮次计调用；仅接入请求 agent.id 为 github.copilot 命名空间的记录。workspaceStorage 与 globalStorage/emptyWindowChatSessions 共用安装来源；本机 10 turn/217 round，输入下界 3,408,279、输出 320,141，见 [审查记录](../../validation/desktop-usage/m9-copilot-review.md)。2026-10-02：一键配置 file 导出本机 30 个 CLIENT chat span 已验收，见 [补充记录](../../validation/desktop-usage/dashboard-repair.md) | copilot-chat 全量重放完整快照；turn/逐模型汇总计 usage_observation，round 计 model_call，逐轮 token 未知；modelTotals 替换旧贡献、模型换序/副本/流式更新不重复累计。末次输入和整轮输出不派生完整总 token；缓存默认未知。应用管理的 OTel file 自动发现，同原主机/用户/会话/本地日择 OTel，保留原生历史及开启前覆盖缺口；trace+span 同源副本去重 | M9 原生和 VS Code file 均有本机真实验收；CLI/JetBrains OTel 仍文档级需独立验收，A06 |
| GitHub Copilot for Visual Studio | **已核验 VS 18 自动遥测**：`Path.GetTempPath()/VSGitHubCopilotLogs/traces/*.jsonl`，OTLP JSON、service.name=vs-copilot；chat CLIENT span 逐请求直报 input/output/cache_read、模型/会话/纳秒时间。发现同时检查 TMP/TEMP/用户默认临时目录，不按 Community/Professional/Enterprise 或年份过滤；安装核查使用官方 vswhere，不拼接安装根。VS 2022 受查 17.14.1713.63837 组件没有 VS 18 的 JSONL exporter；共享 SDK 或安装版本不能确认旧扩展及其他记录格式，见 [跨版本核查](../../validation/desktop-usage/m9-vs-copilot-discovery.md)。会话 MessagePack、模型上下文上限及额度不代替 token | vs-copilot 适配器：invoke_agent 汇总跳过防双计；临时记录清理/滚动不承诺历史；与同源 otel 导出择一；桶并列报告不派生 uncached；仅选择指定属性以保护正文，保留未知和旧游标；[本机实样](../../validation/desktop-usage/m9-vs-copilot-local.md)及本轮 4 调用独立核对/重扫通过 | M9 已实施；真实用量限受测 VS 18 Community 记录，其他 SKU/VS 2022 保留静态与原生边界，A49 |
| GitHub Copilot for JetBrains | **源码核验（2026-10-01 插件 1.18.0-261 解包，[M9 分析记录](../../validation/desktop-usage/m9-jb-copilot-analysis.md)）**：默认本地数据无逐次 token——会话库为嵌入式 Nitrite `copilot-agent-sessions-nitrite.db`（NtAgentSession：turns/模型/**turnCreditsJson=RestoredTurnCredits(messageId,credits)**，credit 非 token 不折算）、日志走 idea.log、App Insights 为远程（排除）；**逐次 token 记录=需启用的 OTel 导出**：插件设置 `otelEnabled`/`otelExporterType(file/otlp-http/otlp-grpc/console)`/`otelOutfile`/`otelServiceName`/`otelCaptureContent`（=官方文档 Agent debug File Logging），与 VS Code `github.copilot.chat.otel.*` 同族同词汇；插件自身 Debug Panel 即从 outfile 解析 `gen_ai.usage.{input,output,cache_read,cache_creation}*` 五桶（OTelSpanProvider 字节码核验依据） | 不新增默认记录适配器（默认仅 credit，违背逐次 token 诉求）；接入路径=既有 otel 适配器（M5）：用户在 Settings → Tools → Copilot → Chat 启用 file 导出后把 otelOutfile 加为手工根；行格式同族容错、本机无 JetBrains 待真实样本锚定；otelCaptureContent 含提示正文——本应用只读取指定属性 | 经 otel 适配器接入（需启用导出的记录，文档级），A06 同族扩展 |
| ZCode | 适配器已实现（M4，2026-09-25）：model-io JSONL 两种字段规则——AI SDK 五键为主（inputTokens 含缓存读）、anthropic snake_case 对照互斥校验（矛盾进诊断）；db.sqlite model_usage 为自动采集主要记录格式，turn_usage 只读对账（活库不一致轮可见）；真实核对重复读取不新增用量 | 已验证 3.14.3（真实测试文件）；未收录版本 latest_fallback；`~/.zcode/v2` 布局与 `%APPDATA%/zcode` 桌面存储未接入（待核验）；缺 requestId/traceId 时不得全部变成 zcode:None | M4 已实施；跨版本待核验，A21 |
| WorkBuddy | 第三方开源解析器证实 `~/.workbuddy/projects/**/*.jsonl` 会话逐次用量字段；本机 8 文件/420 事件真实只读核对、重扫不新增用量通过（[记录](../../validation/desktop-usage/m4-buddy-local.md)） | 独立来源，未套用 CodeBuddy CLI OTLP；`traces/` 的总量与会话明细重叠关系未核，暂不叠加；套餐/账号积分页不纳入 | M4 已实施并核对当前本机格式；跨版本/trace 待核验，A22 |
| Zed | threads.db 的 json/zstd DbThread；cumulative_token_usage 为线程累计，request_token_usage 是 turn 末次请求桶，仅作对账 | 1.22.0 / DbThread 0.3.0 的 llm-usage-zhipu 两模型已真实请求：input 为非缓存桶，正桶报告、默认零未知，不派生总量或底层调用/模型明细；原 hosted 映射独立，imported 跳过，ACP 按底层来源读取 | M8 新增本机非空实样与成品回读；旧 hosted 仍仅源码/schema 依据；[记录](../../validation/desktop-usage/plan-20261007.md)，A23/A38 |

本轮没有将任何“未发现文档”写成“该产品不可能支持”。
若不能取得可靠用量，仍可展示该工具状态与限制，但不占据有数值的总计行。

<a id="扩展覆盖"></a>

<a id="expanded-coverage"></a>

## 扩展覆盖

<a id="扩展覆盖2026-09-29-第二批调研"></a>

下列产品为本轮补全的国内外流行 Agent，来源编号见 [调研索引](research.md#agents)
（A25–A48）。已完成 M8 文档级实施（[验证记录](../../validation/desktop-usage/m8-second-batch.md)）：
各适配器独立目录 + 版本注册表 + V30 结构检查 + 合成测试文件 规则测试
（tests/m8_contract.rs 19 项）；后续真实容器样本和产品专项见
[M8 容器样本](../../validation/desktop-usage/m8-container-samples.md)，其余产品仍待核验。
闭源产品的字段依据来自第三方解析器或逆向分析，真实脱敏测试文件（许可已给）
出现后升级验证；一切估算路径（Kiro Auto 补零、Grok 累计差额、Goose reasoning
差额等）不采纳，仅采信原生计数。

| 工具/变体 | 拟接入来源与可用信息 | 边界与下一步 | 阶段/依据 |
| --- | --- | --- | --- |
| Amp（Sourcegraph，闭源） | `~/.local/share/amp/threads/T-*.json`：messages[].usage（model、inputTokens/outputTokens、cacheRead/cacheCreation、credits）与 usageLedger.events（timestamp/model/credits/tokens 五桶）两种记录格式 | ledger 与逐消息 usage 按 messageId+桶对账防双计；缺显式时间戳不得以 thread created+messageId 推造逐次时间；credits 是计费单位非美元；schema 随版本滚动需测试文件 | M8 已实施（文档级 2026-09-29；ledger 为主、消息 usage 仅对账不推造时间；credits 不映射），A28 |
| Goose（Block；仓库已迁 aaif-goose） | `sessions.db` 的 usage_ledger 逐请求五桶；旧 sessions accumulated_* 聚合兜底；GOOSE_PATH_ROOT 与平台多根 | 两种记录格式互斥；NULL 不用累计默认 0 覆盖；reasoning 差额不采；1.53.0 CLI 本地模型 API/库/应用真实一致 | M8 已实施；estimated/carried_forward cost 不映射，格式参考版本不能核验产品版本；更多场景见 [真实样本](../../validation/desktop-usage/m8-container-samples.md)，A26 |
| Crush（Charm） | `~/.local/share/crush/projects.json` 注册表映射每项目 data_dir 与 crush.db；根会话 cost 累计，token 列是上下文快照 | 0.97.1 真实主循环/标题两次 API 与原生/CLI Estimated 成本一致；人工验收费率不能核验模型价格/账单；token/调用/模型仍未知 | M8 cost-only 已实施；根会话过滤避免子成本重复，真实样本及成品状态见 [M8 容器样本](../../validation/desktop-usage/m8-container-samples.md)，A27 |
| Roo Code | VS Code globalStorage `rooveterinaryinc.roo-cline/tasks/<uuid>/ui_messages.json`；3.54.0 官方 VSIX/真实 VS Code extension-host 与本地 API 已核对 | 正 tokensIn 含缓存，四桶/估价默认零未知；OpenAI-compatible 缓存详情丢失，不按零推导未缓存；取消场景 API 三次/原生两次缺口保留，单调用对照单列；无逐次模型不推断 | M8 已实施；完整旧摘要/未变游标修正范围见数据规则，成品状态见 [M8 容器样本](../../validation/desktop-usage/m8-container-samples.md)；CLI/其他 provider/子代理另证，A29 |
| Aider | 默认历史无逐次 usage；显式 `--analytics-log` 本地 JSONL 的 message_send 含 token/cost | 需启用、不回填；0.86.2 本地模型真实 API/CLI/应用一致；无缓存/推理分项，不走 tokenizer 估算 | M8 已实施；cost=litellm 自算 Estimated；手工根；格式参考版本不能核验产品版本，见 [真实样本](../../validation/desktop-usage/m8-container-samples.md)，A30 |
| Continue（CLI/VS Code/JetBrains） | `~/.continue/sessions` 的 CLI 顶层 usage 会话累计；索引/估算日志不采 | 1.5.47 真实流式本地模型输入/输出与 API 一致；缓存默认零保持未知；无调用数、模型/完整总量或区间起点，不伪造日归属 | M8 已实施；旧完整摘要仅纠正缓存默认零，保留其他冲突处理/历史；GUI 无用量字段，见 [真实样本](../../validation/desktop-usage/m8-container-samples.md)，A31 |
| Droid（Factory.ai，闭源） | `~/.factory/sessions/{uuid}.settings.json` tokenUsage（input/output/cacheRead/cacheCreation/thinking，累计）+ 同名 jsonl 转录 | 累计值保留区间语义，分摊到回合属估计不采纳为逐次；无费用字段；providerLock 与转录时间归属待样本 | M8 已实施（tokenUsage 累计快照→会话聚合；转录字节分摊不采），A32 |
| Amazon Q Developer CLI | `~/.aws/amazonq/history/` 按时间戳 JSON 会话（第三方资料）；官方仓库开源 | history 内 usage/token 字段未核验，先源码级核验再实现；SSO 重登录可能丢历史，保留缺口可见 | 不实施 token 适配器（2026-09-29 源码核验：API tokenUsage 在类型转换层被丢弃，conversations/history 均无 token 字段），A33 |
| Grok Build（xAI，闭源） | `~/.grok/sessions/<workspace>/<session>/`（updates.jsonl、signals.json、summary.json、events.jsonl）与 `~/.grok/logs/unified.jsonl` | 仅取显式 usage 块五桶；累计 totalTokens 增量与压缩差额补偿是推断不采纳；PID 复用/子代理模型归属复杂，归属依据冲突时保持 unknown；GROK_HOME 覆盖 | M8 已实施（仅 updates.jsonl 显式 usage 块；累计差额/补偿/PID 归因不采），A34 |
| Antigravity CLI/扩展（Google） | `~/.gemini/antigravity[-cli]/conversations/<uuid>.db`：gen_metadata protobuf 逐回合 usage（input=固定系统提示+新增、cacheRead、output、thinking、responseId） | protobuf 布局为逆向结论且 1.1.18 时间戳字段变更，须逐版本锚定；IDE 主体用量走 language server，另按需启用核验；与 Gemini CLI 目录同根不同子目录 | M8 已实施（逆向 protobuf：input=#1+#2、#9.#4 时间戳；1.1.18+ 无时间戳行 fail closed；IDE language server 记录格式尚未核验不实施），A35 |
| Junie CLI（JetBrains） | `JUNIE_HOME/sessions` 或 `~/.junie/sessions/<session-id>/events.jsonl`，逐次 modelUsage；26.9.22 官方发行包/七次真实失败任务与 API/CLI 已核对 | inputTokens 为非缓存输入，默认零/费用/耗时保持未知；正费用 Estimated；无 API/provider/产品版本，完整总量不推造；timestampMs 完成时刻，正 time 才补起点 | M8 已实施；doc1→2 完整旧摘要/未变游标修正、事务/冲突保留通过，成品状态见 [M8 容器样本](../../validation/desktop-usage/m8-container-samples.md)，A36 |
| Kiro（AWS，CLI+IDE） | 三种记录格式：CLI `~/.kiro/sessions/cli/*.json(+jsonl)`；kiro-cli `~/.local/share/kiro-cli/data.sqlite3` conversations_v2；IDE globalStorage `kiro.kiroagent`（.chat 快照、execution、promptLogs） | Auto agent 常记 0：估算路径不采纳，仅采显式计数；execution 与 .chat 快照按 executionId 抑制重复；metering credit 独立计价单位；三种记录格式交叉去重 | M8 已实施（CLI turns 真实计数 + kiro-cli SQLite request_metadata；Auto 零计数/估算不采；IDE 估算记录不实施；两种记录格式重叠待真实样本对账），A37 |
| Zed 内置（升级自 F1） | threads.db 的线程累计四桶；turn 桶不能核验逐次完整请求 | zed.dev 沿用原源码规则；新增 llm-usage-zhipu + DbThread 0.3.0 已核验正值/默认零与缓存语义；imported/其他 Provider 不能核验，有界解压 | M8：1.22.0 / 76659a55、glm-5.3 与 glm-5.3-flash 三 turn 非空实样；累计 21,696 非缓存输入/10,816 缓存读/61 输出，写缓存及总量未知；[记录](../../validation/desktop-usage/plan-20261007.md)，A23/A38 |
| Codebuff（原 Manicode） | `~/.config/manicode*/projects/*/chats/<chatId>/chat-messages.json`；CODEBUFF_DATA_DIR 覆盖 | usage 字段与分通道根（manicode/-dev/-staging）待测试文件 | 不实施 token 适配器（2026-09-29 官方源码 caec5fc 证实本地仅 credits、无 token 字段；CODEBUFF_DATA_DIR 非官方变量），A39 |
| Command Code | `~/.commandcode/projects/<slug>/*.jsonl` v3 树形（session/message/model_change；assistant usage 五桶+costUsd）；配置 config.json | rewind 孤儿分支不计；fork 复制按 id+时间戳去重；checkpoints 文件跳过；全零 usage 视为已报告零 | M8 已实施（npm 1.69.0 分发物核验依据：inputTokens 含 cache、当前目录规则、fork 按 id+时间戳跨文件去重、全零=已报告零），A40 |
| jcode（jcode.sh，开源 Rust） | `~/.jcode/sessions/session_*.json` 快照 + `.journal.jsonl` 追加（journal 值优先替换快照）；token_usage 输入/输出与可选缓存 | OpenAI/Anthropic 缓存包含关系差异逐版本归一；JCODE_HOME 覆盖；环境快照版本不能核验所有消息 | M8 已实施；0.91.0 真实离线单次 run 的 API/CLI/快照输入 460、输出 2、缓存读 0 一致；自定义 provider 未推断缓存关系、完整总量/成本/推理未知，journal/其他场景待验，见 [真实样本](../../validation/desktop-usage/m8-container-samples.md)，A41 |
| gajae-code（gjc） | `~/.gjc/agent/sessions/<slug>/*.jsonl`：session v5 头、assistant model/provider/usage 与 Estimated 成本 | GJC_CODING_AGENT_DIR 等覆盖；OpenAI-completions 缺字段零回退不能核验缓存/零用量；message.timestamp 为请求开始 | M8 已实施；0.18.7 真实 API/CLI/记录 412/2/414 一致，补配置链非用量类型、未知缓存/未缓存与旧完整摘要/游标升级；其他 API/版本/分支待验，见 [真实样本](../../validation/desktop-usage/m8-container-samples.md)，A42 |
| Xum（Coder；原 mux） | `.xum/.mux/sessions`、XUM_ROOT/MUX_ROOT 或 RUN_SESSION_ROOT 下的 `session-usage.json` v1；0.30.0 官方源码与两次真实本地调用已核对 | input 为未缓存输入；默认零未知；正文本输出加已知推理，推理未知时为下界；完整输入/总量未知。默认 CLI 临时会话删除且 custom provider 未请求流式 usage，网关对照单独记录 | M8 已实施；成本/逐次调用不虚构；完整旧摘要/游标与回滚/冲突专项通过，成品状态见 [M8 容器样本](../../validation/desktop-usage/m8-container-samples.md)，A43 |
| iFlow CLI（阿里心流，闭源） | 官方固定提交 4642808 已公告 2026-04-17 停服；本地默认逐次用量记录未核验，需启用的 OTel 按 M5 规则另行核验 | 不套用 Gemini 家族格式；不作为现行 M8 本地候选 | 不实施；与 AGENTS/M8 边界一致，A46 |
| Qoder CLI（阿里，前通义灵码） | CLI 设备流 `~/.qoder/`；IDE 为 Electron `%APPDATA%\com.qoder.app.stable*`；另有 globalStorage/`~/.local/share/qoder` 线索 | CLI 会话格式与 usage 字段尚未核验：先本机核验再定实现；与灵码品牌演化记录在案；JetBrains 插件归 F1 | M8 探针级接入（路径已核验、字段尚未核验：只发现识别不解析，fail closed；待本机实测 `<session>.jsonl` 与 `state.json`），A47 |
| AtomCode（AtomGit 生态） | `.meta` turn_stats/model_usage 按模型会话聚合；round_count 来源报告调用汇总；旧单文件同构 | 5.2.1 真实 API/CLI/.meta 为 6,176 输入/2 输出；缓存默认零与坏桶保持未知，未缓存不猜测；有界排除完整配对的 UI v1/rewind v2，其他仍诊断 | M8 已实施；官方 e4215f7/45e05cb 与真实样本、旧完整摘要/游标及并行/回滚通过；产品版本不能核验其他记录，InsCode IDE 归 F1，见 [真实样本](../../validation/desktop-usage/m8-container-samples.md)，A48 |
| Warp | 本地仅账户级用量缓存（requestsUsed/spendCents/syncedAt，工作区级） | 无 token 明细且属账户额度数据：不入 token 统计，最多以状态行展示受限 | 暂缓（额度类），A44 |
| Cursor（CLI/IDE） | CLI 转录 `~/.cursor/projects/<slug>/agent-transcripts/<uuid>/*.jsonl`（有会话、无逐次 token/模型字段）；IDE state.vscdb 记录格式待核验 | 逐次用量仅存在于远端 dashboard（get-filtered-usage-events/CSV 导出），按本地边界排除；不以 tokenizer 从转录估算 | F1（本地用量格式待核验），A45 |
| Windsurf（IDE+CLI） | IDE Cascade 与 2026 年确认存在的 windsurf CLI；本地 usage 存储尚未核验 | 本地逐次 token 格式尚未核验时不探测/不实施；列入 F1 后续支持计划 | F1；后续支持 |

<a id="hermes-agent-本地合同"></a>

<a id="local-hermes-agent-rules"></a>

## Hermes Agent 本地规则

A24 固定源码 `ef70b3661cbfcf57e583008ad91dd04d8ba46070` 与官方 0.21.5 发布
`f97608f178d1ffeca59860195ab7da295f7c8e5f` 确认以下内容；当前真实样本限本地 CLI
主循环与一次续会话，其他版本/历史记录及 gateway 不扩大核验范围：

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
  实际日志文件、轮转、时区、稳定关联 ID、失败覆盖和缓存缺省语义尚待测试文件。
  两份固定源码的 normalize_usage 已完整核验，input_tokens 为非缓存、输出含 reasoning；
  默认零不能证明响应字段存在。正桶保留、零未知，仅全部必需桶已知才派生总量。
  整库 schema_version 不证明每行历史的客户端版本，仍保留 latest_fallback。
- 父子/压缩继承、辅助任务、MoA 聚合及外部 Codex 镜像必须验证覆盖集合；
  不读取 messages/FTS 正文计数来补请求数。源 DB 只读，不能调用其可能迁移/修复的初始化 API。

首个可交付能力可为本机按模型/任务的原生区间统计；逐次请求和精确日统计独立标注能力。
计费金额字段只保留本机调用归属明确的记录，不读取 Hermes Portal 账号余额或调用 account_usage API。

<a id="reuse-within-agent-families"></a>

## 分家族复用的范围

基础读取器可以复用 JSONL、可变 JSON、只读 SQLite、OTLP 和 CSV。
业务映射仅在字段与生命周期测试证明一致后复用：

- pi/oh-my-pi 可共享部分 Usage 类型知识，辅助事件及分支恢复仍各测；
  gajae-code（gjc）为 pi 血统变体，复用同样逐项验证。
- OpenCode/Kilo/MiMo 可共享结构探测工具，不共享未经验证的目录、表名或累计/增量假设。
  Kilo 的库内最高版本只用于格式探测；每条消息按所属 session.version 保留已验证/兼容
  依据。混合库存在未验证消息时保留兼容提示，后续只有已验证增量不能覆盖该状态；
  新版本空会话不能核验，也不降级已有消息的版本核验依据。支持依据/规则更新自动重评。
  OpenCode 同样按产生 step-finish 的所属会话选择依据；无用量的会话不参与版本核验。
  版本支持或解析规则变化时从头重评旧处理位置；分页以 `(time_updated, part.id)`
  续扫，完成窗口后恢复 60 秒重叠，不能因同毫秒或重叠窗口停滞。只有完整有效重评
  标记规则完成，坏记录与未验证记录跨增量保留；事件、续扫位置与规则进度同事务提交。
- Cline/Zoo/旧 Kilo 扩展的 API 记录可以共享解析组件，但子 Agent 汇总、删除与压缩记录逐项测试；
  Roo Code 扩展记录格式相同可复用组件，删除/子 Agent/压缩行为分版本复测。
- VS Code/Copilot/宿主嵌入的 Claude/Codex 不按应用品牌重复计费，使用 origin 归属关系。
- Kimi Work 和 Kimi Code 的数据根、实例身份和日志 revision 独立，重叠来源显式处理。
- Amp 的 usageLedger 与逐消息 usage、Kiro 的 execution 与 .chat 快照属于同源两种记录格式，
  对账去重规则各自验证，不能按时间接近或数值相同猜测同一请求。
- Goose/Xum/Droid 等会话级聚合来源按 Hermes 区间语义处理，不展开成伪造逐次事件。
- Antigravity CLI 与 Gemini CLI 同根（~/.gemini）不同子目录，目录发现互不推断；
  iFlow CLI 与 Gemini CLI 格式相似仅作线索，字段仍逐个核验。

<a id="每个适配器的交付合同"></a>

<a id="adapter-delivery-requirements"></a>

## 每个适配器的交付要求

代码布局遵守 [Agent 独立目录与版本组织](architecture.md#adapter-layout)。
每个 Agent 一个目录，历史版本实现在目录内分模块；该要求适用于本表全部阶段
（目录迁移与版本注册表已随 M2 实施，见 [架构要求](architecture.md#adapter-layout)）；
已验证支持与未知版本兼容尝试分别记录，不把尚待执行的结构调整记为已实现。

| 项目 | 必须交付的内容 |
| --- | --- |
| 目录与版本 | Agent 独立目录、统一入口、发布版本/来源格式到内部实现的映射、逐版本测试文件；新增版本保留历史实现及回归，按 V30 验收 |
| 发现 | 产品/表面/OS/版本范围、本机归属依据、候选路径、环境覆盖、profile、手工添加方法 |
| 探测 | 逐文件/数据库识别 Agent 与输入类型；已知版本按映射分派，未知版本先尝试该 Agent 最新内置解析器；结构/语义不兼容或匹配冲突时明确报错 |
| 字段映射 | 每个 token 字段包含关系、单位、时间基准、模型归属、调用 ID、调用确认依据 |
| 生命周期 | 单次/累计/流式/最终/更正/聚合、重试和分支/子 Agent/辅助调用的处理 |
| 增量 | 游标、重写/轮转检测、事务恢复、数据源自身保留和最早可回填日期 |
| 去重 | 本机副本、多个数据库、宿主镜像、本机遥测/会话导出之间的关联或主来源选择 |
| 完整性 | success-only、是否记录隐藏调用、采样/丢包/日志清理、无 timestamp 的限制 |
| 验证 | 原始版本说明、最小脱敏测试文件、人工期望值、单元/集成结果、真实应用核对结果；兼容尝试覆盖版本未收录但结构兼容、结构破坏及部分可用 |
| 维护 | 源码 commit/schema、最新默认解析器、选择依据、兼容状态、升级重试与不兼容诊断；不因未知版本号直接拒绝 |
| 定时 | 统一采集入口、增量/重扫开销、暂停/取消、无变化扫描和任务恢复；不启动 Agent |

每个样本仅保留事件类型、匿名 ID、时间、模型、数值与必要关系；正文替换为常量。
样本改变后重新计算摘要，记录脱敏方法；不得提交完整真实会话库。

<a id="required-samples-by-family"></a>

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
| Copilot/VS Code/CodeBuddy | chat/model_stream 与父 span 去重、file/OTLP 重传、cache 缺失、无 usage 的失败、SDK/宿主重叠、chatSessions 与 otel outfile 同面双计（同 agent 维度择一）、kv 采样快照与压缩重写（chatSessions 全量重放+同键更新不重复计数）、VS TEMP 遥测清理/实例滚动与 otel 接收器同维度择一 |
| JetBrains/TRAE/其他本地待核验源 | 本地日志/数据库真实样本、版本/插件差异、缺 usage、同区间重导入；远端同步/账号报表必须排除 |
| M8 两种记录格式/对账类（Amp/Kiro） | ledger 全量与部分覆盖、execution 覆盖快照、重复事件、缺 messageId、跨记录格式同一请求 |
| M8 会话级聚合类（Goose/Xum/Droid） | accumulated 与单次列、跨日会话、快照+日志合并（jcode）、provider 前缀模型 key、累计下修 |
| M8 逆向/闭源类（Antigravity/Grok/Junie/Qoder/iFlow） | protobuf 布局版本变更、累计与压缩差额、缺时间戳、结束时刻语义、品牌迁移双根 |
| M8 开源同构类（Roo/gjc/Command/jcode/Codebuff/Continue/Crush/Aider/Amazon Q） | rewind/fork 分支、删除汇总、子代理重放、缓存包含关系差异、需启用日志、SSO 丢历史 |

<a id="session-archives-and-recoverable-history"></a>

## 会话归档与历史可回采性

<a id="会话归档与历史可回采性2026-09-27-复核"></a>

来源归档与本应用分级保留是两件事。发现来源时也检查已证实的归档记录；
同一来源的活动/归档副本共用原生调用身份。没有共同调用 ID 的文件或数据库必须明确主来源，
不能按时间接近、token 相同或会话 ID 相同猜测同一请求。

| 来源 | 已核验入口与行为 | 实现和核验范围 |
| --- | --- | --- |
| Codex | CODEX_HOME 下 sessions 与 archived_sessions；官方 thread/archive 移动 JSONL，恢复时移回 | 两目录合并发现，只有归档目录也可采集；活动/归档副本与反复重扫去重。当前本机 306 个活动文件、无真实归档，归档行为由官方规则及合成移动/副本测试验证 |
| Qwen Code | 旧 tmp 与新 projects 下各项目的 chats/archive 为独立归档目录；daemon 将活动 JSONL 移入或移出归档，异常情况下活动和归档副本可同时存在 | 扫描两种布局的 chats 与 archive，按同来源原生 uuid 去重；QWEN_RUNTIME_DIR、QWEN_HOME 与手工根按已证实路径选择，相对 Agent 工作目录需配置绝对手工根；0.25.0 容器真实主循环已核对，真实归档及其他版本仍另验 |
| ZCode | cli/db/db.sqlite 的 model_usage 保留已完成调用，JSONL 可能删除/压实 | DB 为同一来源的主要记录格式，按原生 id 及来源/日快照替换；从未有 DB 时才读 JSONL。使用过 DB 后丢失或损坏即报错并保留旧统计，不回退造成双计。旧 JSONL 未识别记录若无法与 DB 调用可靠对应，保留健康告警，不直接相加或抹除 |
| Claude Code | projects 中主/子会话、orphaned JSONL 与 superseded 副本 | 现有递归发现覆盖这些路径，以请求/消息身份去重；已有合成测试文件，当前无真实本机归档可核对 |
| Kilo / OpenCode | SQLite session.time_archived 是归档标记，调用仍在 message/part | 查询不排除归档会话；补充归档、恢复、再次归档的完整过程测试。Kilo 使用真实脱敏测试文件，OpenCode 为固定源码规则合成测试文件 |
| Gemini CLI / Kimi Code / Kimi Work / pi / oh-my-pi | 已知本地会话文件；Gemini 自动保存与手动 checkpoint 不同 | Gemini 自动清理默认约 30 天，是删除而非另一个归档目录；手动 checkpoint 可能复制会话，但没有已验证的独立逐次用量规则，不能叠加。其他来源现有目录按格式采集；omp usage_history 是额度窗口，不能当调用历史 |
| Cline / Zoo / Hermes / DSH | 按各自已验证格式处理压缩、删除汇总或区间记录 | 沿用能力矩阵，不能把汇总反推出逐次明细；无本机真实档案的来源仍待验收 |
| OpenClaw | 已核验 schema 24 hot transcript 与旧迁移/冷归档分离 | 已核验正桶入账；默认零、底层调用与完整总量保持未知；冷归档缺口可见，旧 JSONL 不叠加，其他协议/版本单独核验 |

ZCode 安装包的 recordModelUsage 会按 started_at 清理约 30 天前的记录。
DB 不是永久完整档案：边界日已有统计时不以可能不完整的快照覆盖；来源清掉的旧日保留
在应用汇总中。新发现来源只能统计当前仍存在的数据，不能恢复从未采集且已被删除的记录。
DB 没有逐行产品版本，结构校验通过后使用 latest_fallback 标记，不宣称逐版本验证。

最新真实核对：8,225 次调用、2,398,950,011 Token；再次扫描新增为 0、修订不变。
turn_usage 有 24 个轮级对账差异，仅作对照，不能与 model_usage 相加。
会话文件目前存在不能证明产品永远不清理；清空应用统计前的备份仍有必要。

依据：[Codex App Server](https://learn.chatgpt.com/docs/app-server)、
[OpenCode Session 源码](https://github.com/anomalyco/opencode/blob/dev/packages/opencode/src/session/session.ts)、
[Qwen Code 会话归档规则](https://github.com/QwenLM/qwen-code/issues/6057)、
[Qwen Code 活动/归档副本问题](https://github.com/QwenLM/qwen-code/issues/9688)、
[Qwen Code 环境路径](https://github.com/QwenLM/qwen-code/blob/main/docs/users/configuration/settings.md)、
[Gemini CLI 会话管理](https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/session-management.md)；
本机安装包指纹、只读 SQL、脱敏结果及测试入口见[审查记录](../../validation/desktop-usage/review-2026-09-27.md)。

<a id="progress-for-unverified-tools"></a>

## 暂未证实工具的推进方式

M4 保留新版 Kimi Code、Kimi Work、ZCode、WorkBuddy；初始范围将 JetBrains/TRAE、Zed 内置及
未核验 IDE 变体后移 F1。2026-09-29 第二批调研后：
Junie CLI 与 Zed 内置（threads.db）已取得本地记录格式资料，从 F1 转入 M8；
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

M8 注册范围见 [第二批实施记录](../../validation/desktop-usage/m8-second-batch.md)：18 个适配器
（17 个解析 + Qoder 探针）；Amazon Q/Codebuff 经源码核验确认本地无逐次 token 记录、
iFlow 已停服（2026-04-17）且唯一已识别的用量格式是需启用的 OTel（归 M5），三者不实施
token 适配器；Cursor/Windsurf 维持 F1。历史十源样本见
[容器样本](../../validation/desktop-usage/m8-container-samples.md)；本轮另取得 Zed 实样，
容器账户/协议限制与 F1 本机核查见 [本轮记录](../../validation/desktop-usage/plan-20261007.md)。
当前环境未取得样本的项移出 Plan.md 活动待办；文档级实现不能核验未核验的版本或记录格式。
