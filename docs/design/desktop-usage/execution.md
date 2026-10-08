# Execution requirements and delivery order

<a id="执行要求与交付顺序"></a>

Current status and remaining work are maintained only in [Plan.md](../../../Plan.md).
This file retains the applicable delivery requirements; behavior follows the data, scheduling,
pricing and platform designs. Report implementation, synthetic tests, real sources, native desktop
and CI results separately. Passing documentation or synthetic-data tests does not establish other versions
or real environments.

<a id="m0m1基线与统计存储"></a>

## M0/M1: Baseline and statistical storage

Deliver reproducible lockfiles, development/build entry points, three-platform CI and fixed-format
samples. SQLite distinguishes known/unknown values, calls, attempts and cumulative observations.
Commit each batch's events, cursors, parsing state, aggregates and jobs in one transaction.
Updates remove previous contributions; rescans do not add duplicates; recovery after a crash is
idempotent. Make consistent backups and check available space before migrations, rebuilds and cleanup.
Read source databases without checkpointing or repairing them. Record performance, resources,
package size and platform results in the actual environment; language or architecture cannot establish them.

<a id="m1a"></a>

<a id="m1a来源身份与交换"></a>

## M1a: Source identity and exchange

Store stable host ID, hostname and source instance with both original provenance and collection
location. Identity survives restarts, renaming and confirmed migration. Include provenance in event
uniqueness and aggregate partitions; hostname does not prove local origin. Versioned exchange declares
provenance, partition keys, time ranges, revisions and snapshot/increment semantics. Skip duplicates,
replace revisions from the same source, add mutually exclusive partitions and retain conflicts.
Aggregate exchange and [complete normalized detail Merge](detail-merge.md) are implemented. Verify
import previews, complete source days, sealing, retention floors and event/cumulative/summary transactions
separately. Round-trip checks assert nonempty exports before comparing independent expectations.
CSV and charts are not lossless round-trip formats.

<a id="m2-layout"></a>

<a id="m2m5m8逐适配器交付"></a>

## M2–M5/M8: Delivery per adapter

Each Agent has its own directory, entry point, version probing/dispatch and implementations under
versions/. Keep product mappings in that directory; logic shared across Agents needs verified field semantics for each product. Dispatch known
versions through the registry. For unknown/missing versions,
try the latest built-in parser for that input type and retain a compatibility marker when validation
succeeds. Associate source references/checks with each record’s version; the highest database version or empty
sessions cannot verify other records.

Each adapter delivers discovery, probing, incremental reading, field mappings, capability notes
and fixed-format test data. Prefer read-only comparisons of existing same-version details, summaries
and rescans. Running official clients against real models in isolated Podman containers is authorized
this round. The specified Providers and Zed configuration are explicitly authorized; verify protocols,
versions and native files/databases first, and keep credentials out of ordinary configuration and logs.
Retain reasons for missing sources, unsafe reads and insufficient fields; do not fill gaps with remote
billing/APIs. Verify manual roots, environment overrides and platform paths separately. Route managed
roots by file/database format and verify restoration of old registrations through the entire registry's real discover
path. Shared-reader changes require regressions for every actual consumer.

Across source formats, use native shared identity or select one verified partition. Matching timestamps/token
counts cannot establish call identity. Session cumulatives do not become individual model requests.
Explain parent/child spans, host mirrors, failed requests, sampling, retransmission and partial history
independently. Telemetry uses field allowlists and accepts only enabled local instances; reject bodies,
credentials, remote forwarding and account/organization totals. Local loopback does not prove original
provenance. Implement local HTTP authentication, system credentials and conditional recovery under
[receiver authentication](receiver-auth.md). Verify V07–V12/V17/V22/V25/V30 against actual supported file/database formats.

<a id="m8"></a>

The [adapter matrix](adapters.md#扩展覆盖) defines the M8 scope and required samples for each family.
Built-in parsers with format references may be delivered at documentation level without expanding real
acceptance labels. Exclude estimated-token routes; discover and deduplicate both roots after brand
migration. Qoder is a limited probe. Verified Amazon Q/Codebuff versions have no local per-call token
records; exclude iFlow under its shutdown boundary.

<a id="m6m7桌面后台与发行验收"></a>

## M6/M7: Desktop, background collection and release acceptance

Connect the five pages to real queries, refresh, settings, retention, exchange and ten languages.
Charts, tables and selections use the same range and revision. Failed or stale responses cannot replace
the current filters. Manual collection includes all enabled sources; automatic jobs follow source
enablement and deadlines. The same source cannot run concurrently; merge overlapping manual, scheduled
and GUI/headless requests. Store rule timezones independently and handle DST/sleep under the
[scheduling design](scheduling.md). Separate expected/actual Windows task states; leftover triggers cannot
bypass disabled settings.

Native acceptance uses actual Tauri IPC/WebView. Browser mocks cannot replace it. With isolated nonempty
sources/databases, verify settings, queries, export/import, cleanup cancellation, restart, offline behavior,
languages, zoom and accessible names. Measure all-process resources on actual hardware, first-screen
percentiles and release package size separately. Record native Windows, three-platform CI, WSL builds
and actual Linux GUI acceptance separately. Windows installation lifecycle and isolated Podman's Linux
packages/GTK/WebKit/FUSE are authorized this round; verify the [lifecycle requirements](installation-lifecycle.md).
macOS desktop or specific hardware is not required. Additional DPI, complete screen-reader coverage,
host login/logout and OS wake-up are no longer required this round; retain existing results.
The main branch, signing/notarization and Release user instructions were reported complete and are no
longer active tasks.

<a id="f1后续-ide-支持"></a>

## F1: Subsequent IDE support

Investigation is authorized this round. Move items without a testable installation or local usage files/database
out of the active plan; retain product/variant limits in the [adapter matrix](adapters.md) without claiming
the product is unsupported. Junie CLI and built-in Zed belong to M8; JetBrains Copilot's manual OTel route belongs
to M9; JetBrains's own AI Assistant remains F1. When a host runs a supported external Agent, count its
underlying source without claiming support for the host's built-in Agent. Future delivery provides
versioned adapter source references or specific limitations.

<a id="f2"></a>

<a id="f2价格与费用"></a>

## F2: Pricing and costs

The [pricing design](pricing.md) governs versioned price lists, channels, currencies, tiers, TTL and coverage
markers. Estimation and online refresh default to off. Separate prices at occurrence from current-price
simulation; background refresh cannot rewrite historical estimates. models.dev fetches only public pricing
metadata without local data; retain the designed long raw-response cache and failure fallback. Exact prices
take priority; missing prices may use an unambiguous official API reference for the same model without
inferring actual payment. Disabled-by-default [usage and cost alerts](budget-reminders.md) are implemented.
This round's specified endpoints are Coding Plans and cannot use pay-as-you-go estimates as actual cost.
Verify amount arithmetic and V29 anomalies/partial pricing/fallback scenarios independently.

<a id="f3"></a>

<a id="f3多语言"></a>

## F3: Languages

All ten languages use the same statistical rules and stable English diagnostic codes. Format numbers
and dates with user settings. Synchronize new interface keys across every language and check keys and
interpolation placeholders. V31 native language/layout acceptance accompanies M6.

<a id="变更回退与完成记录"></a>

## Changes, rollback and completion records

Keep changes independently reviewable and preserve user edits. Commits, pushes and CI on the separate
test branch were authorized for that batch; this does not automatically authorize merging, deployment
or publication. See Plan.md for authorization scope/current results. A parsing failure stops subsequent
reading of that source and retains old results. Failed migrations restore a consistent backup; old programs
refuse to write a newer schema. Explain recovery conditions before irreversible cleanup and retain source
files. Record actual cwd, commands, versions, environment, exit codes, quantities, results and gaps.
Update progress only after completing and recording the corresponding checks. Temporary artifacts belong
only under repository-root build/. Current designs do not repeat the narrative of every historical round.
