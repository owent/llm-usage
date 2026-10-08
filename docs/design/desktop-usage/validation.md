# Tests and acceptance requirements

<a id="测试与验收要求"></a>

<a id="测试与验收计划"></a>

This document preserves acceptance criteria. Track progress in [Plan.md](../../../Plan.md)
and [current acceptance](../../validation/desktop-usage/current-acceptance.md).
Specialized adapter-version, pricing and source-exchange results are in the
[validation directory](../../validation/desktop-usage/). Implementation does not establish
acceptance: record GUI, native-platform and real-source gaps separately. Every item needs
its version, dataset, command, exit code and actual result; checking an ID alone is insufficient.

<a id="测试层级"></a>

## Test levels

1. Pure-function tests: field mapping, mathematics, time, identity and lifecycle, using independent manually calculated expectations.
2. Adapter format tests: fixed-version redacted test data through actual reading, parsing, normalization and queries.
3. Local integration: real temporary SQLite, incremental/rewritten files, WAL, transaction interruption and migration, beyond database mocks.
4. Desktop acceptance: actual Tauri IPC, WebView, configuration and refresh; component tests do not replace native desktop checks.
5. Authorized real-source comparisons: compare local agent statistics/session exports and record differences in values and coverage.
6. Performance and releases: release artifacts, all application processes, actual installation/upgrade/uninstall/recovery.

Choose actual commands from root package.json. verify runs static, unit and format checks;
test:browser uses simulated IPC; test:headless uses the real executable and SQLite;
test:desktop uses Windows WebView2 with actual IPC. Build first and isolate source environments
for the latter two. Explicit system-task API round trips do not establish OS-triggered startup.
Most checks at the first four levels need no model requests.

<a id="验收用例"></a>

## Acceptance cases

| ID | Scenario | Required observation |
| --- | --- | --- |
| V01 | Provider cache inclusion, reasoning subset and cache-TTL subsets | Independently calculated equations; no duplicated tokens; distinguish missing, zero, invalid negative values and overflow |
| V02 | Two valid requests with identical tokens/model/time; duplicate final for one request | Count the former twice and the latter once; corrections replace old contributions; reordered input cannot silently overwrite data |
| V03 | Success, failure, cancellation, transport retries, no usage, user/tool messages and cumulative values | Separate calls/attempts/messages; unknown tokens stay unknown; no request count without verified calls |
| V04 | Midnight, leap day, ISO year-boundary weeks, Sunday week start and DST 23/25-hour days | Correct half-open ranges, week year and local dates; distinguish repeated hours |
| V05 | Model changes, identical names across providers, unknown values and changed aliases | Future settings cannot identify historical models; totals include unknown models; aliases preserve original records |
| V06 | Daily to weekly/monthly aggregation, weighted ratios, distinct sessions, partial periods and percentiles | Do not average percentages/P95 or sum daily distinct counts; expose sample range and gaps |
| V07 | JSONL partial lines, split UTF-8, oversized lines, BOM, invalid lines, new directories, rotation and equal-length replacement | Preserve complete events; partial lines cannot advance cursors; detection goes beyond file length; errors never leak bodies |
| V08 | File/OTLP retransmission, cumulative-metric resets, both token histogram forms and sampling | Repeat input adds no duplicate contributions; distinguish sum/count; do not distribute unknown cross-day intervals; sampled totals are incomplete |
| V09 | Termination after reading, event writing, cursor writing, before aggregation and after commit | Restart/replay produces the same result; no cursor-only commit losing data or event-only commit doubling it |
| V10 | Parent/child sessions, inherited forks, host copies, duplicate source databases and auxiliary calls | Explain coverage sets; merge only with explicit identity relationships; otherwise select a primary source to avoid duplication |
| V11 | Actual concurrent SQLite writers, busy, uncheckpointed WAL, read-only access and schema changes | Consistent reads without modifying the source database/journal; retain old results when safe reads fail |
| V12 | Refresh today, new/corrected yesterday data, double refresh, one source failing and wake from sleep | Actually collect; daily/weekly/monthly results share a revision; expose failed-source status while others continue; no demo fallback |
| V13 | Changed timezone/week start, deleted details, project-collection switch and hidden columns | Preview rebuildable ranges; old summaries cannot invent new groupings; separate display settings from deletion |
| V14 | Shorter/longer retention, finite/unlimited changes, cancelled cleanup, date boundaries and another full scan | Preserve exactly the specified local days; expired data cannot reappear; source logs stay unchanged; archived days cannot accumulate twice |
| V15 | Disk reclamation, readers blocking checkpoints, full disk and expired backups | Account for main DB/WAL/backups; bounded retries and notices; backups cannot bypass maximum retention; no silent loss |
| V16 | Repeated old-database imports, overlapping ranges, corrections, cancellation, migration failure and old apps opening new schemas | Repeat imports add no duplicates and can roll back; retain legacy labels; recover migration failures; old apps refuse writes |
| V17 | Verified versions; unknown/missing versions with compatible structure; optional additions; structural/semantic breakage and partial usability | Use known mappings; try that agent's latest built-in parser for unknown versions; valid data counts with a compatibility label; diagnose incompatible data without presenting failure as successful zero records |
| V18 | Consistent table/chart filters, keyboard/scale/light/dark, absent/partial data, usage alerts and costs | Chart values match queries; explicit units; keep currencies/cost categories separate; restarts do not resend alerts |
| V19 | Actual installation, missing runtime, offline use, restart and disabling background/tray operation | Core features work offline; clear missing-WebView notices; no hidden resident process; settings restore correctly |
| V20 | 1M/10M events, 366 days/multiple models/sources and a few huge sessions | Measure 10-minute GUI idle means/peaks and first million-record import peaks under resource requirements; report all processes, headless and working set separately; record throughput, query percentiles and DB/WAL size; no unbounded whole-file loading |
| V21 | Release package size, startup, refresh, idle CPU and uninstall/upgrade | Meet architecture.md targets or record unmet reasons; include WebView children and runtime assumptions |
| V22 | OTLP authentication/rate limits/decompression bombs, CSV injection, path escape, malicious source strings and failed local import | Never execute external text or persist bodies/credentials; no unauthorized access; interrupted local imports recover |
| V23 | Global/per-source intervals/fixed times, disable/resume, overlapping manual/watch/scheduled triggers, clock rollback, DST and sleep | One scan per source, merge pending triggers; disabled automatic collection never reads; missed times cause at most one catch-up per source; correct next run |
| V24 | Windows task registration/disable failures, ordinary users, GUI/headless competition, exit/logout/upgrade/uninstall | Expose actual state; no password/elevation requirement; one writer without launching WebView/agents; disabled settings also block residual triggers; clean only owned tasks |
| V25 | Local WSL/containers, remote synchronized files, saved cloud bills, account reports and loopback forwarding/forged host | Explicitly authorize/deduplicate local instances; exclude remote/unknown origins with reasons; no outbound remote usage/API calls or implicit environment startup |
| V26 | GitHub Windows/macOS/Linux matrix, lockfiles, release artifacts, test instrumentation and permissions | Actual builds/tests on all three OSes with separate status; Windows 11 hardware acceptance separately; no test listener/personal data in artifacts or automatic release |
| V27 | Local WSL 2 Linux builds, copies, missing dependencies, AppImage and WSLg availability | Trace revision/uncommitted differences/environment; separate compilation/tests/packaging/GUI; never share databases across systems or equate WSL builds with native desktop acceptance |
| V28 | Host rename, same-named hosts, moved roots, copied DBs, deleted details, missing old provenance, reimport, same-source revisions/partial snapshots/conflicts | Stable provenance participates in uniqueness/history partitions; export original ownership; importing hosts cannot replace it; duplicates add nothing, accepted revisions replace, exclusive sources add; old mixed summaries cannot invent splits; failures retain old results |
| V29 | Pricing channels, model aliases/versions, cache TTL, reasoning inclusion, tiers/batch, effective boundaries, currencies, missing prices and offline/update failures | Reviewable price sources/scope; fixed token/rate cases with independent expectations; unknown stays unknown; expose partial pricing coverage and deterministic rounding; reproducible history without usage uploads or treating subscription data as actual payment |
| V30 | Adapter-directory migration, historical versions, mixed known/unknown roots, latest-parser fallback, format conflicts, shared-logic changes and old-cursor recovery | Independent agent directories with internal version implementations; correct dispatch/fallback and traceable compatibility; identities/usage/recovery unchanged for verified versions; repeats add nothing; preserve historical regressions |
| V31 | UI language changes, missing-key fallback, negotiation, localized numbers/dates/units and separate export formats | F3 localization rules; diagnostic default-language fallback; calculations independent of language; explain CSV and UI formats separately |

M1a verifies V28 provenance persistence, migration and merge decisions; M6 verifies actual
exports. Aggregate packages contain the source registry and daily/period/hourly data;
[detail packages](detail-merge.md) also contain complete events and cumulative records.
Both imports are implemented; independently test transactions, revisions, unknowns,
complete days, archives and native round trips. Cross-device collection remains excluded.
V29 uses fixed price cases P1–P6, independent expectations E1–E8 and abnormal cases A1–A10;
see [cost engine](../../validation/desktop-usage/f2-cost-engine.md) and
[online refresh](../../validation/desktop-usage/f2-online-refresh.md).
Check current API references separately from observation-time estimates, which require a
known provider channel. [Usage/cost alerts](budget-reminders.md) are implemented: verify
default-off behavior, user/period isolation, exact amounts, unknown coverage and restart deduplication.

<a id="adapter-versions"></a>

<a id="v30适配器目录与历史版本回归"></a>

### V30: Adapter directories and historical versions

M2 migrates existing single-file adapters. M3–M5 and later F1 adapters follow the
[directory specification](architecture.md#adapter-layout). Record each scenario separately;
directory existence or compilation does not establish version compatibility.

- Check each agent's directory, stable entry point, dispatch and internal implementations; product-specific mapping cannot remain in root-level single files.
- Compare fixed samples of previously verified versions and committed cursors/parser context before/after migration: identity, events, summaries
  and increments. Moving implementations creates no new sources, duplicates or cursor resets; old public entries/examples/tests remain usable.
- Verify every actually supported release's dispatch. Historical files with different versions in one root select their matching implementations.
  Unknown/missing versions first use that agent/input type's latest built-in implementation rather than rejection solely for an unlisted version
  number.
- Test unchanged formats with unlisted versions, recognizable agents without versions and optional additions. Actual
  detection/scanning/storage/query results must match independent token expectations and show unverified compatibility. Detection, scanning and
  capability declarations use the same policy.
- Inject invalid required types, contradictory token inclusion, unrecognizable usage records and competing format matches. Preserve independently
  valid partial data with coverage labels; stop calculations requiring unknown cumulative baselines. Failed batches cannot advance cursors or
  overwrite old results. Invalid files leave independent valid files usable; skipping invalid lines cannot establish complete success or
  successful zero records.
- Compatibility remains queryable after restart; daily summaries/archives/exports retain parsing references. Repeat scans add nothing. Parser
  updates, source changes or explicit rescans allow retries; later dedicated implementations replace earlier contributions rather than append
  them again.
- Regress all supported historical versions when adding a version, and all actual consumers when shared components change. Trace the support
  matrix, sample descriptions and parser_version; never delete old samples to conceal regressions.

For agents with one verified version, first check migration/current behavior. Unverified
historical versions remain pending. Synthetic dispatch tests establish dispatch logic,
not that agent's actual historical formats.

<a id="固定数学样本"></a>

## Fixed mathematical cases

Use these values in unit/end-to-end test data. Hard-code independent expectations in the
test-data instructions; never calculate expectations using the implementation being tested.

| Input | Expected result |
| --- | --- |
| Uncached input 100, cache read 800, cache write 100, output 100 | Total input 1000, total tokens 1100, cache-input ratio 80% |
| Total input 1000 including cache read 800; output 100 including reasoning 40 | Total tokens 1100; no extra cache/reasoning addition; uncached input 200 only when absent cache creation is established |
| A input 100/cache 90; B input 900/cache 90 | Combined ratio 18%; individual ratios 90%/10%; no averaging to 50% |
| Input 100, unknown output | Known input 100; output/complete total unknown; no complete total_tokens=100 |
| One request corrected from usage 100 to 80 | Current value 80, one call; neither 180 nor MAX=100 |
| Two stable request IDs each with usage 100 | Total 200, two calls; identical content cannot deduplicate identities |
| DSH same attempt streams 80 then final 100; retry final 40 | Two verified attempts total 140; model_call identity follows verified adapter rules |
| Cumulative 100→150→150, explicitly new process 20 | Interval increments 50/0/new-process 20; preserve first 100 in its original interval, without assigning it all to today |
| Same session active on two days | Weekly/monthly DISTINCT sessions=1, active days=2 |
| Hermes two-day cumulative row: token=1000/api_call_count=3 with first_seen/last_seen only | Preserve source interval summary; no three invented model_call rows or all 1000 assigned to the final day; repeats add nothing |
| Hermes main-model cumulative 100, independent auxiliary task cumulative 20, sessions main loop also 100 | Total 120 only with verified exclusive coverage; no 220, duplicate model/session addition or omitted auxiliary task |

Also cover near-i64 limits, values above JS safe integers, millisecond/second confusion,
repeated DST hours and null propagation.

<a id="适配器真实核对"></a>

## Real adapter comparisons

Read-only extraction of real local agent data for implementation is authorized; follow
the [minimum extraction procedure](implementation-readiness.md). M0 may extract required
redacted fields first, then compare end-to-end real sources after parser checks pass.
F1 inspection is authorized this round. Items lacking installation/local data leave the
active plan while the matrix retains their limits and unimplemented status.
Extra DPI, complete screen-reader, host login/logout and OS-wake requirements were cancelled
this round; Plan.md owns active acceptance.

Each released adapter fixes at least one actual version; multiple-version support requires
schema samples for each. Cover single calls, tool loops, model changes, subagents,
failure/cancellation, restart recovery and auxiliary requests. Record unavailable scenarios
for that product; another product's data cannot substitute.

Compare agent-local statistics using identical timezone/range/local instance/model/units.
Investigate source write delay, log retention, hidden auxiliary requests and cross-device
data in the source UI before explaining differences. Matching tokens does not establish
matching request/cache values: accept each field independently. Prefer existing local
history; do not automatically issue paid requests or switch to account APIs for testing.

<a id="调度与平台补充验收"></a>

## Scheduling and platform checks

Use controllable clocks for V23/DST. V24 uses actual Windows 11 ordinary-user test tasks:
mock registration cannot establish working OS tasks. Operate only the application's test
namespace and confirm cleanup. V25 uses explicit local/cross-device/unknown-origin synthetic
data; audit outbound behavior during startup/manual/scheduled/import paths. Separate build
dependency downloads from collection-time network activity; agents calling cloud models
are not collector outbound usage traffic.
V26 requires actual native builds and Rust/format/SQLite/scheduling results on all three
platforms; record missing GUI automation separately. Windows installation/tasks/resources
still require actual Windows 11 checks.
On 2026-10-05 the user removed macOS desktop/specific-hardware requirements and accepted
isolated WSL/Debian Podman GTK/WebKit/IPC for Linux GUI/package lifecycle. Host login/logout,
tray/notifications and other distributions retain separate scopes. Extracted execution
cannot establish successful FUSE mounting. V27 is optional local validation: report unavailable
environments as unexecuted without deleting Linux CI or claiming it passed.

<a id="不作为通过依据"></a>

## Insufficient acceptance results

- Compilation, schema existence or discovered directories do not establish correct usage.
- Prototype READMEs, third-party statistics mappings and search snippets cannot replace upstream format references.
- Fixed local samples do not verify every release, IDE, real service or production coverage.
- Empty Tauri-shell package sizes cannot represent the full chart/database/collection/telemetry product.
- Markdown lint checks formatting and cannot establish application acceptance.
