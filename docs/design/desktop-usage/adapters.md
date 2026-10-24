# Agent research and capability matrix

<a id="agent-接入调研与能力矩阵"></a>

Initial research: 2026-09-24; archive behavior reviewed 2026-09-27, as described below and
in [the review](../../validation/desktop-usage/review-2026-09-27.md). Expanded Agent research
(A25–A48) and M8 documentation-based implementation are complete
([M8 results](../../validation/desktop-usage/m8-second-batch.md)): 18 registered adapters,
originally 17 parsers plus a Qoder probe; source-derived CLI reading now extends Qoder. Source inspection found no local per-call token records for
Amazon Q/Codebuff; iFlow has shut down. They are excluded under the local-source boundary.
The tables distinguish implementation candidates from verified support.

Initial implementation inspected local data read-only. Later real requests/container acceptance
followed their specific authorizations. Merge the user's duplicate Codex listing, but identify
CLI, desktop and IDE interfaces separately. The supplied website identifies “Harness Agent”
as Hermes Agent. Retain local-support plans for all tools; IDEs without verified local formats
remain F1. Authorized local-installation checks on 2026-10-07 removed candidates without
testable installations/files from this round. Junie CLI and built-in Zed moved to M8.
Zed external-provider samples are in [the current record](../../validation/desktop-usage/plan-20261007.md).
Real local Agent data may be extracted for implementation validation under
[readiness requirements](implementation-readiness.md). Enterprise APIs, account reports and
remote logs cannot substitute for local sources.

<a id="如何解释支持"></a>

## Interpreting support

- **Local candidate**: official material/source establishes storage/fields; a version-bound
  redacted sample is still required.
- **Enable first**: official telemetry/export exists; enabling usually cannot recover earlier history.
- **Conditional**: requires a particular local version, logging switch or local session export;
  default installations are not guaranteed to work.
- **Unverified**: only prototype parsing/product descriptions exist, without verifiable local
  field/format references.
- **Deferred F1**: user-deferred IDE products/variants, rather than a technical conclusion of
  unsupported behavior; excluded from first-release mandatory acceptance.

Implementation states use supported / partial / unsupported_format / not_found / disabled /
error, separately showing tokens, cache read/write, per-call identity, model, time, cost and
latency capabilities. File existence differs from format support; unreadable files produce
no all-zero events. Unknown/missing versions try the latest reader under
[compatibility policy](architecture.md#unknown-version). Validated data can enter statistics
with internal compatibility/reader references and visible coverage gaps. A valid, complete
expected structure needs no compatibility warning. Unregistered
versions alone cannot become unsupported_format; successful compatibility reading does not
verify each version.

The exact Codex registry contains 26 legacy and 11 modern versions. Version 0.3.2 adds
four independently checked modern versions and 27 Kilo session versions, each backed by
native redacted samples and calculated expectations; see [version checks and recovery](../../validation/desktop-usage/release-032.md).
The table's original 21 legacy Codex mappings remain historical evidence;
see [0.3.0 samples](../../validation/desktop-usage/release-030.md).
Other versions retain compatibility checks without inheriting these results.

Codex streams complete oversized `response_item`/`world_state` records through a
separate 64 MiB limit, validates a JSON object and UTF-8, and retains only its record type.
Each such record ends the current scan window. Usage, unknown types, malformed or
incomplete records and positional arrays retain the ordinary 8 MiB limit and held cursor. Parser 8 rechecks
old cursors without deleting history; [native recovery](../../validation/desktop-usage/release-033.md#oversized-codex-records)
does not certify every field of the new client version.

<a id="用户指定工具"></a>

## User-specified tools

Reference IDs link to [official/source references](research.md#agents). Fields below describe
observed upstream capabilities or candidate prototype mappings.

| Tool/interface | Proposed source and available information | Limits and next checks | Stage/reference |
| --- | --- | --- | --- |
| Claude Code | Native projects JSONL: 2.1.197 mirror checked against official integrity; two real Debian/Podman main-loop calls to two Zhipu models have matching positive API/CLI/native input/output. [Samples](../../validation/desktop-usage/claude-container-sample.md). | Four content blocks deduplicate by message.id into two calls. Bind per-record version; default-zero cache/complete totals unknown. No native channel: do not infer provider/cost. Complete old summaries/unchanged positions correct automatically. Anthropic models, positive cache, auxiliary/subagent/retry and other versions need separate checks; do not add OTel. | M2/M5 implemented; 2.1.197 compatible-endpoint samples passed; A01. |
| Cline | Separate legacy UI documentation-based reader and VS Code SDK schema 1. Official 4.1.22 VSIX/real GUI/local API verified three native metrics records; [executable recheck](../../validation/desktop-usage/cline-container-sample.md). | SDK inputTokens includes cache; positive buckets reported, default zeros unknown; use message modelInfo/ts. Metrics may merge runs/retries: observation, unknown calls. Rewritable origin.version remains latest_fallback. Read native messages without manifest/DB addition. Legacy UI four-bucket/deletion/subagent/compaction rules remain separate, without real samples. CLI/desktop SDK, migration/other providers require checks; empty registry DB verifies nothing. | M3 implemented; SDK real samples passed; other interfaces separate; A03. |
| CodeBuddy Code / IDE / extensions | Extension storage implemented, real local PASS 2026-09-30: `%LOCALAPPDATA%\CodeBuddyExtension\Data\...\history\<session>\<conversation>\index.json` requests[].usage is request-level aggregation, inputTokens=cacheTokens+cachedMissTokens; extra.modelId identifies models. [Results](../../validation/desktop-usage/m4-buddy-local.md). Official CLI `~/.codebuddy/projects` JSONL; third-party parser/tests show message.usage/providerData.rawUsage per-call tokens with input_tokens including cache read. Official monitoring provides opt-in OTLP/HTTP protobuf model_stream. | Extension usage aggregates requests, rather than individual LLM calls. All-zero requests without model calls are excluded. credit/lastTokens semantics unverified. Copies across profile/workspace trees deduplicate by request ID. CLI native samples still absent; local files/OTLP require source selection to avoid overlap. | Extension M5 implemented/real checks; registered CLI documentation-based reader awaits samples; A04. |
| Codex CLI / desktop / IDE | Prototype reads CODEX_HOME rollout token_usage_record; official OTel has requests, response-completed usage and streaming events. | Exact-version redacted samples: 0.155.0-alpha.16.3 (M0/M2-A), 0.154.0-alpha.6.1/6.2 and 0.153.0 (M2-D); per-call/cumulative/turn_context records, with compaction resets. Dedicated rollout_legacy supports 21 exact 0.139–0.151 mappings via token_count increments; see m2d limits. Unknown newer versions latest_fallback. Prototype fixed paths require discovery/configuration. Keep legacy/new formats separate; structured context identifies models and missing response IDs must not collide. | M2/M5; local candidate/opt-in telemetry; A02 + prototype. |
| GitHub Copilot CLI | Implemented/real PASS 2026-09-29: session-store.db assistant_usage_events, schema_version=8, 36 real redacted rows/expectations. COPILOT_OTEL_FILE_EXPORTER_PATH JSON-lines lacks documented row schema; OTLP defaults http/json with chat gen_ai.usage.*, cache/TTFT. Official warning excludes additive invoke_agent totals. 2026-09-30: newer `~/.copilot/session-store.db` is chronicle/search (sessions/turns/checkpoints/search_index), without assistant_usage_events. Discovery supports COPILOT_HOME; chronicle fingerprint gives UnsupportedVersion rather than wrong-product error. VS Code globalStorage chronicle also lacks usage; its turns are text. copilot-chat reads VS Code usage separately. | assistant_usage_events verifies the older primary format. Newer CLI removed it; checked related copilot-agent session-state events.jsonl lacks per-call tokens and copilot-user-cache.json premium_interactions is account quota. Current checked CLI has no verified local per-call format; await real newer samples or enabled OTel. File-export schema still needs native samples; otel tolerant parsing is documentation-based. Premium multipliers/nano AIU are not tokens. The 2026-09-25 disappearance was an upgrade migration and recovery was completed. | Older schema 8 M5 implemented/real; newer layout recognized without importing unverified usage. New samples/OTel acceptance deferred; OTel M5 documentation-based; A05. |
| JetBrains built-in AI Assistant / Junie | IDE plugin local schema unverified/F1. Junie CLI moved to M8 using `~/.junie/sessions/<id>/events.jsonl` modelUsage input/output/cache/reasoning/cost/time/provider; timestampMs is completion time. | A07 establishes excluded remote enterprise analytics only. Junie CLI A36 is no longer awaiting format references; AI Assistant IDE token/cache schema remains unverified, without current discovery/development. | IDE F1; Junie CLI M8; A07/A36. |
| DeepSeek Harness (DSH) | Official npm 0.2.0-rc.2, two local CLI/resume rounds, native v4 JSONL/zstd and API checked; new Windows/Linux executable reads passed. | Positive input is uncached. Recover cache-inclusive input only from that row's pi-ai openai-completions/version 2 positive input. Output includes reasoning; computed totals are not source totals. Select settlement/stream, separate retries, exclude inherited prefixes. Native row time has observation basis. Unwritten 205-token title is not invented. Old doc1 stays separate. | M3 current main loop complete; other protocols/real seed/retry/failure require checks. [Rules](m3-runtime-samples.md), A08. |
| Hermes Agent (originally “Harness Agent”) | M3 implemented; official 0.21.5/v2026.9.24 image local request/resume checked. Schema 30 does not verify other historical versions. | session_model_usage is interval-cumulative uncached/cache-read/cache-write/output/reasoning. Default zeros unknown; api_call_count does not expand into calls. Reevaluate complete old summaries/positions. Model/cost dimensions and detailed logs need separate checks. | M3 local samples checked; A24. |
| OpenClaw | Schema 24 read-only reader implemented M3, 2026-10-06, with official npm 2026.9.8 CLI/resumed local-model samples. | transcript_events TEXT/zstd; verified local provenance, external/migrated inputs isolated. Input uncached; default zero, computed totals/cost and underlying calls unknown. Database-wide version does not verify historical rows. Cold-archive gaps visible; other protocols/schemas separate. | Eleven real CLI checks, original-DB upgrade and Windows executable double-read passed. A09; [rules](openclaw-runtime.md), [acceptance](../../validation/desktop-usage/openclaw-container-sample.md). |
| Gemini CLI | Official `~/.gemini/tmp/<project_hash>/chats/` token storage; OTel input/output/thought/cache/tool and request metrics. | Probe actual JSON/JSONL versions. Verify thought/cache/tool inclusion per provider. Gemini web/Code Assist are separate products. | M2/M5; local candidate/opt-in telemetry; A10. |
| Kilo Code CLI / desktop / IDE extensions | Implemented M3, 2026-09-25: read-only SQLite/staging-copy rules; five exclusive per-message data.tokens buckets, incremental positions. Session token columns reconcile only (275/276 matched), with unstable version semantics. Independent real sums match 13,342 calls/1.65B tokens, without repeat additions. | 30 exact session versions verified, including 7.8.3/7.8.7; [0.3.2 samples](../../validation/desktop-usage/release-032.md) add 27 complete sessions/787 calls. Bind each message to its owning session.version; unlisted/missing versions retain latest_fallback. New core session_message has zero rows; switching needs a dedicated verified reader. IDE extension F1; CLI acceptance does not verify it. | CLI/desktop M3 implemented; unverified IDE format F1; A11. |
| New Kimi Code | Implemented M4, 2026-09-25, with shared kimi_wire.rs and protocol_version=1.5. Real 13 files/965 calls/116,428,813 total tokens match jq per field and repeats. Interrupted-step echo gaps appear as echo_subset. | usage.record lacks stable ID, so provider/per-call latency remain unavailable. Protocol 1.4 is registered after independent native verification; other unlisted versions use latest_fallback. The official 2.3.1 wire manifest at commit 684bae9ea6a437f3bae98bba1dcb797a917d500b identifies durable state/control records that create no model calls; goal/cumulative fields are not expanded into calls, and unlisted types retain diagnostics. | M4 implemented; cross-version verification pending; A12. |
| Kimi Work | Implemented M4, 2026-09-25: independent A13 roots, conv-*/ctitle-* with daimon host, 1.4 reference. Real 69 files/1,337 calls/125,225,933 tokens match per field; 69/69 reconciliation matched. Same-millisecond swarm keys include session:agent identity. | No official default-layout/environment documentation found; migration requires manual roots. No native subagent.completed/positive cache-write samples. Share verified wire parsing with Kimi Code while reporting separately. | M4 implemented; A13. |
| MiMo Code | Official 0.1.15 CLI/resume eight step-finish records match API individually. New Windows/Linux executable upgrade reads original SQLite/WAL. | Invert its SDK-normalized input/output/cache/reasoning; zero cache/reasoning/cost unknown; no message-copy addition. Owning-session versions remain per-record latest_fallback. MIMOCODE_HOME/data and MIMOCODE_DB precedence independently checked. Eight length finishes do not establish task success. | M3 current main loop complete; other versions/protocols/nontruncated tasks separate. [Rules](m3-runtime-samples.md), A14. |
| oh-my-pi | Official session documentation: `~/.omp/agent/sessions/...jsonl`, assistant usage/model/provider. | Native 18.2.7 has three redacted samples, 2026-09-25 ([resumed M2-B/C](../../validation/desktop-usage/m2bc-resumed.md)). All assistants have usage/duration/ttft in floating milliseconds. model_change uses combined model=provider/model, unlike Pi fields. Child files under `<ts>_<parentUUID>/` infer parents by verified path shape. .omp/logs has context-estimate debug lines, without per-call usage; title-generator overlap unverified. Native compaction/branch_summary lacks usage; auxiliary-format rules implemented, real samples pending. | M2 local candidate; A15. |
| Pi | Fixed source Usage input/output/cacheRead/cacheWrite/reasoning; session message, independent usage, compaction/branch-summary usage. | Reasoning included in output. Assistant-only reading misses auxiliary usage; inherited branches are not new calls. Resolve directories via version configuration functions. Native 0.87.1 sample/repeat checks passed 2026-09-25 ([record](../../validation/desktop-usage/m2bc-resumed.md)). Fork-copied entries carry fork-session identity; conflicts retain the first scanned contribution without net duplication. | M2 local candidate; A16. |
| OpenCode | Independent v1 part and v2 session_message readers. 1.18.34 isolated main-loop/cache samples match API/CLI/native DB/app. 2.0.32 Windows native projection: 7 completed assistants, independently checked with repeat reads and old unsupported-registration recovery. | v2 input is uncached; default-zero buckets/cost unknown. Compaction is an observation, fork-inherited history is excluded; no session/event-table addition. Two nonempty layouts remain unverified overlap. v1 default-title API gap (549 tokens) remains. | v1 real main-loop acceptance; v2 native storage only, Provider/API and real fork/compaction separate. [Rules](opencode-runtime.md), [0.3.3 record](../../validation/desktop-usage/release-033.md), [v1 container record](../../validation/desktop-usage/container-sources.md), A17. |
| Qwen Code | recording service usageMetadata/model. Real isolated 0.25.0 Podman main loop matches CLI/model service/SQLite without repeat additions. Real file export verifies main/background API logs and LLM spans, including background cache read. | Default automatic-memory calls are absent from native session per-call files; do not invent events from cumulative values. Files contain consecutive multiline SDK JSON, requiring the dedicated reader, rather than generic OTel JSONL. The 2026-10-06 reader selects verified host/user/session/local-day SDK coverage, retains native history and excludes sealed overlap. Goal is not added; active/archive deduplicates UUID. Unknown local model names receive no prices. Cloud/other versions/main-loop cache hits remain unverified. | M2 native main loop accepted; M5 0.25.0 SDK reader/selection implemented. [Container record](../../validation/desktop-usage/container-sources.md), [data rules](data-contract.md), A18. |
| Zoo Code | Official VSIX 3.86.0 isolated VS Code GUI/public API/local model; native array/callback/API match new Windows/Linux executable reads. | Complete ask/say enumeration. Positive tokensIn includes cache; default-zero buckets/cost unknown. Incomplete cache prevents uncached derivation. Do not invent model/client version from global configuration. Keep old finished LIFO/condense documentation path. Real task was actively cancelled, rather than completed. | M3 current extension main calls complete; CLI/deletion/compaction/other versions separate, JetBrains F1. [Rules](m3-runtime-samples.md), A19. |
| TRAE / TraeCode / extensions | Future local IDE/CLI logs, session databases, plugin data and local session exports. | Exclude A20 enterprise APIs/reports. 2026-09-29 inspection: tokscale TRAE uses a disk cache of remote usage API data, dollar_float/extra_info, also outside scope. Verifiable local per-call format still absent; defer the product family. | F1 future support; A20 establishes exclusions. |
| VS Code | Native chatSessions v3 last-call promptTokens input lower bound, whole-turn completionTokens, per-model modelTotals and stable-ID/time toolCallRounds main calls. Only github.copilot agent.id namespace. workspaceStorage/globalStorage/emptyWindowChatSessions share installation source. Real 10 turns/217 rounds, input lower bound 3,408,279/output 320,141 ([review](../../validation/desktop-usage/m9-copilot-review.md)). 2026-10-02 one-click file setup: 30 native CLIENT chat spans accepted ([record](../../validation/desktop-usage/dashboard-repair.md)). | Replay complete snapshots. Turn/model totals are observations; rounds count calls with unknown tokens. modelTotals replaces old usage; array reordering/copies/streams do not duplicate. Last-call input plus whole-turn output cannot derive complete total; cache unknown by default. Auto-discover app-managed OTel files, select OTel per original host/user/session/local day, retain native history/pre-enable gaps and deduplicate trace+span export copies. | M9 native and VS Code file real local acceptance. CLI/JetBrains OTel remains documentation-based, requiring separate acceptance; A06. |
| GitHub Copilot for Visual Studio | Verified VS 18 automatic `Path.GetTempPath()/VSGitHubCopilotLogs/traces/*.jsonl`, OTLP JSON service.name=vs-copilot. CLIENT chat spans report per-request input/output/cache_read, model/session/nanosecond time. Discovery checks TMP/TEMP/default user temp without Community/Professional/Enterprise/year filters. Official vswhere locates installations. Checked VS 2022 component 17.14.1713.63837 lacks the VS 18 JSONL exporter. Shared SDK/installed version does not verify other old formats ([checks](../../validation/desktop-usage/m9-vs-copilot-discovery.md)). MessagePack sessions/context limits/quota cannot replace tokens. | vs-copilot skips invoke_agent totals; temporary cleanup/rotation prevents complete-history promises. Select it or same-source otel. Parallel buckets do not establish uncached input. Selected-key privacy protection, unknowns and old cursors retained. [Native sample](../../validation/desktop-usage/m9-vs-copilot-local.md) and current four-call independent/repeat checks passed. | M9 implemented; real usage limited to tested VS 18 Community records. Other SKUs/VS 2022 keep separate static/native limits; A49. |
| GitHub Copilot for JetBrains | Plugin 1.18.0-261 inspected 2026-10-01 ([analysis](../../validation/desktop-usage/m9-jb-copilot-analysis.md)). Default Nitrite copilot-agent-sessions-nitrite.db has NtAgentSession turns/models/turnCreditsJson=RestoredTurnCredits(messageId,credits), without per-call tokens; credits are not converted. Logs use idea.log; remote App Insights excluded. Opt-in OTel settings otelEnabled/otelExporterType(file/otlp-http/otlp-grpc/console)/otelOutfile/otelServiceName/otelCaptureContent correspond to Agent debug File Logging and VS Code github.copilot.chat.otel.*. Debug Panel parses gen_ai.usage.{input,output,cache_read,cache_creation}* into five buckets; OTelSpanProvider bytecode checked. | No default-format token adapter. Existing M5 otel accepts a manual otelOutfile root after Settings → Tools → Copilot → Chat file export is enabled. Related row parsing is tolerant; no local JetBrains real samples yet. otelCaptureContent may contain prompts; read only selected keys. | Existing otel integration, requires enablement, documentation-based; A06 family extension. |
| ZCode | Implemented M4, 2026-09-25: model-io JSONL AI SDK five-key mapping primary (inputTokens includes cache read), mutually exclusive Anthropic snake_case comparison/contradiction diagnostics. Automatic primary is db.sqlite model_usage; turn_usage reconciliation only, with live mismatches visible even for completed or cancelled turns. Native repeat check passed. | Verified 3.14.3 real sample; other versions latest_fallback. Unverified `~/.zcode/v2` and `%APPDATA%/zcode` desktop layout not implemented. Missing requestId/traceId cannot collapse into zcode:None. | M4 implemented; other versions need checks; A21. |
| WorkBuddy | Third-party open-source parser identifies per-call fields in `~/.workbuddy/projects/**/*.jsonl`. Real eight files/420 events read-only/repeat passed ([record](../../validation/desktop-usage/m4-buddy-local.md)). | Independent source, without assuming CodeBuddy CLI OTLP. traces overlap with session detail unverified, so not added. Plan/account credits excluded. | M4 implemented/native format checked; cross-version/trace pending; A22. |
| Zed | threads.db JSON/zstd DbThread cumulative_token_usage is thread aggregate; request_token_usage is last-request turn buckets, reconciliation only. | Native 1.22.0/DbThread 0.3.0 llm-usage-zhipu calls to two models: uncached input, positive reported buckets, default zeros unknown. No derived complete totals/underlying calls/model detail. Historical hosted mapping separate; imported skipped and ACP reads underlying sources. | M8 nonempty native/executable reads; old hosted still source/schema only. [Record](../../validation/desktop-usage/plan-20261007.md), A23/A38. |

Unfound documentation is never described as proof that a product cannot support collection.
Tools without reliable usage may still show status/limits, without numeric total rows.

<a id="扩展覆盖"></a>

## Expanded coverage

Additional popular Agents use A25–A48 [research references](research.md#agents). M8
documentation-based implementation is complete ([record](../../validation/desktop-usage/m8-second-batch.md)):
independent adapter directories/version registries/V30 layout checks and 19 synthetic
tests/m8_contract.rs checks. Later real containers/product checks appear in
[M8 samples](../../validation/desktop-usage/m8-container-samples.md); others still need verification.
Closed-source field references come from third-party readers/reverse engineering and gain
verification after authorized real redacted samples. Reject inferred Kiro Auto zeros,
Grok cumulative/compaction differences and Goose reasoning differences; use native counts only.

| Tool/interface | Proposed source and available information | Limits and next checks | Stage/reference |
| --- | --- | --- | --- |
| Amp (Sourcegraph, closed source) | `~/.local/share/amp/threads/T-*.json` messages[].usage model/inputTokens/outputTokens/cacheRead/cacheCreation/credits, plus usageLedger.events timestamp/model/credits/five token buckets. | Reconcile ledger/message usage by messageId+buckets to prevent duplication. Without explicit time, thread creation/messageId cannot invent per-call time. Credits are not dollars. Version-changing schema requires samples. | M8 documentation-based, 2026-09-29; ledger primary, messages reconciliation only, no invented times/credit mapping; A28. |
| Goose (Block, repository moved to aaif-goose) | sessions.db usage_ledger per-request five buckets; legacy sessions accumulated_* aggregate fallback. GOOSE_PATH_ROOT/platform roots. | Select one format; NULL cannot be overwritten by cumulative default zero. Ignore reasoning differences. Native 1.53.0 CLI/local-model API/DB/app match. | M8 implemented; no estimated/carried_forward cost mapping. Format reference does not verify product version; [samples](../../validation/desktop-usage/m8-container-samples.md), A26. |
| Crush (Charm) | `~/.local/share/crush/projects.json` maps per-project data_dir/crush.db. Root-session cost cumulative; token columns are context snapshots. | Real 0.97.1 main/title API calls match native/CLI Estimated cost. Manual acceptance rates do not verify model prices/bills. Tokens/calls/model remain unknown. | M8 cost-only; root filtering avoids duplicate child costs. [Samples/executable status](../../validation/desktop-usage/m8-container-samples.md), A27. |
| Roo Code | VS Code globalStorage `rooveterinaryinc.roo-cline/tasks/<uuid>/ui_messages.json`; official 3.54.0 VSIX/real extension host/local API checked. | Positive tokensIn includes cache; default-zero four buckets/estimates unknown. OpenAI-compatible cache details lost: zero cannot establish uncached input. Cancellation leaves three API calls/two native records, preserved gap; one-call comparison separate. No invented per-call model. | M8 implemented; old-summary/unchanged-cursor corrections follow data rules. [Samples](../../validation/desktop-usage/m8-container-samples.md); CLI/other providers/subagents separate; A29. |
| Aider | Default history lacks per-call usage; explicit --analytics-log local JSONL message_send contains tokens/cost. | Enable first without backfill. Real 0.86.2 local-model API/CLI/app match. No cache/reasoning components; no tokenizer estimates. | M8 implemented, manual root; cost is LiteLLM Estimated. Format reference does not verify product version. [Samples](../../validation/desktop-usage/m8-container-samples.md), A30. |
| Continue (CLI/VS Code/JetBrains) | `~/.continue/sessions` CLI top-level cumulative usage; exclude indexes/estimate logs. | Real 1.5.47 streaming input/output matches local-model API. Default-zero cache unknown. Calls/model/complete total/interval start absent; no invented day ownership. | M8 implemented; complete old summaries permit cache-zero correction only, keeping other conflicts/history. GUI has no usage fields. [Samples](../../validation/desktop-usage/m8-container-samples.md), A31. |
| Droid (Factory.ai, closed source) | `~/.factory/sessions/{uuid}.settings.json` cumulative tokenUsage input/output/cacheRead/cacheCreation/thinking, plus same-name JSONL transcript. | Retain cumulative interval semantics; turn allocation is an estimate, not per-call usage. No cost fields. providerLock/transcript time attribution needs samples. | M8 session aggregates; reject transcript-byte allocation; A32. |
| Amazon Q Developer CLI | Timestamped JSON history under `~/.aws/amazonq/history/` from third-party references; official open-source repository. | History usage fields require source inspection first. SSO re-login may lose history; show gaps. | No token adapter: 2026-09-29 source shows tokenUsage lost in type conversion and absent from conversations/history; A33. |
| Grok Build (xAI, closed source) | `~/.grok/sessions/<workspace>/<session>/` updates.jsonl/signals.json/summary.json/events.jsonl, plus `~/.grok/logs/unified.jsonl`. | Explicit five-bucket usage only. Cumulative totalTokens/compaction differences are rejected inference. PID reuse/subagent ownership complex; conflicting model references stay unknown. GROK_HOME. | M8 explicit updates.jsonl usage only, without cumulative compensation/PID attribution; A34. |
| Antigravity CLI/extensions (Google) | `~/.gemini/antigravity[-cli]/conversations/<uuid>.db` gen_metadata protobuf per-turn usage; input=fixed system prompt+new input, cacheRead/output/thinking/responseId. | Reverse-engineered layout and changed 1.1.18 time fields require version-specific references. IDE language-server usage needs separate opt-in verification. Shared Gemini root, different subdirectories. | M8 reverse-engineered input=#1+#2/time=#9.#4; 1.1.18+ untimed rows rejected. Unverified IDE language-server format unimplemented; A35. |
| Junie CLI (JetBrains) | `JUNIE_HOME/sessions` or `~/.junie/sessions/<session-id>/events.jsonl` modelUsage. Official 26.9.22/seven real failed-task calls match API/CLI. | inputTokens uncached; default-zero tokens/cost/duration unknown, positive cost Estimated. No API/provider/product version; no invented complete total. timestampMs completion; positive time alone supplies start. | M8 implemented; doc1→2 complete old-summary/unchanged-cursor correction, transactions/conflicts passed. [Executable/sample status](../../validation/desktop-usage/m8-container-samples.md), A36. |
| Kiro (AWS CLI/IDE) | Three formats: CLI `~/.kiro/sessions/cli/*.json(+jsonl)`, kiro-cli `~/.local/share/kiro-cli/data.sqlite3` conversations_v2, IDE globalStorage kiro.kiroagent .chat/execution/promptLogs. | Auto agent often zero; explicit counts only, without inferred usage. execution/.chat deduplicated by executionId. Metering credits separate. Cross-format deduplication required. | M8 real CLI turns/kiro-cli SQLite request_metadata; Auto zeros/estimates rejected, IDE estimate format unimplemented; overlapping formats need real reconciliation; A37. |
| Built-in Zed (moved from F1) | Thread cumulative four buckets in threads.db; turn buckets do not verify complete per-call requests. | Keep original zed.dev source rules separate; llm-usage-zhipu/DbThread 0.3.0 positive/default-zero/cache semantics verified. Imported/other providers unverified; bounded decompression. | M8 native 1.22.0/76659a55, three glm-5.3/glm-5.3-flash turns: 21,696 uncached input/10,816 cache read/61 output, unknown cache write/total. [Record](../../validation/desktop-usage/plan-20261007.md), A23/A38. |
| Codebuff (formerly Manicode) | `~/.config/manicode*/projects/*/chats/<chatId>/chat-messages.json`; suggested CODEBUFF_DATA_DIR override. | Usage/channel roots manicode/-dev/-staging need samples. | No token adapter: official caec5fc source 2026-09-29 has local credits without tokens; CODEBUFF_DATA_DIR is not official; A39. |
| Command Code | `~/.commandcode/projects/<slug>/*.jsonl` v3 tree session/message/model_change, assistant five usage buckets/costUsd; config.json. | Exclude orphan rewind branches, deduplicate fork copies by ID/time, skip checkpoints; all-zero usage is reported zero under this verified format. | M8 npm 1.69.0 distribution reference, cache-inclusive inputTokens, current directory rules, cross-file fork ID/time identity, reported all-zero usage; A40. |
| jcode (jcode.sh, Rust) | `~/.jcode/sessions/session_*.json` plus append-only .journal.jsonl, journal superseding snapshot. token_usage input/output/optional cache. | Normalize API-specific OpenAI/Anthropic cache inclusion per version; JCODE_HOME. Environment snapshot version does not verify all messages. | M8; real 0.91.0 single offline run matches API/CLI/snapshot input 460/output 2/cache read 0. Custom-provider cache inclusion unverified; total/cost/reasoning unknown. Journal/other scenarios separate. [Samples](../../validation/desktop-usage/m8-container-samples.md), A41. |
| gajae-code (gjc) | `~/.gjc/agent/sessions/<slug>/*.jsonl` session v5, assistant model/provider/usage/Estimated cost. | GJC_CODING_AGENT_DIR overrides. OpenAI-completions missing-field zero cannot establish cache/reported-zero usage. message.timestamp is request start. | M8 real 0.18.7 API/CLI/native 412/2/414; recognize nonusage configuration chains, unknown cache/uncached and complete old-summary/cursor upgrade. Other APIs/versions/branches separate. [Samples](../../validation/desktop-usage/m8-container-samples.md), A42. |
| Xum (Coder, formerly mux) | .xum/.mux/sessions, XUM_ROOT/MUX_ROOT or RUN_SESSION_ROOT session-usage.json v1. Official 0.30.0 source/two native local calls checked. | Input uncached, default zeros unknown. Positive text output + known reasoning; unknown reasoning gives lower bound. Complete input/total unknown. CLI temporary-session deletion/default custom-provider missing streaming usage distinguished from gateway comparison. | M8, without invented cost/per-call identity. Complete old-summary/cursor/rollback/conflict regressions passed. [Samples/executable status](../../validation/desktop-usage/m8-container-samples.md), A43. |
| iFlow CLI (Alibaba, closed source) | Official 4642808 announcement: service ended 2026-04-17. Default local per-call format unverified; opt-in OTel separately checked under M5. | Do not borrow Gemini-family formats; excluded as a current M8 local candidate. | Unimplemented, consistent with AGENTS/M8; A46. |
| Qoder CLI / IDE / JetBrains plugin | CLI session JSONL/state.json; IDE/plugin transcript paths and SDK usage fields have separate official references below. | Verify persisted token semantics, versions and local ownership for each interface; SDK fields do not establish native storage. | CLI 1.1.76 source-derived transcript reader with synthetic tests; real-client acceptance and IDE/plugin token formats remain pending. All three are requested support targets; see [Qoder requirements](#qoder-local-support). A47. |
| AtomCode (AtomGit ecosystem) | .meta turn_stats/model_usage per-model session aggregate; round_count source call summary; legacy single-file equivalent layout. | Real 5.2.1 API/CLI/.meta input 6,176/output 2. Default-zero cache/invalid buckets unknown; no guessed uncached input. Exclude only complete paired UI v1/rewind v2, diagnose other files. | M8 official e4215f7/45e05cb/native samples, old-summary/cursor/parallel/rollback passed. Product version does not verify other records; InsCode IDE F1. [Samples](../../validation/desktop-usage/m8-container-samples.md), A48. |
| Warp | Local account/workspace usage cache requestsUsed/spendCents/syncedAt only. | No token detail, account quota excluded from token totals; at most show limited status. | Deferred quota category; A44. |
| Cursor CLI/IDE | CLI `~/.cursor/projects/<slug>/agent-transcripts/<uuid>/*.jsonl` has sessions without per-call tokens/models. IDE state.vscdb format unverified. | Per-call usage found only in remote dashboard get-filtered-usage-events/CSV and excluded. Do not estimate with a tokenizer. | Local-format F1; A45. |
| Windsurf IDE/CLI | IDE Cascade/CLI confirmed in 2026; local usage storage unverified. | No discovery/implementation without verified per-call format; retain future F1 support. | F1 future support. |

<a id="qoder-local-support"></a>

## Local Qoder support requirements

The requested scope includes CLI, IDE and JetBrains. The 2026-10-22 read-only local
check found no CLI executable, default `.qoder`/`.qoder-cn` directory or Qoder entry
in the inspected Windows application directories. The user confirmed no local installation and authorized source-derived synthetic data.
No client was initialized and no login, hooks or model requests were added.

| Interface | Current official reference | Required before importing usage |
| --- | --- | --- |
| CLI | [Sessions](https://docs.qoder.com/zh/cli/sessions) documents project JSONL/state.json and `QODER_CONFIG_DIR`. [SDK assistant messages](https://docs.qoder.com/cli/sdk/references-typescript#sdkassistantmessage) declare optional input/output/cache-read/cache-creation tokens. | Version-bound native persistence, input/cache inclusion, default-zero validity, model/time/request identity, resume/fork/compaction and failed-call coverage. Do not apply SDK field names to state.json without checking the writer and samples. |
| IDE | [Hooks/transcripts](https://docs.qoder.com/extensions/hooks#transcript-file-format) documents `~/.qoder/projects/<project>/transcript/<session-id>.jsonl`, with session metadata, user/assistant/progress records. The published transcript examples do not specify token usage. | Inspect the installed IDE writer and a nonempty local transcript/database; establish usage and per-call identity independently of assistant-message counts. Hooks are unnecessary for reading existing files and are not enabled automatically. |
| JetBrains plugin | The same [hooks documentation](https://docs.qoder.com/extensions/hooks) explicitly covers the IDE and JetBrains, while noting that CLI hooks differ. | Check plugin/version-specific paths, persistence and usage fields; the shared hooks documentation does not verify that IDE transcript examples or CLI usage fields apply to the plugin. |

[Credits](https://docs.qoder.com/cli/sdk/cost-usage) are distinct from tokens and currency.
Request Credits, cumulative session totals and account quotas are separate scopes;
do not add them or convert them into tokens. Remote account/usage queries remain
outside collection. Context-window occupancy is not consumed token usage. Missing
fields remain unknown; model family names and subscription plans do not identify prices.
The independent Qoder desktop product stores history separately from the IDE, according
to its [product overview](https://docs.qoder.com/qoder/overview); imported history needs
explicit duplicate handling before any later support extension.

Keep one Agent directory with independent version/format readers and interface
attribution. Share only proven field mappings; one physical file has one reader.
Each interface needs writer-bound fixtures and independent sums, real registry
discovery, repeat reads, old-cursor recovery, malformed/partial records, transaction
rollback and cross-interface duplication tests. The authorized CLI source fixtures
satisfy implementation regression; real-client acceptance is recorded separately.
Unverified formats retain the probe and diagnostics without importing guessed values.
An incompatible upgrade preserves earlier statistics and falls back to detection only.

### Source-derived CLI reader

The user authorized source analysis and synthetic data instead of local installation.
Official npm `@qoder-ai/qodercli` 1.1.76 and SDK 1.0.55 were downloaded without running
installation scripts. The CLI tarball's verified npm SHA-512 is
`N4GLhnDoCOUBL8Ei5JY53u57/hQcXgu+W83mXwGJraSfohwmldnQnsb+/vKOu6ysl228+UNcOqFQe3abybFmGw==`.
The following requirements permit a source-derived CLI reader with synthetic regression
data; real-client acceptance remains pending. IDE and JetBrains formats require their
own writer checks and cannot inherit this mapping from their CLI integration.

| Requirement | CLI 1.1.76 writer/normalization basis |
| --- | --- |
| Native file | `Mm` chooses `projects/<project>/<session>.jsonl`; `KLo` and `SessionTranscriptWriter.insertMessageChain` write per-record `sessionId`, `version`, `entrypoint`, `userType`, UUID and ISO timestamp. Require the CLI structure plus a custom-model or native Credits marker in the same file; official responses need no modelSource field. Indistinguishable chat-only transcripts retain the probe, rather than accepting every sessionId-shaped JSON file. |
| Counted response | Read completed assistant `message.id`/model/stop_reason; skip buffered partial responses, `<synthetic>` API/error messages and copied records with valid `forkedFrom`. Message ID deduplicates multiple content records and copies; no user/tool/system or `token-stats` call counts. |
| Token fields | `cU` maps inclusive `prompt_tokens` to native `input_tokens`. `uot` adds Anthropic uncached/read/write before that conversion. Positive input is inclusive; positive output/cache-read/cache-creation remain independent reported fields. `za` and `A$n` insert default zeros, which stay unknown. Derive uncached input only with both cache components known; derive total only with complete input/output. |
| Redaction | `bxu`/`kU` preserve custom-model token counts, but ordinary official-channel serialization masks counters to zero. Such completed calls count with unknown tokens. Do not enable token logging or infer consumption from Credits. |
| Missing detail | `XVe` and `gpt` attach reasoning/source-total values as non-enumerable properties, lost by object spreading/JSON serialization. Do not recover them from source field names or zero defaults. Credits do not identify currency/provider. |
| Time/ownership | The native timestamp is the persistence observation time. Retain each row's version; `isSidechain`/agent ID identify subagent category without creating another call. Reject unsupported remote entrypoints and malformed ownership; do not infer a host application from the common CLI entrypoint. |
| Upgrade/failure | Route the complete CLI envelope to Qoder before shared transcript detectors; transfer an old misrouted file only without statistical history, preserving other invalid files and diagnostics. Replay the old probe's processing state without clearing statistics/history. Preserve record errors across windows, isolate malformed fields, retry partial lines, and verify repeated reads, conflicts and rollback. Unverified state.json, IDE/plugin records and other formats retain explicit diagnostics. |

The native marker probe examines complete lines within 32 MiB and 200 ms, with an 8 MiB
line cap and one retained line. It handles BOMs, long initial messages and metadata prefixes;
outside this bound, provenance remains unverified. Metadata-only windows advance without
certifying usage. A verified native envelope also permits progress past earlier invalid
records, whose diagnostics and degraded health remain. Filename substrings such as
subagents/segments do not establish auxiliary content and never suppress manual files.
Reader 2 rechecks consumed old probe/reader positions without clearing history or conflicts.

Routing and format detection run outside the statistics writer lock. The CLI marker probe
shares a process-local cache of at most 4,096 paths across manual routing, ownership claims
and parser windows. Reuse completed positive/negative decisions only when length, mtime,
creation time and both sampled fingerprints match; recheck after probing before caching.
Changed files are probed again. Interrupted, timed-out, unreadable or unstable probes do
not become cached negative evidence. Cache eviction/restart may repeat bounded detection;
neither caching nor lock separation relaxes old-owner history checks or corruption diagnostics.

<a id="hermes-agent-本地规则"></a>

<a id="hermes-agent-本地合同"></a>

## Local Hermes Agent rules

Fixed A24 `ef70b3661cbfcf57e583008ad91dd04d8ba46070` and official 0.21.5 release
`f97608f178d1ffeca59860195ab7da295f7c8e5f` establish the following. Real acceptance currently
covers the local CLI main loop and one resumed session, without extending to other versions,
historical records or gateway usage.

- get_hermes_home uses context override, HERMES_HOME, then platform default: Windows
  `%LOCALAPPDATA%/hermes`, macOS/Linux `~/.hermes`. Named profiles have separate directories/
  state.db. Probe/read allowed roots without importing/executing Hermes; Linux-only hardcoding
  misses other platforms.
- sessions holds cumulative usage. session_model_usage keys session/model/provider/base_url/
  billing_mode/task and includes input/output/cache_read/cache_write/reasoning, api_call_count
  and first_seen/last_seen. Normalize base_url in memory into a credential-free provider ID
  or local HMAC; do not persist full URLs/query parameters.
- update_token_counts has incremental and absolute paths; absolute cannot recover historical
  model routing. record_auxiliary_usage updates task aggregates without main sessions totals.
  Neither unconditional table selection nor table addition is valid. Unexplained differences
  remain unknown-model/coverage gaps, rather than assigned to the current session model.
- Schema v20 backfills historical sessions into model rows; v22 adds task to the key, with
  other source fixes for old keys. Probe actual columns/primary keys, rather than version
  integers alone. Backfilled rows do not establish one model for all historical calls.
- first_seen/last_seen are aggregate-write boundaries, rather than call timestamps. Keep
  cross-day cumulative intervals; without details, exact daily trends are unavailable.
  api_call_count remains a source aggregate and cannot produce synthetic per-call events.
- agent/turn_usage.py has response usage/logging with model/provider/ID. Actual log paths,
  rotation, timezone, stable IDs, failure coverage and cache defaults still require samples.
  Both fixed normalize_usage implementations were fully checked: input uncached/output includes
  reasoning. Default zero does not establish response-field presence. Retain positive buckets,
  unknown zeros and derive totals only with every required bucket known. Database-wide
  schema_version does not verify each historical client version; keep latest_fallback.
- Verify coverage of parent/child/compaction inheritance, auxiliary tasks, MoA and external
  Codex mirrors. Message/FTS text counts cannot fill calls. Read databases without invoking
  initialization APIs that might migrate/repair them.

The first deliverable may be native local model/task interval statistics, with separate
per-call/exact-day capabilities. Retain cost only for explicitly local calls; exclude Hermes
Portal balances/account_usage APIs.

<a id="分家族复用的范围"></a>

## Reuse within Agent families

Basic JSONL, mutable JSON, read-only SQLite, OTLP and CSV readers may be shared. Share business
mappings only after field/lifecycle tests establish equivalent semantics:

- Pi/oh-my-pi share some Usage knowledge; auxiliary events/branch restoration remain separately
  tested. Pi-derived gajae-code (gjc) likewise requires individual checks.
- OpenCode/Kilo/MiMo share structural probes, without unverified directories/tables/cumulative
  assumptions. Kilo's highest DB version identifies format only; bind each message to its
  owning session.version and preserve verified/compatible references. Unverified messages
  in mixed DBs keep notices despite later verified-only increments. Empty new-version sessions
  neither verify usage nor downgrade existing message references. Support/rule updates trigger
  reevaluation. OpenCode also uses the session owning each step-finish; empty sessions verify
  nothing. Replay old positions from the beginning on support/rule changes, paginating by
  (time_updated, part.id), then restore 60-second overlap after finishing the window. Equal
  milliseconds/overlap cannot stall progression. Only complete valid reevaluation marks rules
  updated; retain invalid/unverified state across increments. Events, continuation positions
  and rule progress commit together.
- Cline/Zoo/legacy Kilo extension API records may share components, with separately tested
  subagent/deletion/compaction summaries. Roo equivalent formats still require these version checks.
- VS Code/Copilot/embedded Claude/Codex uses origin relationships, rather than duplicate billing
  by application brand.
- Kimi Work/Code roots, instance identities and log revisions remain independent; resolve overlaps explicitly.
- Amp usageLedger/message usage and Kiro execution/.chat are same-source dual formats. Verify
  their reconciliation/deduplication separately; nearby times/equal values cannot establish identity.
- Goose/Xum/Droid session aggregates follow Hermes-like interval semantics, without synthetic calls.
- Antigravity/Gemini share ~/.gemini but have independent subdirectories/discovery. iFlow's
  apparent Gemini similarity is only a research lead; verify every field separately.

<a id="每个适配器的交付要求"></a>

<a id="每个适配器的交付合同"></a>

## Adapter delivery requirements

Follow [independent directories/version organization](architecture.md#adapter-layout): one
directory per Agent, historical implementations within it. This applies to every stage in
the matrix; directory migration/registries were implemented with M2. Record verified support
separately from unknown-version compatibility; proposed changes are not implemented work.

| Area | Required delivery |
| --- | --- |
| Directory/version | Independent Agent directory, common entry, release/format-to-reader mappings and per-version samples. New versions preserve old readers/regressions; V30 acceptance. |
| Discovery | Product/interface/OS/version range, local ownership, candidate paths, environment overrides, profiles/manual addition. |
| Detection | Identify Agent/input per file/database; dispatch known mappings, try latest Agent reader for unknown versions; explicit errors for semantic/structural incompatibility or ambiguous matches. |
| Field mapping | Every token's inclusion, units, time basis, model ownership, call ID and call-verification references. |
| Lifecycle | Per-call/cumulative/stream/final/correction/aggregate, retries, branches/subagents/auxiliary calls. |
| Incremental reading | Cursor, rewrite/rotation, transaction recovery, source retention and earliest backfillable date. |
| Deduplication | Copies, multiple databases, host mirrors, local telemetry/session-export links or primary-source selection. |
| Completeness | Success-only/hidden-call coverage, sampling/loss/log cleanup and missing-timestamp limits. |
| Validation | Original version references, minimal redacted samples, manually checked expectations, unit/integration and real-app results. Unknown-version tests cover unregistered compatible shapes, structural breakage and partial usability. |
| Maintenance | Source commit/schema, latest default reader, selection basis, compatibility state, upgrade retries/incompatibility diagnostics; unknown version alone does not reject. |
| Scheduling | Unified entry, incremental/rescan cost, pause/cancel, unchanged scans and recovery without launching Agents. |

Samples retain only event kind, anonymous IDs, time, model, numbers and necessary relationships;
replace conversation bodies with constants. Recompute digests after changes and document
redaction. Never commit complete real session databases.

<a id="各家族必须补的样本"></a>

## Required samples by family

| Family | Required cases |
| --- | --- |
| Claude/Codex | Multiple fragments per request, tool/user without usage, duplicate final, legacy cumulative/new per-call formats, model switch, subagents. |
| Pi/oh-my-pi | Message/independent usage, titles/compaction/branch summaries, fork inheritance, error/cancel, log auxiliary-event deduplication. |
| Cline/Zoo | Started/finished merge, updated same record, compaction/deleted-request summaries, simultaneous subagent summaries/details, absent model. |
| OpenCode/Kilo/MiMo | Live old-row updates, new core/legacy DB detection, cumulative-session/detail relationship, provider changes. |
| Gemini/Qwen | Prompt/cache/thought/tool inclusion, file rewrite, provider conversion, cumulative telemetry reset, repeated Goal recovery. |
| DSH | Stream → final replacement → same-step retry, old/new stateVersion, context estimates excluded, cumulative decreases. |
| OpenClaw | Concurrent SQLite writing, legacy JSONL/migrated DB overlap, external CLI mirrors, nested sessions, remote paths/nonpersistent sessions. |
| Hermes | Windows/profiles, model switch, incremental/absolute, auxiliary tasks, v20 backfill/v22 keys, cross-day cumulative, compaction/subagents/MoA, missing usage. |
| Kimi Code/Work/ZCode | Real versions, stable per-call IDs/sequences, model/provider, millisecond time, missing IDs, directory migration. |
| Copilot/VS Code/CodeBuddy | chat/model_stream parent deduplication, file/OTLP retries, absent cache/failed-call usage, SDK/host overlap, chatSessions/otel same-interface selection, sampled key-value snapshots/compaction replay without repeated keys, VS TEMP cleanup/rotation and receiver same-dimension selection. |
| JetBrains/TRAE/other unverified local sources | Native logs/DB samples, version/plugin differences, absent usage, same-interval reimports. Exclude remote synchronization/account reports. |
| M8 dual-format/reconciliation (Amp/Kiro) | Complete/partial ledger, execution coverage, duplicates, missing messageId, same request across formats. |
| M8 session aggregates (Goose/Xum/Droid) | Accumulated/per-call columns, cross-day sessions, snapshot/journal merge (jcode), provider-prefixed model keys, cumulative correction. |
| M8 reverse-engineered/closed-source (Antigravity/Grok/Junie/Qoder/iFlow) | Protobuf-version changes, cumulative/compaction differences, missing timestamps, completion-time semantics, brand migration roots. |
| M8 similar open-source formats (Roo/gjc/Command/jcode/Codebuff/Continue/Crush/Aider/Amazon Q) | Rewind/fork branches, deletion summaries, subagent replay, cache semantics, opt-in logs, SSO history loss. |

<a id="会话归档与历史可回采性"></a>

## Session archives and recoverable history

Source archives differ from application-layer retention. Discover verified archive formats
alongside active data; copies share native call identity. Without shared call IDs, choose an
explicit primary source. Nearby time, equal tokens or equal session IDs cannot establish requests.

| Source | Verified entry/behavior | Implementation/validation scope |
| --- | --- | --- |
| Codex | CODEX_HOME sessions/archived_sessions; official thread/archive moves JSONL, restoration moves it back. | Discover both, including archive-only roots; deduplicate copies/repeats. Historical local check: 306 active files/no real archive; official behavior plus synthetic move/copy tests verify archive handling. |
| Qwen Code | Old tmp/new projects chats/archive; daemon moves active JSONL in/out, with possible duplicate copies. | Both layouts discovered, deduplicated by native UUID/source. Verified QWEN_RUNTIME_DIR/QWEN_HOME/manual-root choices; Agent-working-directory-relative paths require absolute manual roots. Real 0.25.0 main loop checked; real archives/other versions separate. |
| ZCode | cli/db/db.sqlite model_usage retains completed calls; JSONL may be deleted/compacted. | DB primary, native ID/source-day replacement; JSONL only before any DB exists. Once used, missing/damaged DB errors and retains statistics without duplicate-producing fallback. Unmatched legacy JSONL remains a health warning; neither add nor erase it without reliable DB-call identity. |
| Claude Code | projects main/child/orphaned JSONL and superseded copies. | Recursive discovery/request-message deduplication; synthetic samples, without native archive acceptance at that historical check. |
| Kilo / OpenCode | SQLite session.time_archived marks archive; calls remain in message/part. | Queries include archived sessions, with archive/restore/rearchive tests. Kilo real redacted samples; OpenCode archive tests synthetic from fixed source. |
| Gemini CLI / Kimi Code / Kimi Work / Pi / oh-my-pi | Known session files; Gemini automatic saving differs from manual checkpoints. | Gemini default cleanup around 30 days deletes data rather than moving it to another archive. Manual checkpoints may copy sessions but lack separately verified per-call rules, so not added. Other roots read by format. omp usage_history is quota-window data, not call history. |
| Cline / Zoo / Hermes / DSH | Verified compaction/deletion/interval formats. | Follow capability matrix; aggregate values cannot reconstruct per-call detail. Real archive acceptance remains pending where samples are absent. |
| OpenClaw | Verified schema 24 hot transcripts separate from old migration/cold archives. | Positive buckets retained; default zero/underlying calls/complete totals unknown, cold gaps visible, old JSONL not added, protocols/versions separately checked. |

Installed ZCode recordModelUsage deletes records roughly 30 days before started_at. The DB
is not permanent complete history. Do not replace existing boundary-day statistics with a
potentially incomplete snapshot; source-expired days stay in application summaries. Newly
discovered sources contain only surviving data and cannot recover never-collected deleted
records. No per-row product version exists: successful structure checks retain latest_fallback,
without claiming exact-version verification.

At the 2026-09-27 real check: 8,225 calls/2,398,950,011 tokens; repeat scan added zero and left
revision unchanged. turn_usage had 24 reconciliation differences and remains comparison-only,
without addition to model_usage. Current files do not prove permanent retention; backups before
clearing application statistics remain useful.

References: [Codex App Server](https://learn.chatgpt.com/docs/app-server),
[OpenCode session source](https://github.com/anomalyco/opencode/blob/dev/packages/opencode/src/session/session.ts),
[Qwen archives](https://github.com/QwenLM/qwen-code/issues/6057),
[Qwen active/archive copies](https://github.com/QwenLM/qwen-code/issues/9688),
[Qwen environment paths](https://github.com/QwenLM/qwen-code/blob/main/docs/users/configuration/settings.md),
[Gemini session management](https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/session-management.md).
Installed-package fingerprints, read-only SQL, redacted results and test entries are in
[the review](../../validation/desktop-usage/review-2026-09-27.md).

<a id="暂未证实工具的推进方式"></a>

## Progress for unverified tools

M4 retains new Kimi Code, Kimi Work, ZCode and WorkBuddy. Initial scope deferred JetBrains/
TRAE, built-in Zed and unverified IDE variants to F1. After 2026-09-29 research, Junie CLI/
Zed threads.db moved to M8 with local-format references. Cursor, Windsurf, JD JoyCode, Zhipu
CodeGeeX, Baidu Comate and Huawei InsCode/CodeArts Snap local usage formats remain F1. Warp
account quota and remote Cursor/TRAE per-call usage are excluded without estimated replacements.
At each stage identify version, inspect local storage/export/telemetry, extract minimal redacted
samples and test field/lifecycle rules. Do not defer Cline/Roo/official VS Code telemetry solely
because they are IDEs with established local fields.

Record versions, checked official local paths/formats, authorization, read results, missing
fields, failures and next conditions. Without local versions/samples, use “Not attempted”/
“Awaiting samples”, rather than claiming verified unsupported behavior. Account-only quota
pages mean local usage is unavailable. Text without reliable usage does not enable default
tokenizer estimates. TLS interception, browser cookies, process injection and private billing
interfaces are not gap-filling methods. A future user-requested metering gateway needs a separate
design; it cannot recover earlier history or necessarily cover subscription channels, so is
not the current general fallback.

[M8 registration](../../validation/desktop-usage/m8-second-batch.md) includes 18 adapters,
originally 17 parsers and Qoder probe; the CLI transcript extension is recorded above. Amazon Q/Codebuff source confirms absent local per-call tokens;
iFlow ended 2026-04-17 and its identified usage route requires enabled OTel under M5. These
three have no token adapter. Cursor/Windsurf retain F1. Historical ten-source samples are in
[the container record](../../validation/desktop-usage/m8-container-samples.md); new Zed samples,
container account/protocol limits and F1 installation checks are in
[the current record](../../validation/desktop-usage/plan-20261007.md). Candidates without samples
in this environment were removed from active Plan.md work. Documentation-based implementation
does not verify untested versions/formats.
