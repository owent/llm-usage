# VS Code 内置 Copilot Chat 本地会话接入（2026-10-01）

<a id="local-vs-code-built-in-copilot-chat-sessions-2026-10-01"></a>

本记录保留首次实施时的验证结果；同日 [审查修正](m9-copilot-review.md) 覆盖其中
“10 请求”“一个 turn = model_call”、默认输入输出派生总量及无工作区目录的结论。
当前为 10 个用量 turn、217 个已观测主循环 round；调用与用量分别入账。

<a id="背景与证据链"></a>

<a id="background-and-investigation"></a>

## 背景与核验过程

- 用户报告：当前 VSCode 里的 Copilot 有使用数据，要求提取真实 token 用量与请求数，
  而非仅 credit（premium_interactions 额度）。此前结论（2026-10-01 前）认为
  "VS Code Copilot Chat 本地不落盘逐次 token"——本轮推翻。
- 只读检查本机所有候选文件和数据库（按准备说明中允许的字段提取，不输出正文）：
  - `%APPDATA%\Code\User\globalStorage\github.copilot-chat\session-store.db`：
    chronicle v3（sessions/turns/checkpoints/search_index），`turns.assistant_response`
    为纯文本，无 usage（与 CLI 同族布局，维持 CLI 适配器 fail-closed 处理）；
  - `transcripts/*.jsonl`（扩展转录）：assistant.message/turn_start/turn_end/
    tool.execution，无 token、无模型名；
  - `debug-logs/<id>/main.jsonl`：仅 session_start；`chatEditingSessions`：编辑状态；
  - 全局/工作区 `state.vscdb`：`lmBaseCount/<model>` 为全模型同值 204 的排序种子
    （非真实计数）；`github-*-usages` 为认证扩展 lastUsed；
  - **实际用量文件：`workspaceStorage/<hash>/chatSessions/<sessionId>.jsonl`**——
    VS Code 原生会话日志，逐请求携带 promptTokens/completionTokens/copilotCredits/
    elapsedMs/promptTokenDetails/modelTotals/toolCallRounds。
- microsoft/vscode 源码锁定语义（2026-10-01 拉取核对，本机 VS Code 1.140.0 /
  内置 Copilot Chat 0.68.0）：
  - `chatSessionStore.ts`：保存到 `workspaceStorageHome/<workspaceId>/chatSessions`
    （无工作区窗口 `no-workspace/chatSessions`）；
  - `objectMutationLog.ts`：行格式 Entry——kind 0 初始（首行/压缩重写后）、
    kind 1 Set(k,v)、kind 2 Push(k,v[],i=截断后长度)、kind 3 Delete；
    超 1024 条整体重写（replace）；
  - `chatModel.ts` toJSON + `chatSessionOperationLog.ts` storageSchema v3：
    `promptTokens` = **末次模型调用输入**（IChatUsage.promptTokens 描述最近一次调用）、
    `completionTokens` = **整 turn 跨调用累计输出**（_setUsage 逐调用累加，
    modelTotals 存在时以该值为准）、`copilotCredits` = turn 级 credit（nano AIU 折算）、
    `elapsedMs`、`outputBuffer`、`modelTotals`（IChatUsageModelTotal：
    model/inputTokens/cachedTokens/outputTokens，"sums across every model call
    the response made"，仅 agent host 会话提供）、`sessionCopilotCredits`、
    `modelState{value,completedAt}`（0 Pending/1 Complete/2 Cancelled/3 Failed/
    4 NeedsInput，sealed=1/2/3）；
  - `agentIntent.ts`：数值直传 API 响应 `usage.prompt_tokens`/`completion_tokens`/
    `prompt_tokens_details.cached_tokens`。
- 采样快照判定：流式计数器更新序列（如 15 次更新对 40 轮调用）由周期 saveState
  保存，是采样而非逐调用记录——**不能对更新序列求和恢复逐调用输入**；
  `toolCallRounds` 才是逐轮记录（含 modelId/thinking.tokens/timestamp）。

<a id="implementation"></a>

## 实施

- 新增 `copilot_chat` 适配器（独立目录 + 版本注册表 `session_log_v3`，
  agent=`vscode-copilot-chat`，与 otel 适配器对该面的 service.name 归属一致）：
  - 发现：`%APPDATA%\Code\User\workspaceStorage`（及 `Code - Insiders`；
    macOS/Linux 对应路径）两级有界枚举 `<hash>/chatSessions/*.jsonl` 与
    `no-workspace/chatSessions`；手工根支持 chatSessions 目录、workspaceStorage
    目录或单个 .jsonl；每个 chatSessions 目录一个实例；
  - 探测：首行 `{kind:0, v:{version:3, sessionId, requests[]}}` 结构；
    version=3 KnownVersion，其他/缺失 LatestFallback；空文件 Pending；
  - 扫描：**每轮全量重放**（kv 采样快照不可增量拼接；文件经压缩有界，
    真实样本 5.25MB/最大行 837KB），事件键 `vscode-chat:<sessionId>:<requestId>`
    重复写入不重复新增（同内容 unchanged，流式增长走 Replace；压缩重写导致来源代数变化，按读取规则
    触发重扫后同键收敛）；
  - 首次实施的映射（已由上述审查修正）：一个 user turn = 一条 model_call（与 copilot CLI assistant 消息级
    粒度一致）；input_total=promptTokens（**末次调用输入，turn 输入下界**，
    quality=reported）、output_total=completionTokens（整 turn 累计）、
    total 派生；modelTotals 存在时按整轮逐模型累计值替换（cached 记
    cache_read、uncached 派生；多模型时逐模型一事件）；occurred_at=
    completedAt（完成时刻，缺失回退请求 timestamp 并标 source_start）；
    duration=elapsedMs、ttft=result.timings.firstProgress；模型
    resolvedModel > modelId（去 copilot/ 前缀）> 末轮 modelId；
    lifecycle 按 modelState 1/2/3=Final、0/4=Partial；无 token 信号的请求
    跳过（仅 credit 不构成事件）；copilotCredits 不入 token；
  - 数据库无新增表：usage_events 为既有 agent 无关通用表；额度时序沿用
    上一轮的 quota_history（copilot-user-cache.json，agent=copilot）。
- 同步修正过时结论：copilot CLI 适配器 capability 的 hidden_calls 说明、
  `copilot_quota.rs` 文档头"本机唯一可提取"表述、data-contract.md 对应段落。

<a id="validation-and-remaining-limits"></a>

## 验证与未完成条件

- 合成单测：`cargo test -p llm-usage-core copilot_chat`——17 项通过
  （v3/未知版本/非 JSON/空文件探测、Set last-wins 映射、Partial 生命周期、
  Push 截断、Delete、压缩重写重置、无 usage 跳过、modelTotals 单/多模型、
  坏行诊断、缺时间戳跳过、空日志 Pending）。
- 本机真实数据（只读）：
  `cargo run -p llm-usage-core --example real_verify_copilot_chat --
  <chatSessions 目录> build/desktop-usage-validation/copilot-chat-real`：
  files=3、events=10、diagnostics=0；历史 count=10（不是当前模型调用数）、input=3,408,279、
  output=320,141、models=1（claude-opus-4-8）、partial=0；重扫 count 不变、
  added=0，verdict=PASS。数值与独立脱敏提取（Python 重放 kv）逐请求一致。
- 应用管线：同库 `daily_usage` 物化 `2026-09-30 / vscode-copilot-chat /
  claude-opus-4-8 / 10 次 / input_known_sum=3408279 / output_known_sum=320141`，
  总览与趋势查询按 agent 分组即可见。
- 统一检查：`npm run verify` 退出码 0（Markdown、Svelte、fmt、clippy -D warnings、
  Rust 全量 224+ 测试、Web 构建）；`npm run test:browser` 退出码 0。
- 未完成/边界：turn 内逐调用输入未保存（input 为末次调用下界，
  modelTotals 出现后改用整轮逐模型累计值）；thinking tokens 覆盖不全不入
  output_reasoning；inline chat 等不经 chatSessions 的面不计；与 otel
  file exporter 文件（同 agent 维度）首次实施时同时启用会重复计数，需择一使用；后续按来源分区选择的规则见审查修正；
  `completionTokens` 对后端重报（上游 isSameUsage 去重失效场景）可能高估，
  上游注释已承认；跨版本（v4+）走 latest 兼容尝试仍需真实样本确认。
