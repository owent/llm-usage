# Local AI usage desktop design

<a id="本地-ai-用量桌面客户端设计"></a>

Current implementation and remaining acceptance work are in [Plan.md](../../../Plan.md).
This directory maintains the current design. Public documentation, fixed-version source
and actual implementation provide distinct references; sample, native desktop and CI results
are recorded separately. See the [adapter matrix](adapters.md) for capabilities/limits and
[implementation prerequisites](implementation-readiness.md) for the authorized read-only scope.
[Telemetry setup](copilot-otel.md), [dashboard interactions](dashboard-polish.md) and
[pricing](pricing.md) define their respective behavior. Local HTTP source authentication,
credentials, failure recovery and revocation are in [receiver authentication](receiver-auth.md).

<a id="产品目标与边界"></a>

## Product purpose and scope

Show observed local usage by Agent and model for today and historical periods: where
consumption occurs, how cache usage changes, and which sources have incomplete collection.
Statistics use local data, requiring no account login, cloud synchronization, external
database or additional language-model calls.

Local means logs, databases, usage files and explicitly enabled telemetry produced by
Agent instances on this computer. Local records of calls to cloud models qualify; models
need not run offline. Local WSL/container instances require explicit roots, instance or
distribution identity and deduplication; do not discover or start these environments automatically.
Exclude SSH/remote gateways, network shares, cloud bills and enterprise/cross-device account
reports. Downloading remote exports, syncing remote sessions or using a cloud-synced directory
does not establish local origin. Local session exports must retain verifiable local attribution
and native detail. Exclude unattributed records from totals and explain why. Missing data
does not mean zero usage; source logs do not establish that all model requests were recorded.

Windows 11 x64 is the first desktop target; GitHub CI retains Windows/macOS/Linux build and
test jobs. Local WSL 2 Linux builds, WSLg smoke tests, CI builds and real desktop acceptance
have separate results. Harness Agent was identified as [Hermes Agent](https://hermes-agent.nousresearch.com/),
with A24 references. The user authorized read-only extraction of minimal, redacted native
Agent data during implementation. This round also authorized F1 IDE investigation; missing
installations/local formats leave active work, while limits remain in the matrix. Built-in
Zed and Junie CLI belong to M8. Zed 1.22.0 has native two-model/cache samples for the specified
external provider, verifying neither hosted service nor other providers. Extensions/telemetry
with local field references continue in their assigned stages; see the matrix.

<a id="主要决策"></a>

## Main decisions

| Decision | Purpose and constraints |
| --- | --- |
| Tauri 2 with a Rust backend | System WebView; local backend owns file/database access. Measure package size, all-process resources and platform behavior in the actual environment. |
| SQLite, one writer and WAL | Local transactions, unique constraints, revisions and summaries without a database service. Retention/recovery never modifies source databases. |
| Svelte/TypeScript, selective ECharts imports and SVG | Filters, settings and charts. Artifacts/resources have measurements; no pure-TypeScript comparison was run, so no performance difference is claimed. |
| Local history, explicit telemetry and local session exports | Verify each source's version, fields, lifecycle and local origin independently. Unverified IDE formats move to F1; no remote usage API. |
| Separate events, cumulative values, interval reports and quotas | Message rows, histogram sample counts and credits are not requests or tokens. Check samples per field. |
| Stable host/source identity and versioned exchange | Hostname is descriptive. Aggregate and complete normalized-detail packages import separately, retaining revisions, conflicts and archived partitions. |
| Built-in compiled adapters with version directories | Ship through application upgrades and retain historical implementations. Unknown versions try compatibility reading and keep unverified-version markers after successful validation. |

See [architecture](architecture.md) for alternatives and [research](research.md) for source references.

<a id="页面与交互"></a>

## Pages and interactions

M6 implements one main window and five navigation pages: Overview, Trends, Sources, Details
and Settings. Overview/Trends panels can be hidden and dragged into order. Automatic collection
has one global interval, initially one hour; 0 disables it. Each source can be enabled/disabled
and use an interval/daily/weekly schedule. Login startup and Windows background tasks default
off. System tasks check saved due rules every minute, reporting requested and actual state
separately. Disabling automatic collection prevents scans. Settings explains collection
with a closed window, exited process, sleeping computer or signed-out user; see [scheduling](scheduling.md).

| Page | Contents | Operations and failure feedback |
| --- | --- | --- |
| Overview | Today summary, hourly charts, model/Agent distributions and tables; historical calls/sessions and token charts | Today, last 2 calendar days, 7/30/365 days; separate today/history drag selections update summaries/distributions. Show collection progress and failure time. |
| Trends | Calls/sessions, daily/weekly/monthly/hourly tokens, API price references, model/Agent distributions and model table; activity calendar and weekday distribution last | Click/drag selects a shared range for summaries, distributions and model table. Reset clears selection. Calendar/weekday panels show the full range. |
| Sources | Discovery/enabled state, versions, local origin, fields, last/next collection, errors and assigned user | Add roots, enable/disable, collect now, rescan, remove, reassign users and switch user; explain limitations. |
| Details | Paginated usage: time, Agent, model, category, input/cache read/output/total, duration and session; no prompts, responses or tool output | Agent/model filters, previous/next page and page jump. |
| Settings | Timezone/language/week start/interval; tiered retention, per-tier statistics and manual/all-data cleanup; startup/background; host alias; export/import | Preview schedules/configuration effects. Explicitly enable/disable Windows tasks and report actual state on failure. |

Charts include calls/sessions; total tokens, stacked cached/uncached input, output and weighted
cache-input ratio; model/Agent pies with tables; today's hourly usage; activity calendar and
weekday distribution. Cost/error/latency charts depend on source capabilities. Requests and
tokens do not share one numeric axis; ratios are not stacked with counts. Every chart provides
a readable table, keyboard access, both themes, color-accessible palettes and explicit units.
Icons/static assets are specified in [asset guidance](../../../desktop/assets/README.md),
with local previews. UI/tray/empty-state interactions have separate M6 desktop checks.

The implemented M6 layout illustrates information groups; panels remain hideable/reorderable:

```text
[Overview] [Trends] [Sources] [Details] [Settings]   [Time range] [Refresh today] [User]
Input (including cache) | Output | Total tokens | Cached | Uncached | Cache-input ratio
Today's hourly chart, model/Agent pies and tables (Overview: today)
Calls/sessions, tokens, activity calendar, weekday distribution (Overview: history/Trends)
Details: time / Agent / model / category / input / cache read / output / total / duration / session
```

<a id="建议增加的统计"></a>

## Additional statistics

| Statistic | Purpose | Data requirements and limitations | Stage |
| --- | --- | --- | --- |
| Cache read/write and weighted hit ratio | Explain high input usage and cache reuse | Cache creation belongs to total input; unknown fields remain unknown | First version |
| Per-call token average, P50/P95 and large requests | Identify large context/high-consumption calls | Complete per-request data only; exclude session cumulative records | First version, by capability |
| Project/workspace and primary/sub-agent/auxiliary shares | Identify title generation, compaction and background usage | Project grouping defaults off; parent totals and child events must not double count | First version, by capability |
| Retries/errors/cancellation and known tokens on failed calls | Explain consumption growth and reliability changes | Request lifecycle/telemetry required; success-only logs cannot imply 100% success | M5 |
| TTFT, duration P50/P95 and output speed | Describe waiting and service changes | TTFT needs first-token time; speed uses the actual generation interval, never file times | M5 |
| Estimated/reported costs and locally attributable credits | Manage usage and compare models/cache | Local usage with versioned prices; ambiguous channels receive no price; estimates are not bills | F2; estimation/online refresh default off |
| Period comparisons, usage/cost alerts and trend anomalies | Identify sustained increases or daily spikes | Today compares equal elapsed time yesterday; daily/monthly token or single-currency estimate alerts default off; explain incomplete coverage | M6/F2; [alert rules](budget-reminders.md) |
| Freshness, parse errors and field completeness | Explain and inspect totals | Coverage describes discovered sources/observed records, never estimated unrecorded calls | First version |
| Sessions, active days and activity calendar | Review usage patterns | DISTINCT session identities; active dates are not work duration or productivity | First version |

Do not derive code quality, saved hours, personal performance or model rankings from tokens.
Carbon, GPU time and true internal cache-hit probability lack required data and have no product
metrics. The UI calls cache_input_ratio “Cache hit rate”; its formula remains cached input
as a fraction of input. See [metric formulas](data-contract.md#metrics).

<a id="需求映射"></a>

## Requirement mapping

| User requirement | Design reference | Acceptance checks |
| --- | --- | --- |
| Refer to previous-draft | [Prototype review](research.md#prototype), retaining useful groupings/chart interactions | Preserve prototype; audit historical migration separately |
| Model statistics and summaries | [Data rules](data-contract.md), separating model/provider/Agent | V01–V06, M1/M6 |
| Persist host/source history and support export/import/Merge decisions | [Provenance](data-contract.md#provenance), [detail merge](detail-merge.md) | V28, M1a/M6; aggregate and complete normalized-detail exchange implemented |
| API price snapshots and token estimates | [Cost rules](data-contract.md#pricing), [pricing](pricing.md) | V29/F2; check observation-time estimates/current API references separately |
| Languages and localized formats | [Localization research](execution.md#f3) | V31; F3 research and M6 implementation |
| Refresh today | [Collection](architecture.md#refresh) | V07–V12, M2/M6 |
| Scheduled collection | [Scheduling/lifecycle](scheduling.md) | V23/V24, M1/M6 |
| Local only; attempt all Agents in stages | This page's scope and the [matrix](adapters.md) | V17/V25; per-adapter M2–M5/M8 checks; unverified IDE formats remain F1 |
| Use native local data; defer IDEs; confirm platforms | [Implementation prerequisites](implementation-readiness.md) | Permission/stages clear; preparation passed, execution recorded separately |
| Windows first, three-platform CI and WSL builds | [Platform/CI](platform-ci.md) | V26/V27, M0/M7 |
| Windows/Linux Podman installation; no macOS desktop/specific hardware requirement this round | [Installation](installation-lifecycle.md), [native source preparation](implementation-readiness.md) | [Lifecycle checks](../../validation/desktop-usage/installation-lifecycle.md), [container sources](../../validation/desktop-usage/container-sources.md); host integration checked separately |
| Daily/weekly/monthly summaries | [Time rules](data-contract.md#time) | V04–V06 |
| Other meaningful statistics | The capability-dependent table above | V01–V06, V18 |
| Retention/grouping settings | [Settings rules](data-contract.md#settings) | V13–V16, M6 |
| Charts | Page specifications above | V18, native desktop checks |
| Domestic/international Agent extensions | [Tool matrix](adapters.md), [extended coverage](adapters.md#扩展覆盖) | Per-adapter M2–M5/M8 checks; F1 is not mandatory first-version acceptance |
| One directory per Agent, historical versions within it | [Adapter layout](architecture.md#adapter-layout), [migration](execution.md#m2-layout) | V30, M2–M5; reused when F1 implementation begins |
| Unknown versions try the latest parser | [Compatibility rules](architecture.md#unknown-version) | V17/V30, M2–M5; reused for F1 |
| Small local database | [Database decision](architecture.md#database) | V11/V15/V20 |
| Resource-efficient desktop | [Resource targets](architecture.md#budgets) | V19–V21, actual release artifacts |
| Continue implementation after design | [Plan.md](../../../Plan.md), [implementation scope](implementation-readiness.md) | Current work/remaining conditions in Plan; original research retains historical scope |

<a id="文档职责"></a>

## Document responsibilities

Plan.md maintains unfinished work; execution.md defines outputs/order; data-contract.md defines
statistics; adapters.md defines supported tools; research.md records sources; validation.md
defines stable acceptance criteria. current-acceptance.md indexes current results. Detailed
records preserve environment, commands, first failures and historical scope. Update the relevant
design before changing behavior. Do not duplicate formulas or contradictory support statements.
