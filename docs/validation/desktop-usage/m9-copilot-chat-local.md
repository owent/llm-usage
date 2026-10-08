# Local VS Code built-in Copilot Chat sessions, 2026-10-01

<a id="vs-code-内置-copilot-chat-本地会话接入2026-10-01"></a>

This record preserves the first implementation results. The same-day [review and
corrections](m9-copilot-review.md) supersede its “10 requests”, “one turn = model_call”,
derived input-plus-output total and no-workspace directory conclusions. Current counts
are 10 usage turns and 217 observed main-loop rounds; usage and calls are recorded separately.

<a id="背景与核验过程"></a>

<a id="背景与证据链"></a>

## Background and investigation

- The user reported existing VS Code Copilot data and requested actual token usage/call
  counts, beyond credit/premium_interactions quota. This investigation overturned the
  earlier conclusion that VS Code Copilot Chat saved no per-call token information locally.
- Read-only inspection of local candidate files/databases used only the permitted fields
  in the preparation instructions, without printing message text:
  - APPDATA/Code/User/globalStorage/github.copilot-chat/session-store.db: chronicle v3
    sessions/turns/checkpoints/search_index; turns.assistant_response is plain text without
    usage. Related CLI layout remains rejected by the CLI adapter when usage format is unknown.
  - transcripts/*.jsonl: assistant.message/turn_start/turn_end/tool.execution, without
    token fields or model names.
  - debug-logs/id/main.jsonl: session_start only; chatEditingSessions: editing state.
  - Global/workspace state.vscdb: lmBaseCount/model is the same ordering seed, 204, for
    every model, rather than actual counts; github-*-usages stores authentication-extension lastUsed.
  - **Actual usage files: workspaceStorage/hash/chatSessions/sessionId.jsonl**, VS Code
    native session logs with promptTokens/completionTokens/copilotCredits/elapsedMs/
    promptTokenDetails/modelTotals/toolCallRounds per request.
- microsoft/vscode source inspected on 2026-10-01 established field semantics for local
  VS Code 1.140.0 / built-in Copilot Chat 0.68.0:
  - chatSessionStore.ts saves workspaceStorageHome/workspaceId/chatSessions, including
    no-workspace/chatSessions for windows without a workspace.
  - objectMutationLog.ts: kind 0 initializes the object, including after compact rewrites;
    kind 1 Set(k,v); kind 2 Push(k,v[],i), where i is the length after truncation;
    kind 3 Delete. More than 1,024 entries triggers a whole-file replacement.
  - chatModel.ts toJSON and chatSessionOperationLog.ts storageSchema v3: promptTokens is
    **the last model call's input**, as IChatUsage documents. completionTokens accumulates
    **output across calls within the turn**, through _setUsage; modelTotals replaces that
    value when present. copilotCredits is turn-level credit converted from nano AIU.
    Other fields include elapsedMs/outputBuffer/modelTotals, sessionCopilotCredits and
    modelState with value/completedAt. IChatUsageModelTotal has model/inputTokens/
    cachedTokens/outputTokens accumulated across all model calls, for agent-host sessions
    only. State values: 0 Pending, 1 Complete, 2 Cancelled, 3 Failed, 4 NeedsInput;
    sealed states are 1/2/3.
  - agentIntent.ts forwards API usage.prompt_tokens/completion_tokens and
    prompt_tokens_details.cached_tokens directly.
- Periodic saveState writes sampled streaming counters: for example, 15 updates for
  40 model rounds. **Summing updates cannot reconstruct per-call input.** toolCallRounds
  contains round records, including modelId/thinking.tokens/timestamp.

<a id="实施"></a>

## Implementation

- New copilot_chat adapter, independent directory and session_log_v3 version registry;
  agent=vscode-copilot-chat, matching the OTel adapter's service.name assignment:
  - Discovery: APPDATA/Code/User/workspaceStorage and Code - Insiders, plus corresponding
    macOS/Linux paths; bounded two-level enumeration of hash/chatSessions/*.jsonl and
    no-workspace/chatSessions. Manual roots accept a chatSessions directory, workspaceStorage
    directory or individual .jsonl file. One instance per chatSessions directory.
  - Detection: first-line kind=0 with v.version=3, sessionId and requests array. Version 3
    is KnownVersion; other/missing versions use LatestFallback; empty files remain Pending.
  - **Full replay each run:** sampled object updates cannot be joined through incremental
    reads. Compaction bounds the file; native sample 5.25 MB, largest line 837 KB. Event key
    vscode-chat:sessionId:requestId prevents duplicate insertion. Identical content stays
    unchanged; growing streaming records use Replace. A compact rewrite changes the source
    generation and triggers replay, converging on the same event keys.
  - **Initial mapping, superseded by the review above:** one user turn produced one model_call,
    matching CLI assistant-message granularity. input_total=promptTokens, the last-call input
    and a lower bound for the turn, quality=reported; output_total=completionTokens, the
    whole-turn accumulated output; total was derived. When present, modelTotals replaced
    these with whole-turn totals per model: cached→cache_read, uncached derived, one event
    per model for mixed-model turns. occurred_at=completedAt, falling back to request
    timestamp with source_start; duration=elapsedMs, ttft=result.timings.firstProgress.
    Model preference: resolvedModel, then modelId without copilot/ prefix, then last-round
    modelId. State 1/2/3→Final; 0/4→Partial. Requests without token signals are skipped;
    credits alone do not create token events, and copilotCredits never becomes tokens.
  - No new database tables: existing agent-independent usage_events; quota_history from
    copilot-user-cache.json remains separate under agent=copilot.
- Updated outdated CLI hidden_calls capability wording, the copilot_quota.rs header's
  claim of being the only locally extractable data, and the matching data-contract.md section.

<a id="验证与未完成条件"></a>

## Validation and remaining limits

- Synthetic unit tests: cargo test -p llm-usage-core copilot_chat, 17 passed. Covers
  v3/unknown/non-JSON/empty detection, last Set wins, Partial lifecycle, truncated Push,
  Delete, compact rewrite reset, no-usage skip, single/multiple modelTotals, malformed
  rows, missing timestamp and empty-log Pending.
- Read-only native check: cargo run -p llm-usage-core --example real_verify_copilot_chat
  with a chatSessions directory and output under build/desktop-usage-validation/copilot-chat-real.
  files=3, events=10, diagnostics=0; **historical count=10**, input=3,408,279, output=320,141,
  one model claude-opus-4-8, partial=0. Repeat count unchanged, added=0, verdict=PASS.
  Per-request values match independent redacted extraction through Python object-log replay.
  This historical count does not represent the current model-call count.
- Application daily_usage contained 2026-09-30 / vscode-copilot-chat / claude-opus-4-8 /
  historical count 10 / input_known_sum=3408279 / output_known_sum=320141, visible through
  agent-grouped overview/trend queries.
- npm run verify exit 0: Markdown/Svelte/fmt/clippy -D warnings, 224+ full Rust tests and
  web build; npm run test:browser exit 0.
- Per-call input within a turn is unsaved; default input remains a last-call lower bound,
  replaced by whole-turn modelTotals when available. Incomplete thinking-token coverage
  excludes output_reasoning. Inline chat outside chatSessions is excluded. At this stage,
  simultaneous native/OTel-file collection for the same agent could duplicate counts;
  choose one. Later partition selection is described in the review. completionTokens may
  overestimate when upstream isSameUsage fails to deduplicate repeated backend reports,
  acknowledged by an upstream comment. v4+ uses latest-compatible parsing and still needs
  native samples before a verified-format claim.
