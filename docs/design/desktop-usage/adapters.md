# Agent 接入调研与能力矩阵

调研日期：2026-09-24。下表是实施候选和证据状态，不是已发布支持列表。
本轮未安装或启动这些 Agent，未读取本机个人会话，也未用真实服务发起请求。
用户列出的 Codex 重复项合并，CLI/桌面/IDE 仍须分别标识产品表面。
Harness Agent 按用户提供官网更正为 Hermes Agent。所有工具均保留本地支持计划；
按用户最新决定，缺少本地格式证据的 IDE 后移 F1，当前不探测/实施。
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

| 工具/变体 | 拟接入来源与可用信息 | 边界与下一步 | 阶段/证据 |
| --- | --- | --- | --- |
| Claude Code | projects 下会话/子 Agent JSONL；官方 OTel 的 input/output/cacheRead/cacheCreation、模型与请求事件 | 本地 JSONL 逐请求形态需 fixture；遥测需显式启用，防会话/辅助统计重叠 | M2/M5；本地候选/需启用，A01 |
| Cline | 适配器已实现（M3，2026-09-25，文档级证据：固定源码 dcf8c3c；本机未安装 not_found，真实数据验收后置） | say 载体四桶互斥 + request 键；compaction 估算不入账；deleted_api_reqs/subagent_usage 聚合处理；不能每条 say 算请求；模型逐请求归属需真实样本核验；CLI SDK 单独探测 | M3 已实施（待真实验收），A03 |
| CodeBuddy Code / IDE / 插件 | CodeBuddy Code 官方 OTel model_stream，输入/输出/总 token、模型和 TTFT；IDE 有积分用量页 | CLI 官方合同不能自动推广到 IDE/插件；model_request 与 model_stream 不双计；缓存字段尚需样本 | CLI M5；缺证 IDE/插件 F1，A04 |
| Codex CLI / 桌面 / IDE | 原型读 CODEX_HOME 下 rollout 的 token_usage_record；官方 OTel 支持请求、响应完成 usage 和流事件 | 已验证版本（逐版本脱敏 fixture）：0.155.0-alpha.16.3（M0/M2-A）、0.154.0-alpha.6.1/6.2、0.153.0（M2-D），逐次/累计/turn_context 三类记录，compaction 重置累计快照；0.139–0.151 本机实测无 token_usage_record（仅 token_count 累计快照），按未知版本回退尝试并判不兼容，待专用旧版实现取证；未知新版本默认 latest_fallback 带兼容标记；原型固定默认路径需改为发现/配置；旧 token_count 与新记录必须分格式；模型从结构化上下文归属，缺 response ID 不碰撞 | M2/M5；本地候选/需启用，A02 + 原型 |
| GitHub Copilot CLI | 官方 OTel 的 chat span、token 统计，支持 JSONL file exporter；原型另有 assistant_usage_events SQLite | 优先官方文件/OTLP；本机 1.0.73 实测 events.jsonl 无逐次 token，session-store.db 的 assistant_usage_events 逐 turn 全字段（M0 fixture）；不能认定所有版本都有该库；premium request 独立；2026-09-25 盘点发现 ~/.copilot 用量存储全树消失（疑升级迁移），M5 实施前需重定位 | M5；需启用，A05 |
| JetBrains 内置 AI Assistant / Junie | 后续探测 IDE 本地日志、插件数据、会话存储/本机导出 | A07 只证实企业远端分析，已排除；本地 token/cache schema 待证，当前不探测/开发 | F1；后续支持，A07 为范围排除依据 |
| DeepSeek Harness（DSH） | 适配器已实现（M3，2026-09-25，文档级证据：token-meter README 46a7f68；本机未安装 not_found） | final 替换流式、retry 新开计费 attempt、边界定稿末样本（V03 数学样本语义）；pressure 估算不计账；occurred_at 用观察时间（无逐事件时间）；落盘行形状为合成假设待真实样本 | M3 已实施（待真实验收），A08 |
| Hermes Agent（原需求 Harness Agent） | 适配器已实现（M3，2026-09-25，文档级证据：固定源码 ef70b36；本机未安装 not_found） | session_model_usage 按合同映射区间汇总（不虚构逐请求/每日分桶）；api_call_count 不拆调用；跨日归属/辅助互斥/压缩继承按 A24；日志详单能力待真实样本 | M3 已实施（待真实验收），A24 |
| OpenClaw | 适配器已实现（M3，2026-09-25，文档级证据；本机未安装 not_found） | 官方文档未给出表级/条目级 schema ⇒ 运行时库与旧归档均 fail closed（诊断注明待真实样本）；归档按迁移输入降级（doctor --fix 路径）；不沿用“只扫 JSONL”假设；远端 Gateway 不等于本机 | M3 已实施（fail-closed 占位，待真实样本扩展），A09 |
| Gemini CLI | 官方会话存储 ~/.gemini/tmp/<project_hash>/chats/ 含 token；OTel 提供 input/output/thought/cache/tool 和请求统计 | JSON/JSONL 具体版本探测；thought/cache/tool 字段的包含关系逐 provider 核验；Gemini 网页和 Code Assist 不冒充 CLI | M2/M5；本地候选/需启用，A10 |
| Kilo Code CLI / 桌面 / IDE 扩展 | 适配器已实现（M3，2026-09-25）：SQLite 只读 + 暂存副本合同；message.data.tokens 逐次五互斥；水位增量；session 行 tokens 列仅作会话对账（275/276 matched，列级语义随版本不稳定不映射事件）；真实核对 13,342 调用/1.65B token 与独立求和一致、幂等 | 已验证 7.4.8/7.4.9（fixture）；库内 7.3.42–7.7.12 未收录走 latest_fallback；新 core 数据层（session_message，当前 0 行）切换后需专用实现取证；IDE 扩展后移 F1；不能拿 CLI 通过代表 IDE 通过 | CLI/桌面 M3 已实施；缺证 IDE 变体 F1，A11 |
| 新版 Kimi Code | 适配器已实现（M4，2026-09-25）：家族共享 kimi_wire.rs；注册表锚点 protocol_version=1.5；真实核对 13 文件 965 调用 total 116,428,813 与 jq 逐字段相等、幂等；打断步回声缺失以 echo_subset 可见 | usage.record 无稳定 ID ⇒ provider/逐次延迟不入账（如实 unavailable）；旧会话 1.4 走 latest_fallback；其他版本仍需各自 fixture | M4 已实施；跨版本待证，A12 |
| Kimi Work | 适配器已实现（M4，2026-09-25）：独立目录（A13 不与 Kimi Code 合并）；conv-*/ctitle-* 布局（daimon 宿主）；锚点 1.4；真实核对 69 文件 1,337 调用 total 125,225,933 逐字段相等、69/69 对账 matched；跨文件同毫秒（swarm）事件键含 session:agent 身份段 | 官方默认布局/env 未见文档（迁移需手工加根）；subagent.completed 与缓存写>0 无本机真实样本；与 Kimi Code 共享 wire 解析但统计分列 | M4 已实施，A13 |
| MiMo Code | 官方开源；固定源码 message tokens 有 input/output/reasoning/cache.read/write，路径经 resolveMimocodeHome | 单独 namespace 与 schema 探测；不能因 OpenCode 派生关系直接使用其目录；step 与 message 汇总不双计 | M3；本地候选，A14 |
| oh-my-pi | 官方 session 文档确认 ~/.omp/agent/sessions/...jsonl，assistant usage 与模型/provider | 本机 18.2.7 已取 3 个脱敏 fixture（2026-09-25，[M2-B/C 恢复记录](../../validation/desktop-usage/m2bc-resumed.md)）：assistant 全带 usage + duration/ttft 浮点毫秒；model_change 落盘为 `model`="provider/model" 组合字段（非 pi 分字段）；子 Agent 文件在 `<ts>_<父UUID>/` 目录按路径形状归父；~/.omp/logs 仅上下文估算 debug 行、无逐次用量，title-generator 无重叠证据；compaction/branch_summary 本机均无 usage（辅助载体合同已实现，待真实样本） | M2；本地候选，A15 |
| pi | 固定源码 Usage 含 input/output/cacheRead/cacheWrite/reasoning；session 有 message、独立 usage、compaction/branch summary usage | 推理已含于 output；仅 assistant message 会漏辅助用量；分支继承不代表新调用；目录由该版本配置函数解析；本机 0.87.1 已取真实 fixture 并通过幂等核对（2026-09-25，[M2-B/C 恢复记录](../../validation/desktop-usage/m2bc-resumed.md)）；fork 复制条目带 fork 会话身份，仲裁为 conflict 保留先扫者，净效果不双计 | M2；本地候选，A16 |
| OpenCode | 固定源码 core/session/sql 有 token 累计列和事件/投影数据层，支持拆分输入/输出/推理/缓存 | 新 core 与旧 message-v2 不能通用解析；不要把 session 累计列逐次相加；逐调用模型时间继续追 event 层 | M3；本地候选，A17 |
| Qwen Code | 固定 recording service 记录 usageMetadata/model，位于 ~/.qwen/tmp/<project_id>/chats；官方 OTel | 不把 Goal 累计 histogram sum 当总消耗；旧日志缺 spend 时零不等于实测零；显式禁用 prompt 日志 | M2/M5；本地候选/需启用，A18 |
| Zoo Code | 固定源码 consolidateTokenUsage 包含 api_req_started/finished、condense_context、cacheReads/Writes | 独立产品，不能擅自改为 Roo Code；API protocol 影响字段含义；已有字段证据的表面独立验收，缺证 JetBrains 变体后移 | 本地候选 M3；JetBrains 变体 F1，A19 |
| TRAE / TraeCode 及插件 | 后续探测本地 IDE/CLI 日志、会话库、插件数据和本机会话导出 | A20 企业 API/控制台报表不纳入；当前整个产品家族后移，不推测通用目录/schema | F1；后续支持，A20 为范围排除依据 |
| VS Code | 官方 Copilot Chat OTel 的逐 LLM chat span 有模型、输入/输出、可选缓存与推理、TTFT | VS Code 是宿主；与 Copilot/Claude/Codex 等底层 Agent 去重；不承诺读取所有扩展的 usage | M5；需启用，A06 |
| ZCode | 适配器已实现（M4，2026-09-25）：model-io JSONL 双口径——AI SDK 五键为主（inputTokens 含缓存读）、anthropic snake_case 对照互斥校验（矛盾进诊断）；db.sqlite 只读对账（活库 418/438 matched，在途轮 mismatch 可见）；真实核对幂等 | 已验证 3.14.3（真实 fixture）；未收录版本 latest_fallback；`~/.zcode/v2` 布局与 `%APPDATA%/zcode` 桌面存储未接入（待证）；缺 requestId/traceId 时不得全部变成 zcode:None | M4 已实施；跨版本待证，A21 |
| WorkBuddy | 官方日志诊断入口作本地探测线索；核验安装/profile 对应的日志/缓存与本机会话导出 | 套餐/账号积分页不纳入；未证实本地逐请求 token/cache，不能把 CodeBuddy CLI OTel 直接套用 | M4；待证实，A22 |
| Zed | 官方说明 hosted 服务有 token 计量，客户端 telemetry log 可查看；外部 Agent 有独立数据源 | 内置存储 schema 后续核验；外部 ACP Agent 仍按底层适配器读取原生日志，不计为 Zed 内置支持 | 内置 Agent F1；外部来源按原阶段，A23 |

本轮没有将任何“未发现文档”写成“该产品不可能支持”。
若不能取得可靠用量，仍可展示该工具状态与限制，但不占据有数值的总计行。

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

- pi/oh-my-pi 可共享部分 Usage 类型知识，辅助事件及分支恢复仍各测。
- OpenCode/Kilo/MiMo 可共享结构探测工具，不共享未经验证的目录、表名或累计/增量假设。
- Cline/Zoo/旧 Kilo 扩展的 API 记录可以共享解析组件，但子 Agent 汇总、删除与压缩记录逐项测试。
- VS Code/Copilot/宿主嵌入的 Claude/Codex 不按应用品牌重复计费，使用 origin 归属关系。
- Kimi Work 和 Kimi Code 的数据根、实例身份和日志 revision 独立，重叠来源显式处理。

## 每个适配器的交付合同

代码布局遵守 [Agent 独立目录与版本组织](architecture.md#adapter-layout)。
每个 Agent 一个目录，历史版本实现在目录内分模块；该要求适用于本表全部阶段，
已验证支持与未知版本兼容尝试分别记录，不把 M2 中尚待执行的目录迁移或兼容策略记为已实现。

| 项目 | 必须交付的内容 |
| --- | --- |
| 目录与版本 | Agent 独立目录、统一入口、发布版本/来源格式到内部实现的映射、逐版本 fixture；新增版本保留历史实现及回归，按 V30 验收 |
| 发现 | 产品/表面/OS/版本范围、本机归属依据、候选路径、环境覆盖、profile、手工添加方法 |
| 探测 | 逐文件/数据库识别 Agent 与输入类型；已知版本按映射分派，未知版本先尝试该 Agent 最新内置解析器；结构/语义不兼容或匹配冲突时明确报错 |
| 字段映射 | 每个 token 字段包含关系、单位、时间基准、模型归属、调用 ID、request 证据 |
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
| Copilot/VS Code/CodeBuddy | chat/model_stream 与父 span 去重、file/OTLP 重传、cache 缺失、无 usage 的失败、SDK/宿主重叠 |
| JetBrains/TRAE/其他本地待证源 | 本地日志/数据库真实样本、版本/插件差异、缺 usage、同区间重导入；远端同步/账号报表必须排除 |

## 暂未证实工具的推进方式

M4 保留新版 Kimi Code、Kimi Work、ZCode、WorkBuddy；JetBrains/TRAE、Zed 内置及
表中缺证 IDE 变体列入 F1，首版不探测/实施。各产品到其所属阶段后，按
“识别版本→本地存储/本机导出/本地遥测→最小脱敏样本→字段/生命周期测试”推进。
不能因同属 IDE 后移有明确本地字段证据的 Cline 或已文档化的 VS Code 遥测。
每个工具留下核验版本、已查的官方本地入口/候选格式、授权范围、读取结果、缺失字段、
失败原因和后续条件；没有本机版本/样本时记“未尝试/待样本”，不能写“已验证不支持”。
只有套餐/账号额度页时显示本地用量暂不可提取；只有文本无可靠 usage 时不默认使用 tokenizer 估算。

不通过 TLS 中间人、浏览器会话 cookie、进程注入或私有账单接口补齐缺口。
用户未来主动要求自定义 API 网关计量时另做设计；代理不能读取此前历史，
也不一定覆盖官方订阅通道，因此不是当前通用回退方案。

首批之后可按同一准入合同评估 Cursor、Roo Code、Continue、Windsurf 等工具。
本轮未完成它们的格式核验，不将其列入已证实矩阵或首版必达数量。
