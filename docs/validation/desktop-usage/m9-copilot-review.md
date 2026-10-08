# Review of uncommitted Copilot implementation (2026-10-01)

<a id="copilot-未提交实现审查2026-10-01"></a>

<a id="范围与方案"></a>

## Scope and approach

Review VS Code native sessions, Visual Studio OTLP telemetry, CLI new-layout detection,
account quotas, SQLite ingestion and overview display. Preserve existing uncommitted changes;
no Agents/model calls/remote usage APIs/commits/pushes.

| Confirmed issue | Correction and acceptance behavior |
| --- | --- |
| User turns/per-model totals counted as model_call | Store turn/modelTotals as usage_observation; count only toolCallRounds with stable IDs/valid times, limited to persisted main-loop calls |
| Last-call input plus whole-turn output derived a complete total | Retain reported input lower bound and source-cumulative output; default total_tokens unknown, with coverage notice |
| Old keys contributed after modelTotals; model reorder changed identity | Model keys exclude array indices; complete permitted-field snapshots use monotonic revisions, replace prior contributions/recompute daily summaries; corrections may reduce values |
| Snapshot updates conflicted in one lifecycle; truncated reads could publish stale prefixes | Revise after complete replay; partial lines/insufficient time or read allowance/bad lines/invalid tokens/duplicate model keys commit no new usage; source history cleanup/file disappearance retain consumed usage |
| Other extensions in generic chatSessions attributed to Copilot | Filter request agent.id by github.copilot namespace; absent ownership stays unknown |
| Empty-window paths missed; migration copies double-counted; manual file expanded to directory | Add globalStorage/emptyWindowChatSessions; workspace/empty-window roots of one installation share a source; manual files collect only that file |
| Detection checked limits only after reading first line | Read at most limit plus one byte; incomplete first line remains Pending |
| VS checked service only on first line; failed calls without usage lost | Check service per resourceSpans batch; chat CLIENT spans with trace/span identity and time count calls; absent tokens stay unknown; skip aggregate spans |
| Missing VS trace identities and doubleValue TTFT unhandled | Reject missing trace/span IDs; convert doubleValue seconds to milliseconds with upper-bound checks |
| Missing quota fields became zero; fractions rounded; refresh replaced snapshot times | Preserve unknowns; integer milli_requests stores thousandths of requests; source timestamp_utc controls day/deduplication; metadata correction at the same time may update |
| Quotas missed clearing/hard retention, disappeared without tokens, suffered stale responses/wrong timezone | Include clearing/hard retention; old caches cannot restore hard-expired snapshots; display with empty usage; invalidated responses cannot overwrite cards; per-source scheduling does not also collect account quotas; daily queries/date display use statistics timezone |

Snapshot-update counts do not establish call counts. Thinking/premium credits/account quotas/
model context limits are not converted into tokens. Per-model modelTotals in one turn remain
usage totals. Invalid cache components/token values produce diagnostics without replacing
negative values with zero or choosing the larger conflicting value.

VS Code toolCallRounds count a lower bound of observed main-loop calls; absent rounds do not
invent one call per turn. Full auxiliary/sub-Agent/retry/inline coverage remains unverified.
Last promptTokens may establish an input lower bound, rather than complete inputs of those
calls. Cumulative output depends on upstream deduplication/re-emission behavior.

<a id="依据"></a>

## References

- Read-only local source rechecks independently replay mutation logs/unpack OTLP in Python;
  output only product identities/field names/counts/token totals, excluding bodies. Temporary
  scripts/databases: build/copilot-review.
- VS Code [usage types](https://github.com/microsoft/vscode/blob/main/src/vs/workbench/contrib/chat/common/chatService/chatService.ts)
  distinguish last promptTokens from whole-turn modelTotals;
  [chatModel](https://github.com/microsoft/vscode/blob/main/src/vs/workbench/contrib/chat/common/model/chatModel.ts)
  accumulates completion usage; existing source snapshots were also reviewed.
- Copilot [toolCallingLoop](https://github.com/microsoft/vscode-copilot-chat/blob/main/src/extension/intents/node/toolCallingLoop.ts)
  saves rounds after runOne returns;
  [toolCallRound](https://github.com/microsoft/vscode-copilot-chat/blob/main/src/extension/prompt/common/toolCallRound.ts)
  defines timestamp as round start. main identifies this research snapshot, without establishing
  default rules for future releases.
- [OpenTelemetry token attributes](https://opentelemetry.io/docs/specs/semconv/registry/attributes/gen-ai/)
  define cache as an input subset/reasoning as an output subset. VS implementation containment
  remains unverified per version; generic conventions cannot derive VS uncached input.
- Baselines retain earlier local records: VS Code 1.140.0/Copilot Chat 0.68.0, VS 18.10.1197.
  This batch rechecks native data; running Code processes/standard extension paths did not
  supply new installed-version metadata.

<a id="验证进度"></a>

## Validation progress

Independent comparison: VS Code ten usage-bearing turns, 217 rounds with IDs/timestamps;
promptTokens lower-bound sum 3,408,279, cumulative completionTokens 320,141; no modelTotals
in these samples. Visual Studio: two chat spans, input=17,470/output=219/cache_read=13,184;
two invoke_agent spans excluded from calls/usage.

Full SQLite regressions cover multiple calls/decreasing final corrections/modelTotals replacement/
model reorder/copy/migration/other extensions/manual single files/partial-line continuation/
source history cleanup/failed calls without usage/quota hard retention. Unit cases cover reading
limits/null Set/invalid tokens/TTFT number representations/absent quota fields/times/fractions.
Browser checks cover quota units and visibility without token history.

Corrected real collection databases match independent expectations and daily_usage:

| Local source | Observed calls | Usage observations | Input | Output | Cache read | Complete total tokens |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| VS Code workspaceStorage, three logs | 217 | 10 | 3,408,279 (lower bound) | 320,141 (source cumulative) | Unknown | Unknown |
| Visual Studio traces, one log | 2 | 0 | 17,470 | 219 | 13,184 | Not reported |

All ten VS Code diagnostics describe input coverage, rather than corrupt lines. Both adapters
add zero on repeats with unchanged calls/usage. VS cache_write/reasoning remain None; validation
tools do not fill zero. Two native VS Code model identifiers remain separate source values,
without fuzzy merging by similar names. Current quota cache: limit 1500 requests, remaining
1500, used zero; real extraction/ingestion/read-back passed. Earlier remaining=137.4 serves
fractional regression only, without replacing the current snapshot.

Real validation from repository root, all exit 0:

- python build/copilot-review/audit.py: independent read-only native data, permitted-field totals.
- cargo run --quiet --locked --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core
  --example real_verify_copilot_chat -- &lt;workspaceStorage&gt; &lt;build/copilot-review/chat-real&gt;.
- cargo run --quiet --locked --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core
  --example real_verify_vs_copilot -- &lt;traces&gt; &lt;build/copilot-review/vs-real&gt;.
- python build/copilot-review/compare.py: real details/daily totals match independent expectations, PASS.
- cargo run --quiet --locked --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core
  --example copilot_quota_probe: quota read-back PASS; no paths/login/bodies output.

Added seven Rust unit/six SQLite integration cases plus browser quota-unit/empty-history/statistics-
timezone assertions. Final environment: local Windows, PowerShell 7.6.6, Node.js 24.21.0,
Cargo 1.98.1. Root commands all exit 0:

- npm run verify: Markdown/assets/scripts, Svelte zero errors/warnings, Rust fmt/Clippy,
  716 Rust tests and frontend production build.
- npm run test:browser: installed Edge interactions/new quota assertions.
- git diff --check: passed.

Ignored logs: build/copilot-review/verify-final.log, browser-final.log, real-compare.log.
These are local static/test/real persisted-data checks, without replacing three-platform
or actual IDE interaction acceptance.

<a id="保留的边界"></a>

## Retained limits

Native and OTel file/receiver records for one interface still require exclusive source selection
in the source UI. Shared Agent names do not deduplicate without verified cross-format call
association; do not add them. Automatic cross-format deduplication remains unfinished; VS TEMP
files do not promise complete history. Later [configuration design](../../design/desktop-usage/copilot-otel.md)
confirms pause stops scanning without excluding historical contributions. Introducing OTel
requires statistics-source selection; merely disabling native sources still mixes saved totals.
JetBrains has static plugin checks only, without real IDE/outfile samples, and remains
documentation-level. The CLI chronicle no-per-call-token conclusion applies only to checked
records; unknown newer versions receive no guessed fields.

The existing prerelease database policy prompts rebuilding on version mismatch, without
incremental migrations. This batch changed parser/source namespaces and moved schema to 10;
old trial DBs follow that recovery flow. Comparisons use new isolated databases without
modifying user application DBs or running actual JetBrains services/production systems.

<a id="后续修正2026-10-01健康状态与未知字段展示"></a>

## Later corrections (2026-10-01): health and unknown-field display

Two locally reproduced display issues were corrected using independent real-data checks,
without modifying the user application DB:

- Source notice to check one file: turn_input_incomplete describes promptTokens covering only
  the last call's input. Previously any diagnostic degraded health. This intrinsic format
  limit is neither a bad record nor reconciliation mismatch
  ([architecture](../../design/desktop-usage/architecture.md#unknown-version)). Only other
  diagnostics (bad lines/invalid tokens/duplicate keys/missing ownership, etc.) now degrade
  health; coverage-only notices stay active. session_log_v3 uses: all diagnostics
  turn_input_incomplete => active.
- Today's model-details unknown-field count 396: tool rounds are model_call with
  quality_bucket=unknown/per-round tokens unknown; turn observations carry usage. Previously
  each round added one input/one output unknown field. recompute_day/enrich_hourly_metadata
  now count calls/events for records with quality_bucket='unknown' and no known token fields,
  without adding input/output/total unknown counts, like transport_attempt exclusions. This
  applies across adapters: failed/no-usage calls stay visible through call_count without
  inflating unknown-field counts.

Real read-only real_verify_copilot_chat on local workspaceStorage, four logs: before correction
source_files active×2/degraded×1; vscode-copilot-chat model details input_unknown+output_unknown=434
(round-model claude-opus-4.8 was an independent all-unknown row). After correction active×4/
degraded×0, unknown fields zero, calls=222/observations=12, repeat adds zero. Twelve
turn_input_incomplete diagnostics remain visible without degrading health.

New regressions: session_log_v3 unit turn_input_incomplete_alone_keeps_source_active and
integration copilot_rounds_stay_active_and_do_not_inflate_unknown_fields. Existing no-usage
cases updated consistently (classification_v03/kilo/omp/pi/storage_jobs): syn-a2/failed calls'
input_unknown_count changes 1 to 0 with unchanged call counts.

<a id="旧游标恢复与补充审查2026-10-02"></a>

## Old-cursor recovery and further review (2026-10-02)

Preserve preceding uncommitted repairs/other-task changes and recheck implementation, field
quality, scanner framework and current local data. Upstream
[IChatUsage / IChatUsageModelTotal](https://github.com/microsoft/vscode/blob/main/src/vs/workbench/contrib/chat/common/chatService/chatService.ts)
and [ToolCallRound](https://github.com/microsoft/vscode-copilot-chat/blob/main/src/extension/prompt/common/toolCallRound.ts)
confirm last input/whole-turn usage/identity-and-time-only rounds are distinct. Upstream main
is a cross-check for this batch; real native files have version=3. Installed versions retain
earlier checks; no newer version is presumed accepted.

Additional findings/fixes:

- Fully consumed unchanged files skip parsing, leaving old degraded health/unknown-field daily
  totals after health/SQL-only fixes. Copilot parse_context gains scan_policy_version; absent
  current marker permits one full replay. Preserve revision/tracked_events; advance existing
  monotonic revisions to recompute daily/hourly totals. Bad snapshots never mark success;
  complete success restores skipping. No cursor-context reset/revision decrease/history deletion;
  schema/parser-format version unchanged.
- QualityBucket::of omitted output_reasoning/source_total, misclassifying rows containing only
  those as unknown. Excluding unknown rows could consequently hide real missing fields. Add both
  token fields. Zero/cache components/input-only/output-only still represent valid partial usage;
  estimates remain excluded from reported totals.

New regressions first failed: old-cursor refresh returned unchanged instead of complete;
reasoning-only hourly unknown counts zero instead of one each for input/output/total. Six
new Rust unit/SQLite cases cover recovering the fixed 198-round sample's 396 display/revision
7=>8, model rows/real missing output, bad-line degradation/recovery, no success marker for bad
snapshots, and recomputing old history after source cleanup. Coverage includes day/hour/week/month,
unknown filters/zeros/cache/reasoning/source totals/estimates/transport attempts.

Local Windows / PowerShell 7.6.6 / Node.js 24.21.0 / Cargo 1.98.1, read-only current app DB:
five files active×3/degraded×2; 235 rounds/13 observations; input lower bound 3,832,423,
cumulative output 487,818, complete total unknown; daily input/output missing-field sum 470.
The live sample grew. The fixed 198-round synthetic sample exactly reproduces 396, rather
than presenting it as the current measurement.

SQLite Online Backup made an isolated build/copilot-review/current-app copy. Native
workspaceStorage refresh on the copy with old cursors/Asia/Shanghai produced active×5/
degraded×0 and input/output missing fields 470=>0. Calls/observations/input/output/unknown total
stay unchanged; all 13 observations retain total_unknown_count=13, without inventing complete
totals. Independent Python mutation-log replay matches counts/tokens; repeats add zero.
real_verify_copilot_chat adds --reuse/--timezone for this check, passed an isolated DB only.
User live DB/IDE settings/OTel events.jsonl were unchanged; no Agents/model requests ran.

Completed root checks, all exit 0:

- cargo test --locked --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core
  --test copilot_ide_contract --test unknown_field_counts: 12 integration cases.
- cargo test --locked --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core
  copilot -- --nocapture: focused unit/integration regressions.
- python build/copilot-review/audit.py: independent replay of current native data.
- cargo run --quiet --locked --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core
  --example real_verify_copilot_chat -- &lt;workspaceStorage&gt; build/copilot-review/current-app
  --reuse --timezone=Asia/Shanghai: real old-cursor recovery/no new usage on repeat, PASS.
- python build/copilot-review/compare_current.py: native data/fixed-copy details/daily totals
  match; no conflicts/degraded files, PASS.
- npm run verify: 164 Markdown files/assets/three scripts/13 UI tests/Svelte zero errors or
  warnings/fmt/Clippy/775 Rust tests/frontend production build. Two previously ignored tests
  (models.dev live smoke/local configuration audit) were not run here.
- npm run test:browser: installed Edge interaction regressions.
- git diff --check: passed; initial-patch comparison preserves 16 other uncommitted files.

Ignored build/copilot-review logs: regression-before.log (two failed assertions),
regression-after.log, copilot-regression.log, current-raw-audit.log, current-app-replay.log,
current-app-after.log, current-compare.log, verify-current.log, browser-current.log.
Documentation lint also ran after record/plan synchronization. The validated copy was removed
after acceptance, retaining redacted summary logs only. No running desktop binary was installed
or replaced; refreshing an updated build recovers these old states without clearing the DB.
