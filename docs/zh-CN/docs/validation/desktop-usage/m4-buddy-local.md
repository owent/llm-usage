# WorkBuddy / CodeBuddy Code 本地会话接入（2026-09-29）

<a id="local-workbuddy--codebuddy-code-sessions-2026-09-29"></a>

<a id="证据与边界"></a>

<a id="checked-results-and-limits"></a>

## 核验结果与边界

- [CodeBuddy 官方目录文档](https://www.codebuddy.ai/docs/cli/codebuddy-dir)
  确认 `~/.codebuddy/projects/` 保存会话 JSONL；
  [环境变量文档](https://www.codebuddy.ai/docs/cli/env-vars)确认
  `CODEBUDDY_CONFIG_DIR` 可覆盖配置/数据根目录。
  [官方监控文档](https://www.codebuddy.ai/docs/cli/monitoring)
  另给出 OTLP 导出，默认未启用。
- [AgentHUD 的 provider 定义与源码索引](https://github.com/jazzenchen/agent-hud-open/blob/main/docs/providers.md)
  确认 CodeBuddy 与 WorkBuddy 分别读取 `.codebuddy/projects`、
  `.workbuddy/projects`，使用 assistant `message` / `function_call`
  的本地用量，不读远端额度。
  [tokmesh-core 的解析器和合成测试](https://docs.rs/tokmesh-core/latest/src/tokmesh_core/sessions/tencent_buddy.rs.html)
  提供 JSONL 字段和身份样本。
  [aiusage v1.5.8 变更记录](https://github.com/juliantanx/aiusage/blob/main/CHANGELOG.md)
  指出 CodeBuddy 的 `message.usage.input_tokens` 已含缓存读，另有
  `providerData.rawUsage` 的 cache hit/miss，直接沿用 Claude Code
  加法会重复计数。
- 后两项是第三方源码与测试结果，并非产品官方格式规范。
  初查未发现 WorkBuddy 会话目录，随后复查发现本机 `.workbuddy/projects`；
  `.codebuddy/projects` 仍不存在。WorkBuddy 已补真实会话只读核对，
  CodeBuddy 仍仅有文档依据与合成测试结果。

<a id="implementation"></a>

## 实施

- 分别注册 `codebuddy`、`workbuddy` 来源，按明确目录有界枚举 `.jsonl`。前者支持 `CODEBUDDY_CONFIG_DIR`；手工根仅接受对应产品目录下的 `projects`，避免两个同形解析器互抢。
- 只读已完成的 assistant `message` / `function_call`，要求可信毫秒时间戳、session 和消息身份。token 缺失保持未知；非法数值、缓存读超过总输入或 hit/miss 与总输入矛盾时跳过并记诊断。以 `产品:session:messageId/traceId/id` 为键，行号作为同文件后写修订。
- `input_tokens`/`prompt_tokens` 按含缓存的输入总量处理；明确的 `prompt_cache_miss_tokens` 记未缓存输入，只有可核对时才做减法。`reasoning` 保留在输出分项；未证实的 provider、价格和 trace 汇总不推断。
- 输入与输出均已知且不越界时推导统一总 token；来源总量另存并与推导值核对，矛盾记录不入账。

<a id="validation-and-remaining-conditions"></a>

## 验证与未完成条件

- `cargo test --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core tencent_buddy_wire --lib`：3 项合成单测通过，验证 CodeBuddy 缓存不重复计数、WorkBuddy 身份独立、非法缓存分项跳过、默认目录发现和增量游标。
- `cargo test --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core --test adapter_layout_v30`：3 项通过，含两个产品的独立目录/注册表检查。
- 本机 WorkBuddy：`last-launch.json` 报告最近启动版本 5.6.2
  （build `37a65c0b33b8e394904eb49e6c218c4cc0601649`；不代表所有历史记录均由该版本写入）。
  只读字段探测覆盖 8 个 JSONL 文件、420 条含 `rawUsage`
  的用量记录；420/420 条满足 `prompt=cache_miss+cache_hit`、
  `total=prompt+completion`、消息用量与 rawUsage 输入/输出一致，
  420 条消息身份均唯一。真实扫描命令：
  `cargo run --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core --example real_verify_workbuddy -- <home> build/workbuddy-review/real-check-20260929`。
  结果：8 文件、420 事件、诊断 0；再次扫描新增 0，持久化事件数仍为 420；
  `input=uncached+cache_read`、`total=input+output` 均通过。
- 本机 `~/.workbuddy/traces` 有 6 个 `trace_*.json`；6/6 个
  `trace.totalTokens` 为 0，仅 2/6 个有 `modelInfo` 数值字段，
  `trace.traceId` 与会话用量行的 `providerData.traceId` 未匹配。
  因缺少可验证的重复关系和覆盖证明，trace 不叠加到这 420 条逐次记录。
- Windows 11 x64、Node 22+ / Rust 锁文件环境：`npm run verify` 退出码 0
  （Markdown、资产、脚本/UI、Svelte、fmt、clippy、Rust 测试、Web 构建均通过）；
  Node 子进程在默认沙箱返回 EPERM，获自动审核允许后原命令复跑通过。
  `git diff --check` 退出码 0。统一检查执行于真实样本复查之前，
  后续以定点 Rust 测试、clippy、Markdown lint 和格式检查补验改动。
- 后续需取得 CodeBuddy CLI 本机真实样本；WorkBuddy 仍需核对更广版本、
  子 Agent/trace 覆盖关系以及会话删除/压实后的历史保留。
- CodeBuddy 的 OTLP 与本地 JSONL 可能覆盖同一次调用，当前尚无跨来源去重依据或代码；同时启用会有重复计数风险。验收前择一本地会话或 OTLP 来源使用；既有历史重叠须另做来源级消解，不以简单求和宣称完整覆盖。

<a id="codebuddy-ideplugin-local-checks-and-implementation-2026-09-30"></a>

## 2026-09-30 CodeBuddy IDE/插件（CodeBuddyExtension）本机核验与实施

<a id="扩展存储证据与边界"></a>

<a id="extension-storage-checks-and-limits"></a>

### 扩展存储核验结果与边界

- 用户报告 CodeBuddy 用量提取为 0。只读排查：本机 `~/.codebuddy` 只有
  插件市场/设置/日志（`cli-memwatch-*` 证明 CLI 运行时存在），没有
  `projects` 目录，CLI JSONL 文件在本机无样本，原适配器发现为空即根因。
- 逐一核验 IDE 侧候选均无逐次 token：`%APPDATA%\CodeBuddy CN` 的
  `codebuddy-sessions.vscdb`（ItemTable 0 行）、`automations.db`
  （两表 0 行）、`globalStorage/state.vscdb`（仅设置键）、
  genie-history（无 token 字段）、workspaceStorage、
  `%LOCALAPPDATA%\CodeBuddyExtension` 的 check-point/file-tree/plan-task。
- 实际用量文件：`%LOCALAPPDATA%\CodeBuddyExtension\Data\
  <profile>\<host>\<workspace>\history\<session>\<conversation>\index.json`
  的 `requests[]`，每项 `{id,type,messages,state,startedAt,usage}`；
  `usage` 为请求级聚合（`inputTokens = cacheTokens + cachedMissTokens`、
  `totalTokens = inputTokens + outputTokens`，另有 `cachedWriteTokens`、
  `lastTokens` 语义尚未核验、`credit` 平台积分非货币）。上层 session 级
  `index.json` 只有 `conversations[]/current` 注册表，不含用量。
  消息体 `messages/<id>.json` 的 `extra`（字符串内嵌 JSON）携带
  `modelId/modelName/requestId/isHelperMessage`。按准备说明只提取
  允许字段，未读取/输出消息正文。
- 本机样本：14 个 conversation index（含空请求）；仅 1 个 conversation
  有 4 个 `type=craft`、`state=complete` 请求——1 个含真实用量
  （input 64339 / output 1822 / cache read 53504 / miss 10835 /
  total 66161，模型 `kimi-k3-1`），3 个全零（单条用户消息、无模型调用，
  不入账）。4/4 满足 `input=cache+miss` 与 `total=input+output`。
  另观察到同一 session hash 出现在多个 profile/workspace 树（conversation
  id 各异），复制场景下同请求可能重复出现。

<a id="extension-implementation"></a>

### 扩展存储实施

- `codebuddy` 适配器改为复合入口：CLI JSONL（tencent_buddy_wire，不变）
  与扩展存储（新 `codebuddy/extension_store.rs`）按路径形状分派，共用
  agent 身份；WorkBuddy 不受影响（有守卫测试）。
- 发现：`LOCALAPPDATA` 下 `CodeBuddyExtension/Data` 单一根 + 有界枚举
  conversation 级 index.json（session 级注册表不进扫描）；手工根仅接受
  该 Data 目录。
- 解析：`state != complete`、`type != craft`、缓存分桶矛盾或总量矛盾
  记诊断跳过；全零用量视为无模型调用不入账；记录键
  `codebuddy:request:<请求id>`（UUID），同请求跨目录树复制在单实例内重复读取不重复新增
  upsert 不双计；`session_id` 取 conversation id，`host_application` 取
  host 组件（VSCode/CodeBuddyIDE）；模型按 requests[].messages 引用做
  有界消息文件读取，取最后一条有模型信息的消息（请求内多模型尚未核验）；
  `credit`/`lastTokens` 不映射。游标 offset 记录文件长度供无变化短路，
  改写触发整文件重扫后按键避免重复新增。
- 版本注册表新增 `codebuddy-extension-requests-doc1`（本机核验的格式依据）。

<a id="extension-validation-and-remaining-conditions"></a>

### 扩展存储验证与未完成条件

- `cargo test --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core --lib adapters::codebuddy`：4 项通过
  （请求解析与全零/未核验状态跳过、同文件重复请求折叠、跨树同请求同键、
  复合分派与 WorkBuddy 守卫）。
- 本机真实数据：`cargo run --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core --example real_verify_codebuddy -- <home> <localappdata> build/codebuddy-local-verify/work`，
  结果 files=14、added=1、diagnostics=0、模型 `kimi-k3-1` 1 条；
  复扫 added=0、事件数稳定，`input=uncached+cache_read`、
  `total=input+output` 通过，verdict=PASS。数值与本机只读检查结果一致。
- `npm run fmt:check`、`npm run clippy`、`npm run test:rust` 退出码 0。
- 未完成：CLI `projects` 文件本机仍无真实样本（文档级维持）；扩展
  usage 为请求级聚合，一次回合内的多次模型调用不可分；请求内多模型
  归属、`lastTokens`/`credit` 语义待更多样本。
- 统一检查补跑（2026-09-30，仓库根）：`npm run verify` 退出码 0
  （Markdown、资产、脚本/UI、Svelte、fmt、clippy、Rust 全量测试、
  Web 构建全通过）；`npm run test:browser` 退出码 0
  （日历热图、十语言、主题、时区、过期响应、用户隔离、成员、分页、
  空闲轮询检查通过）。前述"统一检查未重跑"缺口关闭。
