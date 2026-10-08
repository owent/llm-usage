# Usage, storage and configuration rules

<a id="用量存储与配置规则"></a>

<a id="用量存储与配置合同"></a>

This document defines the required statistical behavior; implementation status is in
[Plan.md](../../../Plan.md). Each adapter must declare its field mapping. The UI cannot infer
missing fields. New source identities and price retrieval are tracked in M1a/F2.
Only data produced by local Agents is eligible under the [design scope](README.md).
Remote reports remain excluded even when saved to disk.

Read OpenClaw schema 24 local hot transcripts under the [specific rules](openclaw-runtime.md).
Keep positive uncached input, output and cache buckets; default zeros and computed totals
remain unknown. Database-wide versions and the latest session model do not verify historical
rows. Isolate external/migrated sources and show cold-archive gaps. Events and pagination
positions commit together in the unified batch transaction.

<a id="cline-sdk-会话读取"></a>

## Reading Cline SDK sessions

On 2026-10-06, the official VS Code 4.1.22 native writer at fixed commit
`f58bc118bdeef1bd2813cd08e00d98bdcda96475` was checked against isolated real samples.
The independent cline-sdk-messages-v1 reader complements the separate documentation-based
legacy ui_messages.json reader. Default sessions are
`~/.cline/data/sessions/<session>/<session>.messages.json`, resolved using the official
CLINE_DIR, CLINE_DATA_DIR and CLINE_SESSION_DATA_DIR precedence. Manual roots may be the
Cline root, data, sessions, a single session directory or a native file. Do not add registry
database, manifest cumulative tokens, hooks or conversation text as a second usage source.

Accept only schema 1, agent=lead, origin.source=vscode and origin.mode=user, with matching
sessionId fields in both locations. Other SDK interfaces, imports and subagents require
independent checks. Session metadata origin.version may be rewritten on resume and does not
verify every historical message. SDK reading remains latest_fallback; empty sessions do not
verify the format. Legacy format references do not select SDK reader versions.

Consume only assistant metrics without displayOnly, keyed by sessionId + message.id.
Text, tools and assistants without metrics produce no usage. The writer may place a whole
run's usage on its last message; retry middleware may merge multiple requests. Store
usage_observation and do not infer one underlying call per metrics object. Use only the
message's own modelInfo and ts; mark time uncertain. Do not borrow the current manifest's
model or session start time.

inputTokens includes cache. Positive input/output/cache fields are individually reported.
Writer/codec default zeros and absent fields remain unknown. Derive a complete total only
from known input + output; subtract cache to obtain uncached input only when total input and
both cache buckets are known. Diagnose cache exceeding input; do not truncate it. Cost,
reasoning output and latency lack verified native fields and remain unknown. The real sample
has three observations: input 8,922, output 48, complete total 8,970 and known cache read
5,881; cache write, uncached decomposition and call count are unknown.

Whole-file JSON reads have a 32 MiB limit and cooperative cancellation. Advance the cursor
only after complete parsing; partial writes and unknown schemas do not advance it. Diagnose
invalid numbers per record while importing other valid messages. Rescans and duplicate IDs
within a file add no duplicate usage; differing content for the same ID still uses conflict
handling. Historical messages disappearing from a source snapshot do not revoke observed usage.

<a id="metrics"></a>

<a id="token-与请求的定义"></a>

## Defining tokens and requests

Use mutually exclusive input buckets. Missing fields are null, rather than zero.

| Field | Meaning | Notes |
| --- | --- | --- |
| input_uncached | Input tokens neither read from cache nor classified as cache creation | Some providers include cache in input_tokens; others exclude it. Map each verified version separately. |
| input_cache_read | Input tokens read from cache | Included in normalized total input. |
| input_cache_write | Input tokens used to create cache for this request | Included in total input. TTL subdivisions such as 5 minutes/1 hour are subsets, not additional tokens. |
| input_total | All input tokens | Sum the three known exclusive buckets, or retain directly reported total input with unknown decomposition as null. |
| output_total | Output tokens including known reasoning subsets under the provider's semantics | Gemini candidates/thoughts may be separately reported and need a specific mapping. |
| output_reasoning | Reasoning subset of output | Do not add it again when output_total already includes it. |
| total_tokens | Normalized input_total + output_total | If only a source total is available, retain its value and basis without inventing input/output. |
| source_total | Original source total | Compare with normalized totals; diagnose semantic differences or inconsistencies. |

Mathematical definitions:

```text
input_total = input_uncached + input_cache_read + input_cache_write
total_tokens = input_total + output_total
cache_input_ratio = SUM(input_cache_read) / SUM(input_total)
```

Calculate the first two only when all required fields are known and semantically compatible.
Known total input with unknown components is valid. Calculate the ratio over the same records
with known total input and cache read; show eligible record count, known input and coverage.
For a zero denominator or no eligible samples, show “—”; do not show 0% or average daily
percentages. The share of calls with cache reads is a separate metric whose denominator is
model calls with known cache fields; do not confuse it with the token ratio.

For example, a provider explicitly has no cache creation and reports input=1000 including
cached=800, output=100 including reasoning=40. Uncached input is 200, total tokens 1100 and
cache-input ratio 80%, rather than 1940 tokens. Another source reports exclusive ordinary
input=100, cache read=800, cache write=100 and output=100: total tokens remain 1100 and the
ratio 80%, rather than 88.89% after omitting cache creation. These are synthetic rule-checking
values, rather than model prices or real session statistics.

Record reported / derived / estimated / unknown per field, alongside completeness and source
sampling state. Reported means the source reported it; it does not verify the final bill.
Estimates are excluded from known-usage totals by default. Negative values, overflow or cache
exceeding known total input produce bounded diagnostics; max(0, …) must not hide contradictions.

Continue CLI initializes both session cumulative cache buckets to zero and increases them
only for nonzero values. Fixed official source and real 1.5.47 local-model samples show that
zero cannot distinguish absent reporting from reported zero. Cache zeros in this format
remain unknown; positive native cumulative values are retained. Do not derive a complete
total from cache buckets mixed across providers. Product defaults cannot establish an API's
field presence; see [real samples](../../validation/desktop-usage/m8-container-samples.md).
Parsing-rule updates automatically reread unchanged old cursors. Same-revision correction is
allowed only when complete old/new aggregate summaries differ exclusively in cache
0/reported → NULL/unknown. Other token, quality, ownership, time and coverage changes follow
normal conflict handling. Keep diagnostic history; aggregates and cursors commit together.
Do not bypass retention cutoffs or sealed history.

The real gajae-code 0.18.7 session v5/OpenAI-completions format and fixed source show that
cache zeros come from missing-field fallbacks. Initialized input/output/total zeros also do
not establish API-reported zero. Retain positive values under the verified normalization and
recover total input through its inverse. Cache zeros remain unknown; uncached input remains
unknown unless both cache buckets are known. Do not derive complete totals when input or
output is unknown. Retain nonzero source totals independently; costs are client Estimated
values. message.timestamp is initialized before the request: record source_start. Entry
time and product version do not replace per-call time/format references. Fixed session-manager
source identifies configured_model_chain as configuration rather than usage; skip only that
type, while other unknown entry types still stop reading. Do not generalize this API's
missing-field defaults to other APIs. Old gjc-session-1 → 2 event corrections permit only
these zero/uncached-bucket and time-basis changes, after matching complete canonical/legacy
old summaries. Preserve source revision, identity, model, positive usage, cost and other
quality fields; retain first observation, conflicts and diagnostics. Events, summaries,
fingerprints and cursors commit together; sealing and retention cutoffs still apply.

Official AtomCode 5.2.1 OpenAI mapping and TokenBreakdown default missing fields to zero;
real .meta/API/CLI checks confirm this. Cumulative cache zeros remain unknown; without
reliable cache buckets, uncached input is unverified. Complete native normalized three-bucket
input with a positive sum can recover total input; retain positive output. Do not derive a
complete total when input/output is unknown. Missing or mistyped buckets cannot become zero.
Keep cumulative overflow unknown with bounded diagnostics; import other valid models.
turn_stats total_tokens is the last-request snapshot and is not added to model_usage.
Keep round_count as a separate source aggregate; do not expand turns without timestamps
into per-call records. atomcode-meta-turns-1 → 2 rereads old cursors automatically. Correct
old aggregates only when their complete summaries establish these default-zero/old-derived
fields; other known values, model, interval, call count, coverage and source revision stay
unchanged. Reconstruct unknown old derived candidates only from default zeros and still
trusted native buckets, rather than removing arbitrary real conflicts. Use the same-revision
aggregate-upgrade rules, retaining history, diagnostics, transactions, retention and sealing.
Real .ui.json v1 and .rewind.json version 2 are verified auxiliary state. Exclude them only
with complete JSON/explicit shape, a same-name valid .meta and matching identity, within a
64 KiB probe. Unknown versions, invalid shapes, unmatched files and manual JSON containing
real id/turn_stats still receive format diagnostics/reading. Filename suffixes cannot hide
user-file errors. Other auxiliary formats require verification before exclusion.

Official Junie 26.9.22 (3419.29) distribution and seven real OpenAICompletion calls were
checked. inputTokens is normalized uncached input, rather than total input; cache/output
are independent components. Official OpenAI/Responses, Anthropic and Google converters and
ModelUsage construction were inspected statically. UsageTokens defaults missing fields to
zero in events: zero buckets remain unknown, positive buckets stay reported. The format
contains neither API type nor product version; a model name cannot verify total input or
total tokens, which remain unknown. time=0 may be a default and remains unknown; cost=0
does not establish free usage or reported zero cost. Official OpenAI calcTokenCost estimates
using client ModelCapabilities rates; absent prices do not prove free usage. Positive cost
is Estimated; USD follows the original documented reference. Actual paid channel/unit have
not been accepted. LlmResponseMetadataEvent calls already written before a task fails still
count. junie-events-doc1 → 2 rereads unchanged old cursors and permits only corrections of
input buckets, default zeros/time and cost quality established by complete canonical/legacy
old summaries. Keep original keys, revisions and other fields. Bounded old-candidate
enumeration covers absent/default-zero fields, not arbitrary positive values. Retain first
observation, conflicts and diagnostic history; events/summaries/cursors commit together.
Sealing and retention still apply. JUNIE_HOME is a verified discovery override.
See [real samples](../../validation/desktop-usage/m8-container-samples.md).

Official Roo VSIX 3.54.0, fixed commit 27001b2b and a real VS Code extension host were
checked. api_req_started tokensIn is total input including cache; positive tokensOut is
output. Four buckets initialize to zero; missing provider usage/cache fields still write
zero, so zeros remain unknown. The OpenAI-compatible route reads only top-level
cache_read_input_tokens, rather than prompt_tokens_details.cached_tokens. Real cache hits
may therefore leave native zero buckets; subtracting those zeros cannot establish uncached
input. Derive input_uncached only when both cache subsets are known. Positive total input
and output can yield total_tokens. Cost uses client model rates; positive values are
Estimated. Default zero from absent rates is unknown and does not prove a free bill.
Empty placeholders without numbers produce no events. Requests with explicit numeric
fields, including default zeros, retain one observed call with unknown token fields.
Cancellation may delete incomplete api_req_started records: an unlimited run had three API
calls but two native records; do not invent the missing call. Keep the separate one-call
comparison using the public limit of two. An archive announcement does not establish that
historical local providers cannot run. roo-ui-messages-doc1 → 2 reevaluates unchanged old
cursors. Complete canonical/legacy old summaries permit only zero-bucket/zero-estimate and
their derived-field corrections. Protect keys, time, positive usage, quality, model/ownership
and real conflicts. Events/summaries/cursors commit together; keep diagnostics and first
observation. This does not verify other versions, CLI or tool/subagent coverage.

Official Xum npm 0.30.0, matching commit 81b0b744 and two real local calls were checked.
session-usage.json v1 input is exclusive uncached input; output excludes reasoning. Product
normalization/accumulation defaults five absent buckets to zero. Zero buckets stay unknown;
positive input maps to uncached input. Complete input and total tokens remain unknown: mixed
historical display buckets do not establish complete provider totals. Add positive text
output and known positive reasoning as derived output. If reasoning is unknown, retain
positive text output as a lower bound with xum_output_incomplete coverage notice, without
downgrading source health or deriving complete totals. Ignore CLI unknown-price zero cost;
do not expand byModel into per-call records. The default custom OpenAI-compatible route does
not request streaming usage; real missing usage writes five zeros and does not establish
reported zero. A separate comparison used only a local gateway to request include_usage
from a real model, forwarding its response unchanged. That gateway setting is not a default
client capability. CLI deletes temporary sessions by default; retain native files using
verified XUM_RUN_SESSION_ROOT/MUX_RUN_SESSION_ROOT. Configuration roots support XUM_ROOT/
MUX_ROOT. xum-session-usage-1 → 2 rereads old cursors, permitting only input-bucket/default-zero
corrections and known reasoning combination established by complete old aggregate summaries.
Other tokens/quality, revision, identity, interval, calls and coverage stay unchanged. Keep
diagnostics, conflict handling, transactions, retention and sealing. Format v1 does not
verify all product versions. See [real samples](../../validation/desktop-usage/m8-container-samples.md).

Real native MiMo 0.1.15, Zoo 3.86.0 and DSH 0.2.0-rc.2 files were checked against independent
API observations. Writer/codec, default zeros, component recovery and read limits are in
[the three source specifications](m3-runtime-samples.md). Positive Zoo tokensIn includes
cache; default-zero components remain unknown and do not establish uncached input. Recover
positive MiMo input/output totals only using its own SDK normalization; zero cache/reasoning
remains unknown. Do not add message copies. Native DSH input is uncached. Invert normalization
to recover total input only when the record itself establishes pi-ai openai-completions/
version 2 and positive input was not reduced to zero. Output already includes reasoning;
computed total is not source_total. Select settlement or stream, not both. Same-step
replacement may reduce usage; retry starts another attempt. Inherited prefixes are not
local calls. Unwritten title usage is a visible gap. Snapshot regression or changed source
identity retains existing values/cursors. MiMo/Zoo old-rule corrections require complete
canonical/legacy old summaries, identical revision and matching protected fields; keep real
conflicts within the same batch. Reevaluate consumed old positions and commit events,
summaries and cursors together.

<a id="请求消息与累计值"></a>

### Requests, messages and cumulative values

| Kind | Statistical behavior |
| --- | --- |
| model_call | One model call/attempt established by explicit records; basic request metric unit. |
| transport_attempt | HTTP/WebSocket reconnect/retry, counted independently. Do not add model_call unless it establishes a new model call. |
| usage_observation | One usage record, potentially one-to-many or many-to-one with calls. COUNT(*) is not automatically request count. |
| cumulative_snapshot | Session/process cumulative snapshot; compute differences using identity, version and reset boundaries, rather than summing each row. |
| interval_aggregate | Official daily/session aggregate; retain native scope, fields and units. |
| quota_snapshot | Quota, credit, subscription window or balance; display independently without converting to tokens/requests. |

The default card is “Observed model calls” with its coverage beside it. Only sources with
verified per-call identity contribute. Failed calls may count with unknown usage; do not
zero-fill unknown tokens. Records with no known token fields (quality_bucket=unknown, such
as failed calls or call-only tool-loop rounds) count calls/events but do not inflate the
displayed missing-field counts. Those counts describe gaps in partially observed usage.
Successful-response logs alone cannot calculate overall success rate. Quality partitioning
examines every token field, including output_reasoning and source_total. Cache-only,
reasoning-only or source-total-only records are still partial usage; their missing input/
output/complete totals count as missing fields. Reported zero is known. Estimated fields
remain separate from reported fields. Daily/hourly and derived weekly/monthly aggregates
use the same missing-field rules. User messages, tools, sessions and paid premium requests/
credits are separate metrics. Token-only cumulative values leave call count unknown; the
number of positive differences cannot establish calls.

Hermes and similar sources may provide interval-cumulative api_call_count. Keep it as a
“Source-reported call aggregate”, verify its meaning and display the native interval. Do
not invent per-call model_call or add it to per-call counts covering the same usage. Two
local requests/resumed sessions from the official Hermes 0.21.5 (v2026.9.24) image were
checked: native session_model_usage.input_tokens is normalize_usage uncached input, rather
than total input. Fixed release source agrees with the earlier A24 reference; absent/
invalid buckets become zero and SQL accumulation also defaults to zero. Zero in all five
token buckets and api_call_count stays unknown; positive values stay reported. Derive
input/complete totals with checked addition only when uncached, cache read/write and output
are all known; default zeros cannot complete them. Reasoning is an output subset. Cumulative
rows retain native interval, call aggregate and six-part identity. Database schema_version
describes migrations and does not verify client versions in mixed historical records:
retain compatibility reading. Old-rule upgrades require complete old aggregate-summary
matches, permitting only input_total → input_uncached, default-zero → unknown and specified
derived-field updates. Other counts/quality/range/coverage/revision use normal conflict
handling. Keep historical diagnostics; fully reevaluate consumed old positions and commit
them with cursors. Integrate quota_snapshot only when local records verify local ownership;
account quotas use the independent display below.

Overview/trends read copilot-user-cache.json premium_interactions into generic quota_history
(independent of agent; kind=rate_limit, unit=milli_requests). It is an account premium-request
quota shared across devices/interfaces, locality_verified=false, rather than tokens. Do
not add it to local token/request totals. Future Agents with similar request/credit balances
reuse quota_history without agent-specific tables. Fractional quota is stored as integer
thousandths of a request and displayed divided by 1000. Precision beyond that unit remains
unknown, without rounding/truncating to zero. Bucket/deduplicate using timestamp_utc; absent
source time cannot create a synthetic new snapshot. Same-time value/metadata corrections
may update; identical values at new times are still retained. Quota clearing and hard
retention apply to this table. Collect/verify VS Code native usage and account quota
separately; retain quota records independently.

VS Code Copilot Chat (M9) writes user-turn promptTokens/completionTokens/copilotCredits/
elapsedMs/modelTotals in `workspaceStorage/<hash>/chatSessions/<sessionId>.jsonl`, using
chatSessionOperationLog storageSchema v3 and VS Code chatModel.toJSON semantics. A turn is
usage_observation. promptTokens is the last model call's input: a turn input lower bound,
rather than the sum of every call; VS Code does not persist per-call input. completionTokens
is cumulative output across the whole turn. When present, modelTotals supplies the definitive
whole-turn per-model input/cached/output totals and takes precedence. Cache components remain
unknown by default; do not zero-fill. Thinking coverage is incomplete and excluded from
output_reasoning. copilotCredits measures credits converted from nano AIU, rather than tokens,
and stays outside token totals, as an independent metric like quota above. Streaming counters
are periodically sampled snapshots: replay each whole turn to select final values and update
the same keys without duplicates. Do not sum updates. Stable-ID/timestamp main-loop entries
in toolCallRounds independently count model_call, with unknown per-round tokens. Turn/model
observations contribute usage; these quality_bucket=unknown round markers count calls but
do not inflate missing-field displays. Without rounds, turn count cannot substitute for calls.

turn_input_incomplete, indicating promptTokens covers only the last call, is a format coverage
limit, rather than invalid data or reconciliation disagreement. It does not downgrade source
health; invalid rows/tokens, duplicate keys or absent ownership do. Likewise, Codex cumulative
reconciliation differences reconcile_mismatch/snapshot_regression retain their diagnostics
and mismatch result without independently degrading read health. New independent per-call
records do not depend on cumulative snapshots; old total/last errors needed for call identity
still degrade health. Missing/null info contains no usage and neither counts calls nor becomes
zero. Statistics/health rule upgrades replay existing unchanged Copilot cursors once, keeping
revision and history. Complete valid replay advances revision and recomputes affected unsealed
days. Invalid rows, partial lines or exhausted read limits cannot mark the update successful;
retry later. After completion, unchanged-file skipping resumes. Keep unknown tokens without
clearing the database or increasing schema. Default last-call input and whole-turn output
have different coverage and cannot yield complete total_tokens. New modelTotals replaces old
contributions; model keys remain stable when arrays reorder. Accept only requests owned by
the github.copilot namespace. Workspace/globalStorage/emptyWindowChatSessions in the same
installation share a source to prevent migration-copy duplication. Partial lines, exhausted
read limits or damaged snapshots never replace existing usage. See
[the review](../../validation/desktop-usage/m9-copilot-review.md).

For verified VS Code Copilot file exports, SDK CLIENT chat &lt;model&gt; spans become per-call
events. Input includes cache and reasoning is an output subset, not an extra addition.
Derive total only when input/output are known; cache decomposition stays unknown. Without
shared call IDs, select by verified host/user/session/local day: OTel replaces native
contributions within that scope, retaining native records and revisions. Native rescans
still apply the selection; sealed partitions are never added again. Coverage before enabling
export cannot be recovered and is explicitly partial history. Unconsumed, rejected or expired
records cannot establish a preferred-source range. Normalize Claude decimal/dash model
spellings for grouping/pricing while retaining raw model fields. See [dashboard requirements](dashboard-repair.md).
Same-named CLI/JetBrains attributes in new versions cannot replace real-format acceptance.

Claude Code 2.1.197 native assistant records carry their own version. Bind each record to
its version, rather than installed version, first queued metadata or the highest version
in a file. Other versions are compatible reads; retain separate old unversioned references.
Validate the full four-bucket shape. Positive input is uncached input and positive output
is reported output. The streaming writer adds zero buckets for unreported values: without
a validity marker, zeros remain unknown and do not complete missing input/totals. Multiple
content blocks from one response share message.id. Without requestId, deduplicate using
message ID, rather than treating each uuid as a call. The format has no channel; Anthropic
protocol or model names cannot establish provider/actual payment. CLI estimates do not
fill transcript costs. Correct old doc1 default zeros, derived values, version and provider
assumptions only after matching complete old event summaries. Actual positive usage/model/
quality/revision changes still follow conflict handling. Reevaluate consumed unchanged
positions; retain first observation, original completion time, real conflicts and diagnostics.
Recompute unsealed days in the same transaction. Interrupted bounded scans can resume;
mark rules updated only after a complete valid scan. See
[native samples and upgrades](../../validation/desktop-usage/claude-container-sample.md).

Route manual .qwen roots to Qwen only when the first record in the documented chats layout
has complete ChatRecord identity and Qwen message.parts or usageMetadata. A shared type
field does not establish Claude format. Qwen still reports invalid rows. Recover old wrong
ownership only when the entire source has no events or daily/period/native summaries. Release
the file's incorrect checkpoint while retaining diagnostics/configuration. Historical
contributions require keeping existing ownership.

Qwen Code 0.25.0 local SDK files contain consecutive multiline JSON objects. Only INTERNAL
qwen-code.llm_request spans establish per-call records; logs, HTTP spans, parent interactions
and metrics are not added. Verify product/version in resources and trace+span identity.
Input includes cache read; retain output and thoughts separately. Real samples with thoughts=0
permit total derivation from known input/output. Other reasoning/provider inclusion semantics
require separate verification before combining totals; do not invent cache write or call
ancestry. SDK/ChatRecord lack common call IDs: accepted 0.25.0 SDK spans replace native usage
for the verified host/user/session/local day, retaining native records, excluding rescan
duplication and prioritizing sealed partitions. When native/SDK source-day partitions are
sealed and per-call identity is no longer recoverable, new SDK exports for the same host/user/
local day cannot establish non-overlap. Keep exclusion reasons and coverage notices without
adding them to sealed partitions. Regress actual retention cleanup, restart and late native/
SDK copies, rather than merely simulating detail deletion. Select one export copy per trace+
span. Isolate other versions; missing source identity cannot establish preferred-source scope.
Export may start after session creation and does not promise complete partition history.
Partial objects, exhausted read/object limits and invalid objects remain incomplete, with
recoverable complete-object boundaries. SDK private fields are not a stable OTLP specification.

For Visual Studio totals, derive total_tokens only when input/output in the same CLIENT chat
span are known. Do not add cache again. Missing fields/overflow remain unknown. Replay consumed
unchanged old cursors once; only complete valid snapshots mark the update finished. For old
v1/v2 formats, supplementation is allowed only when the complete old-field hash matches the
new event with total removed. Other field conflicts cannot be treated as upgrades. Preserve
call identities/count, advance data revision and do not clear the database. Missing source
files cannot recover unknown totals.

Visual Studio Copilot per-call records (M9) are automatic OTLP JSON exports from verified
VS 18 components at `Path.GetTempPath()/VSGitHubCopilotLogs/traces/*.jsonl`. Discovery checks
TMP/TEMP and the user's default LOCALAPPDATA/Temp candidates, deduplicating physical roots
without SKU/year filters. Discovery requires no installation root; inspect installations
with official vswhere querying every instance, rather than composing SKU/year paths.
The checked VS 2022 Copilot 17.14.1713.63837 component lacks this exporter. Verify older
extensions/other formats separately; installation, quota, context limits or CSV lacking
event time cannot substitute for per-call usage. See
[cross-version checks](../../validation/desktop-usage/m9-vs-copilot-discovery.md).
One CLIENT chat &lt;model&gt; span is one observed model_call within telemetry coverage.
Failed calls without usage keep unknown tokens. Verify service.name and trace/span identity
per batch. gen_ai.usage.input_tokens/output_tokens/cache_read.input_tokens are reported
per request as OTLP intValue strings. Skip invoke_agent whole-turn aggregate spans to avoid
double counting. Independently reported input/cache buckets do not establish uncached input:
VS documentation does not declare their inclusion relationship, consistent with otel-family
rules. Read only explicitly selected keys; prompt-body properties such as gen_ai.input.messages
are neither stored nor printed. TEMP cleanup/rotation may remove history; retain existing
results without claiming complete coverage.

<a id="标准化记录"></a>

## Normalized records

This is a logical model, rather than an already-created SQL schema.

| Group | Required or optional fields |
| --- | --- |
| Identity | event_id, origin_host_id, source_instance_id, source_record_key, record_kind, schema_version, parser_version |
| Parser compatibility | Nullable original source version, selection basis known_version/latest_fallback, compatibility validation state; separate from field quality. |
| Relationships | origin_call_id, attempt_id, session_id, parent_session_id, host_application, agent, call kind |
| Time | occurred_at_utc, observed_at_utc, source_time, time_basis, optional interval_start/end |
| Model | provider_id, model_raw, model_canonical, model_attribution; retain unknown. |
| Tokens | Fields above, known-field bitmap/quality, source-reported total and bounded explicitly allowed extensions. |
| Lifecycle | partial/final/corrected, source_revision, error/cancel state, nullable duration/TTFT |
| Provenance | Hostname snapshot/display alias, local instance/optional WSL or container instance, locality_basis, ownership validation, format, location identifier, digest, sampling/completeness; may link through source registry. |
| Optional dimensions | Workspace HMAC, custom names, main/subagent/auxiliary/unknown call classification |
| Cost | amount_decimal, currency, reported/estimated, price_version, nullable billing_scope |

Store tokens as nonnegative signed 64-bit integers, checking limits before aggregation.
Reject/report out-of-range records rather than converting to approximate floating point.
Costs use fixed-precision decimals/integer minor units with explicit rounding; do not sum
bills using binary floating point.

Store models as (provider_id, model_raw); derive display aliases/families using versioned
rules. Same-named custom models from different providers do not merge by default. A current
session model cannot identify every historical call: use structured model changes no later
than the call, or its own request fields. Otherwise attribution is unknown. A line-wide regex
finding “model” inside prompt text cannot establish the call's model. IDE is host_application;
agent is the actual caller, such as VS Code + Copilot or Zed + Codex. Host display and Agent
usage are dimensions of one record, rather than two additive events.

Unknown versions try the latest built-in reader under [compatibility policy](architecture.md#unknown-version).
Validated data can enter statistics; compatibility state survives queries, daily summaries,
sealing and exports. Aggregation cannot discard notices. Compatibility does not change
reported/derived field categories or turn absent values into zeros. Partial reads show
coverage gaps. Later dedicated-reader rescans correct old contributions using stable identity
without duplicate accumulation.

Parser version is part of parsing references. On upgrade replay, change parser/digest/
compatibility references and remove that record's incorrect-version notice only when the
complete old event summary equals the new event with parser version removed. Keep time,
usage, call identity, source revision and diagnostic history; transactionally recompute affected
unsealed day/hour summaries and advance revision. Observation time and repeated same-key
final timestamps retain existing duplicate-update rules. Token, quality, model, ownership,
source format or other content changes still follow source-revision/lifecycle handling.
Later metadata updates in the same batch cannot remove real conflicts; a repeated matching
version cannot clear them either. Missing sources cannot directly clear flags. Sealed summaries
stay unchanged. Codex/Kilo upgrades use existing parser-version cursor replay and resume
incremental reading after completion.

<a id="provenance"></a>

<a id="历史来源身份与数据交换"></a>

<a id="历史来源身份与交换合同"></a>

## Historical source identity and exchange

M1a implements this section. Persist original provenance rather than only global Agent/model
totals. Hostnames aid recognition; stable identities form keys/indexes. host_application
means an IDE or similar host application, rather than the computer.

| Identity information | Persistence and stability |
| --- | --- |
| origin_host_id | Opaque stable ID generated/persisted on first registration. Hostname, IP, hardware fingerprint and account secrets are not unique identity. |
| Hostnames/display aliases | Retain observed names and changes. Renaming preserves origin_host_id; same name does not imply same host. Export may redact names. |
| source_instance_id | Agent installation/profile/source instance within the host, with separate WSL/container environment identity. Restart/confirmed directory migration does not create another source. |
| Collection/import information | Separately record executing instance, batch and time without replacing original host/source identity. |

(origin_host_id, source_instance_id) uniquely identifies original provenance. Event logical
keys include that source and source_record_key. An equivalent global source ID namespaced by
host is allowed, but requires database constraints and source/time indexes; display text alone
cannot supply host identity. Call aliases, native cumulative/interval summaries, quotas,
daily/sealed summaries and import manifests retain resolvable source links. Registry lifetime
covers all referencing history; detail cleanup cannot remove required registrations.

Export/reimport preserves original identities; the importing host cannot replace them with
itself. Restoring/copying an application database to another host preserves historical sources
while explicitly distinguishing new local collection identity. Registry conflicts require
mapping/confirmation, rather than silently recording two hosts as one source. Names/source IDs
support deduplication but do not establish local ownership/authenticity; locality_basis and
other eligibility references remain necessary.

Import/Merge rules below distinguish layers. Aggregates compare revisions per partition key:
higher revisions replace; equal revisions also replace under overwrite semantics, with equal
content adding no duplicates. Lower revisions preserve existing values and diagnose conflicts.
Details use the [independent complete normalized package](detail-merge.md), following native
event conflict handling and recomputing within one transaction. Preserve cumulative/sealed
partitions; do not import collection cursors.

| Scenario | Required behavior |
| --- | --- |
| Same original source/key/revision | Skip identical content without duplicates. Different content at the same revision is a conflict; repeated exchange adds no usage. |
| Same source/key with a demonstrably superseding revision | Revoke the old contribution and replace it. Without revision ordering, retain conflict rather than choosing larger tokens/cost. |
| Verified distinct sources/records with disjoint coverage | Add independent contributions. Copies/host mirrors still follow cross-source deduplication; different source IDs alone do not justify addition. |
| Complete daily/interval snapshot for the same source | Validate revision/completeness within identical source partition, time/timezone, dimensions and rule version, then atomically replace. Incremental/partial snapshots cannot replace entire history. |
| Unknown ownership, source conflict or inseparable aggregate coverage | Preserve existing results/diagnostics and preview required mapping. Same hostname cannot justify automatic replacement or addition of overlapping usage. |

Exchange includes format version, original registry, record/partition keys, range/timezone,
field completeness, revision, complete/incremental snapshot kind and batch identity. Batch
identity prevents duplicate import operations but cannot replace source-record identity.
Absence from incremental packages does not imply deletion. Deletion/whole-range replacement
must be explicitly defined. Display CSV/charts need not satisfy exchange requirements and
cannot be treated as lossless reimport files.

Legacy migration uses only verified source mappings. History without host attribution retains
an independent stable legacy_unknown namespace with old database/source links, rather than
one global unknown key or invented attribution to the importing host. Mixed-source summaries
without details keep original values and unknown ownership; do not invent each host's share.
Repeated migration adds no duplicates. Hostnames are identifiable information; export may
alias/omit them while keeping opaque stable keys. These rules preserve later exchange support;
current collection still accepts only verified local Agent data.

<a id="身份与去重"></a>

## Identity and deduplication

Prefer stable source response/request IDs, namespaced by account/installation/runtime instance.
Otherwise use session UUID + event ID/sequence + kind, falling back to file identity +
generation + byte position. Content hashes detect changes and do not alone establish event
identity: legitimate calls may have identical usage. Paths alone do not identify mirrored
calls either; copying/renaming a session cannot create new consumption.

Streaming, final and corrected usage for the same request use upsert. Order by source revision
when available; otherwise use verified format lifecycle. Final values need not exceed partial
values, so MAX is inappropriate. If relative precedence is unknown, retain conflict and
diagnostics rather than choosing the larger record.

Cross-source deduplication has two levels:

1. Within each source, stable record identity prevents duplicates. File mirrors use native
   session/installation identity as well.
2. Local logs, OTLP, host mirrors and local session exports linked by origin_call_id/trace
   become aliases of one logical call. Without links, user selection chooses the primary
   source for overlapping coverage; other sources are comparison-only, without automatic addition.

Priority is selected per scope + metric. It is not a global ranking of logs over telemetry.
Logs might supply historical tokens while future OTLP supplies errors/latency, with persisted
transition boundaries. Unlinkable overlaps are displayed separately. Similar time/model/token
values cannot establish fuzzy duplicates. Cloud bills/account totals are excluded from the
application and cannot substitute for local per-request usage.

Parent-session summaries, subagent requests and auxiliary calls need explicit coverage sets.
For example, DSH final samples replace stream values for the same attempt and retries create
new attempts. Cline deleted_api_reqs/subagent_usage are aggregates and cannot be added to their
corresponding details. Without details, retain aggregate scope rather than inventing models,
times or call counts.

<a id="累计值与遥测"></a>

## Cumulative values and telemetry

Independent session cumulative snapshots are reconciliation only, rather than replacements
for per-call usage. Disagreements between Kilo session.tokens_* and message.data.tokens keep
reconcile_mismatch without degrading valid per-record source health. Incomplete/mistyped
details get detail_incomplete; SQLite conversion cannot turn them into zero/floating totals.
Reconciliation cannot prevent valid-message imports. Invalid JSON, absent roles, malformed
usage and single-record token contradictions still require review. Invalid rows outside
incremental windows remain unhealthy until the same row reads successfully or disappears
from the source database. Health-rule changes increase parser version and replay old positions
through real discovery, rather than clearing the database or diagnostics. Unknown owning
message versions remain compatible reads; readable reconciliation does not verify them.

Cumulative metric-series identity includes resource, instrument, attributes, process instance
and start_time. Repeated same-series/start/end data is processed once; delta data also needs
deduplication. Distinguish explicit resets from source corrections when values decrease;
without reset references, do not restart accumulation from zero. First-seen cumulative values
can remain native interval totals. If the interval crosses days without intermediate samples,
do not assign everything to today or distribute uniformly by duration. Detail trends show
coverage gaps and explain differences from totals.

OTel token-histogram sum is token quantity; count is instrument sample count. Input and output
may each emit one sample, so adding their counts does not establish calls. Count per-request
spans only at leaf LLM calls, excluding invoke_agent session totals. Trace sampling, packet loss,
missing final usage and offline applications affect completeness. Multiplying sampled data
cannot turn estimates into measured complete usage.

<a id="time"></a>

<a id="时间和汇总"></a>

## Time and aggregation

Store UTC integer-millisecond timestamps with source timezone/precision. Display/bucket using
a persisted user-selected IANA timezone. Initially use system timezone; later system changes
cannot silently rebucket history. Naive source timestamps require verified source configuration;
otherwise time is uncertain. File mtime is for scanning, rather than request occurrence time.

All intervals are half-open [start, end). Days/weeks/months use selected-timezone calendar
boundaries. Default weeks start Monday with week-year labels; Sunday-start weeks use start-date
labels rather than claiming ISO week semantics. Calendar months differ from 30 days. Preserve
DST 23/25-hour days and label repeated hours with UTC offset. Cross-midnight calls normally
use source completion time; when only start time is known, explicitly record time_basis.

Persist daily contributions per original host/source instance, then aggregate timezone version,
day, Agent, provider, model, call kind and quality/coverage. Source participates in persisted
partition keys or equivalent unique constraints. Cross-source overview merges queries rather
than retaining only origin-free totals. Weeks/months sum days and recalculate ratios. Use
DISTINCT for sessions crossing periods, rather than summing daily session counts. P50/P95
cannot average daily quantiles: use retained details or a verified mergeable distribution.
The first release omits cross-period quantiles when details are absent. High-cardinality
project/session dimensions query detail indexes on demand.

Each aggregate returns known values, records with all required fields known, unknown counts,
sampling/conflict counts and source coverage. Keep known-subset sums separate from complete
totals: known input may display while unknown output leaves total tokens incomplete. Unknown
models keep separate rows; filtering their display cannot remove known tokens from totals.

Mark incomplete current weeks/months “In progress”; mark the earliest retention-truncated
weeks/months “Partial history”. Year-over-year/period comparisons match range, timezone,
source selection, metric definition and elapsed portion. Basic multi-select filters include
Agent/model/provider/quality, AND across fields and OR within a field. Group/filter Agent,
model and provider using common Unicode lowercase rules, without changing native call IDs.
Model tables retain provider dimensions; same-named models from different providers do not
become one source identity. Deduplicate range sessions by (source instance, session ID);
missing IDs/incomplete detail make the result unknown. Hourly metrics use that hour's records.
Weight average duration by calls with known duration, rather than all calls. Daily partitions
can deduplicate active days exactly; coarse-only periods cannot add each source's active-day
count. Hourly layers retain actual quality buckets/conflicts. Missing old hourly completeness,
ordinary input and cache ratios are recovered only from retained details, never by subtracting
input/cache summed from different sample sets.

Each heatmap cell is one real date in the selected timezone, read from retained daily summaries.
Weekday distribution reuses calls from those dates. Days with only weekly/monthly/yearly
archives cannot be reconstructed and are unavailable. If other sources still retain daily
data for that date, show known values with partial coverage. Imported packages may lack local
daily cutoffs; inspect period archives themselves. Missing days are not zero-call days. Do
not distribute or repeat period totals. Completeness ratios without sample metadata remain
unknown.

<a id="数据表事务与恢复"></a>

## Tables, transactions and recovery

| Logical table | Purpose |
| --- | --- |
| origin_hosts / origin_host_names / users / source_instances / source_files | Stable hosts, names/aliases, statistical users (v6), instance user/environment ownership, enablement, versions, file identity/generation, capabilities and health. |
| ingestion_checkpoints | Committed cursors/versioned context, including model state, cumulative baseline and unfinished requests. |
| usage_events / event_aliases | Normalized calls/records and cross-source links, with unique constraints preventing duplicates. |
| source_aggregates / quota_snapshots | Native interval summaries/quotas, never presented as per-call events. |
| hourly_usage / daily_usage / period_usage / aggregate_generations | Archive layers (v5): hours, original-source-partitioned days, materialized weeks/months/years, known counts, coverage and rule versions; sealing preserves provenance. |
| settings / model_aliases / price_versions | Schema-versioned configuration and cost references. |
| extraction_schedules / schedule_state | Global/per-source rules, timezones, logical trigger times, pause reasons and desired/applied OS-task state. |
| ingest_runs / diagnostics | Bounded execution state/redacted errors, without raw text. |
| import_manifests / schema_migrations | Import batch identity/range, migration versions and rollback information. |

Primary indexes cover original-host/source record uniqueness, source/time, occurred_at,
model/time, Agent/time and source-revision cursors. Add project indexes only when optional
dimensions are enabled. Use real EXPLAIN QUERY PLAN and benchmarks to select indexes, rather
than precreating every dimension combination.

Each batch commits event updates, cursors, parser state and affected daily statistics together.
Replay interrupted uncommitted batches. Another source's successful commit cannot save a failed
source's cursor early. Complex rebuilds use a new aggregate_generation and switch atomically
after completion; the UI retains the old revision meanwhile. Damaged aggregates can rebuild
from retained details; absent historical detail makes exact reconstruction explicitly unavailable.

Check free space and make a consistent backup before migrations. Run versioned migrations
transactionally; failure keeps the old database. An older application encountering a newer
schema refuses writes rather than destructively downgrading. Parser-upgrade rescans use new
revisions and retain existing statistics until comparison succeeds; missing sources cannot
clear historical usage.

<a id="settings"></a>

<a id="分级归档保留"></a>

## Layered archive retention

Details → hours → days → weeks/months/years have progressively longer retention. Defaults
are 7/3/90/1095/3650 days/lifetime:

| Layer | Storage | Default | Notes |
| --- | --- | --- | --- |
| Session details | usage_events | 7 days | Seal the containing day before deletion (V14 semantics). |
| Hourly summaries | hourly_usage | 3 days | Recompute with affected days in each commit transaction; hourly charts read this layer. |
| Daily summaries | daily_usage | 90 days | Daily queries and recent week/month derivation. |
| Weekly/monthly | period_usage | 3/10 years | Materialize completed periods from days; repeated replace adds no duplicates. |
| Yearly | period_usage | Lifetime | Same rule. |

Details have no ordering constraint and may outlive hours; this is redundant, without losing
data. Coarser layers cannot expire before finer layers: validate hourly ≤ daily ≤ weekly ≤
monthly ≤ yearly.

<a id="多用户v6"></a>

## Multiple users (v6)

- users(user_id, name): create random internal IDs, trim display names and reject empty/duplicate
  names. The default user_id=default is created during migration and owns all existing sources.
  Initially use the OS username for its display name, falling back to default if unavailable.
- source_instances.user_id owns each source instance. Different users may share a source host:
  origin_hosts and users have a many-to-many relationship, allowing different people's Agent
  data on one machine to belong to different statistical users.
- Queries select the current user's source set via Filters.instances, resolved in the app.
  Core statistics do not depend on user concepts. An empty set means that user has no sources.
- Imported sources default to the default user and disabled state, using remote_sync ownership
  basis. Users reassign and enable them on the sources page.

- Protect ongoing periods: the daily cutoff cannot cross the current week/month/year start,
  which would permanently lose unfinished periods. Effective retention may exceed configuration
  by up to a period (≤31 days plus year-start protection).
- Materialized weeks use fixed ISO Monday labels. Week-start display changes require explicit
  rebuilding to change materialization.
- distinct_sessions requires complete details and known session identity. NULL in materialized
  layers indicates coverage gaps; V06 does not invent counts.
- Complete periods replace only corresponding same-source/model/kind/quality daily partitions
  and must fall entirely within the query range. A whole month cannot answer a one-day query.
  Later partial daily data cannot overwrite complete periods.
- Repeated retention with the same day/policy/revision neither rematerializes nor advances revision.
- Under the same day/policy, skip completed-period materialization only when changes affect
  ongoing periods alone. Check completed-period daily count/max revision, period count/max
  revision and committed position together. Late corrections/deletions, missing periods,
  changed dates/policies, external future revisions or absent/damaged positions require full
  rebuild. Commit the position with retention; failure/cancellation saves nothing. Clearing
  usage also resets it.
- Aggregate exchange exports registry + all daily/period/hourly partitions without session
  details. Reimport restores historical trends while showing detail-only gaps. Daily partitions
  retain all known/unknown counts and ratio samples; hours/periods retain conflicts and original
  host ownership. Missing completeness in old packages remains unknown. Identical reimports
  do not advance revision. Different same-revision content follows overwrite rules; lower
  revisions keep existing data and diagnose. Invalid partitions roll back the entire batch.
  Same-path sources on different hosts cannot merge. Empty users/export selection does not
  mean all sources. Different aggregate snapshots cannot overwrite partitions with local
  per-call detail; reimporting the same own snapshot does not seal active days. Aggregate
  import restores historical snapshots. Complete normalized details/cumulative Merge are
  implemented under [detail exchange](detail-merge.md). Later local collection retains its
  own cursors and source-revision handling.

<a id="配置与数据保留"></a>

## Configuration and retention

All user settings can be viewed, edited and reset in the UI, with consistent validation,
impact preview, transactional saving and errors. Display-dimension switches differ from
optional-field collection: hiding a column does not delete data, and disabling collection
cannot create recoverable historical fields.

| Setting | Design default | Range/behavior |
| --- | --- | --- |
| Layered retention | Details 7 days, hours 3 days, days 90 days, weeks 3 years, months 10 years, years lifetime | Adjustable with coarse layers at least as long as fine layers. Archives show layer counts/database size; manual day cutoff or clearing/recollection retains hosts/users/settings. |
| Diagnostics | Expire alongside details | Remove expired diagnostics in the same detail-cleanup transaction. |
| Backups | Latest 2, at most 7 days | May be disabled; retention also applies to oldest data inside backups. |
| Automatic collection | One global interval, initially 1 hour | 0 disables; shorter 15-second–24-hour intervals. Per-source enablement implemented; per-source intervals/fixed times awaited integration in this design entry. See [scheduling rules](scheduling.md) for current status. |
| Background/OS tasks | Autostart and hourly Windows headless tasks disabled | Explicit opt-in; system mode minimum 1 minute, no logged-out/sleep execution; show actual applied state and next run. |
| Statistical time | Initial system timezone; locale week start (zh Monday, en-US/CA Sunday) | Select IANA timezone/Monday/Sunday; preview reconstructable history before changes. |
| Default dimensions | Agent, provider, model, day | Weeks/months selectable. Project/workspace collection off by default; session IDs are for linking. |
| Charts | Top 10 + Other; calls/sessions, total/input/output tokens, stacked cache-read/cache-write/ordinary input, cache-input ratio | Select dimensions/metrics, remember filters, reset defaults; missing fields remain gaps. |
| Capacity notices | Total app data 1 GiB or free disk below 1 GiB | Configurable. Pause backfill/writes and notify rather than silently deleting retained data. |
| Costs/reminders | Estimation/reminders off | Explicit enablement, local versioned rates, separate currencies; no cloud usage/bills/account secrets. |

These numbers are initial design values, rather than claims about the user's current volume.
For D-day finite retention, the cutoff is today's selected-timezone start minus D−1 days,
retaining today and the preceding D−1 local dates. This avoids the prototype's ambiguous
“366 days” actually including an extra day.

Daily summaries outlive details. Expired dates become sealed with original provenance,
timezone, fields and source-selection version. Ordinary rescans cannot append to sealed
days. Backfill requires complete date/source partitions, recomputed and atomically replaced;
without complete inputs retain existing summaries and mark them uncorrectable. Deleting
details then rescanning cannot add the same usage again. Keep cursors/import cutoffs/necessary
deduplication state until no longer needed. Expiry removes identifiable details containing
event time/model; file cursors retain only necessary usage-free position state. Removing
an entire source may also remove that state.

Timezone/new-dimension changes rebuild dates covered by detail. Summary-only history retains
old buckets with explicit notices, rather than silently joining incompatible trends. Consistent
whole-history changes require complete source rescans before replacement. Preview the earliest
recomputable date and unrecoverable historical dimensions before action.

Shorter retention previews affected rows/dates/backups before confirmed database cleanup.
Manual cleanup and clearing/recollection share one background maintenance task to avoid
blocking the window with synchronous IPC. Before commit, cancellation is checked while waiting
for collection, backing up and deleting SQL rows. Accepted cancellation rolls back the whole
transaction, preserving revision, retention position, cursors and summaries without recollection.
A backup alone is a recovery file, rather than successful cleanup. Atomically arbitrate commit
entry against cancellation; after commit starts, reject cancellation and show committed state
without promising to undo completed deletion. Clearing cancellation does not cover recollection
after commit. Keep buttons disabled after accepted cancellation until completion. For rejected
cancellation, distinguish finished work from commit entry; do not claim rollback.

Optional query indexes change neither fields nor schema version; add them on first opening
old databases without requiring clearing. Daily summaries, Agent distribution and daily charts
merge metrics per row, retaining only result groups and active-day sets rather than all input
dimension rows. Weekly/monthly archives retain existing partition replacement, unknowns and
sealed coverage. Detail-coverage indexes only accelerate reading; they cannot replace complete
session-identity sets or change filters, DST or duration samples.

Repeated aggregates may use a per-connection cache of at most four entries, retaining at most
2 MiB of string/vector capacity. That limit excludes allocator overhead and is not a process
memory measurement. Keys include complete dates, granularity, timezone, week start, today,
retention boundaries, source/model/Agent/quality filters and selections. Do not compare version
values across connections. Within one read transaction, check data_revision, PRAGMA data_version
and total_changes. Same-connection writes or other-connection commits invalidate cached results;
failures/oversized results are not cached. Desktop reuses two mutex-protected read-only
connections, with short-lived fallback when busy, and leaves no read transactions open.
Measure cold and warm queries separately; cache hits do not verify cold-query targets.
References: [SQLite data_version](https://www.sqlite.org/pragma.html#pragma_data_version) and
[rusqlite 0.40.2](https://docs.rs/rusqlite/0.40.2/rusqlite/struct.Connection.html#method.total_changes).
SQLite first aggregates day/week/month sessions/durations within accurate local-date bounds;
cross-day DISTINCT still merges source/session identities. Hour selections retain actual
timezone conversion, rather than fixed UTC hours replacing DST. Delete in small batches,
then perform controlled checkpoint/space reclamation. Never automatically delete original
Agent logs. Rescans still honor new retention cutoffs and cannot restore expired usage just
cleared by the user. SQLite logical deletion does not guarantee physical erasure; the UI
cannot promise secure erasure. Users manage their own exported files.

<a id="pricing"></a>

<a id="费用规则"></a>

<a id="费用合同"></a>

### Cost rules

Separate locally recorded amounts, local API-rate estimates and subscription/credit consumption
explicitly attributable to local usage. Hide native amount/credit panels without corresponding
local fields. Known local tokens may still be estimated using applicable rates, without requiring
native amounts. Remote Coding Plan/account balances/bills remain excluded and cannot fill gaps.
Estimates record provider/model, region/channel, effective interval, cache read/write/TTL,
batch/service tier and currency. Price units, model versions/aliases and long-context tiers
must match. Reasoning subsets cannot be charged again when already included in output.
Unknown prices/required token components show “Unpriced”; partial amounts state coverage
without implying complete cost. Total tokens alone cannot establish input/output costs using
an average rate. Never add different currencies directly; manual conversion is an estimate.
Subscription calls' API references are not actual bills; cache savings are not actual refunds.

Public API prices are price metadata rather than remote usage/billing. Retrieval was researched
under [F2](execution.md#f2). Snapshots store URL, retrieval time, effective interval, currency/
unit, version and manual-override references. Refresh failures keep validated cache and show
freshness; offline statistics remain available. Send no local usage/source identities.
Results reference usage revision and price version. Background refresh cannot silently rewrite
existing estimates. Separate occurrence-time prices from simulations at current rates. Without
historical rates, current prices cannot masquerade as occurrence-time prices.

Use the fixed precision/rounding above. The cost engine is implemented
([results](../../validation/desktop-usage/f2-cost-engine.md)); optional online refresh is
implemented/default off with models.dev community data, long-lived raw-response cache,
last-successful-download fallback and official-provider fallback matching
([results](../../validation/desktop-usage/f2-online-refresh.md)). Prefer exact provider/model/
channel rows. Without an exact row, or with unknown provider/channel, a verified official
pay-as-you-go row for the same model may supply an explicitly labeled API price reference,
rather than actual billing. Families only constrain official-provider selection; unknown
versions/subscription-only aliases receive no guessed price. Do not zero-fill unknown buckets.
Prefer user channel, then official global API. Remaining region/channel/currency ambiguity
prevents pricing. Known usage_observation tokens may receive partial estimates; priced-event
count is not call count. Rule repairs recalculate retained unsealed detail once; background
price refresh never rewrites occurrence-time amounts.

Current API references and usage tables cover the same retention range. Select detail or
sealed aggregates by complete source partitions. Unknown archive components stay unknown;
known totals remain in coverage denominators. Without per-call tiers, show lower/upper costs
for known components, rather than choosing a per-request tier using period total input. Sum
integer products before rounding by displayed partition so small costs are retained. See
[pricing requirements](pricing.md) for rules/results. Cross-model references require explicit
user authorization, retaining actual model and substitute price model. Exact-model entries
take precedence; ambiguity never triggers substitution and occurrence-time amounts stay
unchanged. The current K2.8 Preview → K2.7 Code exception and UI label are specified in
[dashboard requirements](dashboard-repair.md).

Spending reminders use user-selected known metrics and notify without blocking Agent execution.
Missing sources limit reminder coverage. Restart/backfill cannot repeat the same threshold notice.

<a id="原型历史迁移"></a>

## Migrating historical prototype data

Do not package previous-draft/data/usage.db into the app/tests. User-selected import first
verifies schema read-only and previews dates/source/field coverage. Migration cannot repair
unverified prototype request_id semantics, lost provider or default-zero fields automatically.
Prefer source reconstruction for recoverable dates. Untraceable history is legacy_unverified,
shown separately and excluded from new-data addition by default. Reimporting the same old
database adds no duplicates; cancellation/failure rolls back the batch. Original files stay
unchanged.
