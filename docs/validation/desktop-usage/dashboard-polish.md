# Dashboard interactions and query-performance checks

<a id="看板交互与查询性能验证"></a>

2026-10-02, Windows 11 x64; user-authorized second dashboard repair. References:
[interactions](../../design/desktop-usage/dashboard-polish.md),
[telemetry](../../design/desktop-usage/copilot-otel.md),
[pricing](../../design/desktop-usage/pricing.md). Preserved all earlier user/first-round changes.

<a id="实施结果"></a>

## Implemented results

| Reported problem | Change and verified result |
| --- | --- |
| Oversized overview notice | One status line/two actions; required export instructions in heading tooltip/details. Height below 70 px in Edge at width 1440 |
| Still reports five items after one-click setup | Count only missing and configurable; configured-awaiting-data/valid data/conflicts separate; old batch text cannot override live status |
| Details need manual checks | Read-only inspection of actual outputs; existing valid calls verified automatically; absent outputs wait, unusable samples keep being checked; policy conflicts separate from data results |
| Extra configuration blank lines | JSONC insertion reuses line endings, preserves comments/BOM/LF/CRLF, repeated merges stable; existing whitespace retained |
| Missing cost details | One query returns occurrence-time/current-reference estimates per provider/model, currency subtotals/daily amounts; currencies separate, history unchanged |
| Narrow model table | Full-width today/trend tables, two model cost columns, bottom statistics/currency totals, wrapping headers; screenshots/scrollWidth confirm no horizontal overflow |
| Missing cost curves | Daily/weekly/monthly occurrence-time estimates per currency, optional per-model; hourly usage retains daily costs, unknown amounts stay unknown |
| Slow weekday distribution | Found synchronous cost requests/full-price scans per event; background queries, model candidate cache, daily SQL grouping and early day/hour/detail filters |
| Crowded charts | Full-width usage/calls/weekday charts, two half-width pies, size-dependent donut with separate bottom scrolling legend; collapse cost curve without amounts |
| Selection leaves summary unchanged | Points/x-axis labels/dataZoom update top totals; actual dates/local hours, deduplicated sessions across periods, stale replies discarded, full-range reset |

<a id="本机只读核验结果"></a>

<a id="本机只读证据"></a>

## Native read-only results

No real IDE configuration/agent outputs/user databases changed; no model calls. Scripts,
redacted counts/query plans/performance logs under ignored build/dashboard-polish/. Queries
use the first-round isolated database copy; raw native data excluded from version control.

| Local target, profile identities omitted | Configuration | Data result |
| --- | --- | --- |
| Copilot VS Code primary | configured | verified, 30 valid records |
| Copilot Agent Host | configured | waiting, 0 records |
| Copilot Profile | configured | waiting, 0 records |
| Copilot Profile Agent Host | configured | waiting, 0 records |
| Codex | missing | waiting, 0 records |

Expected overview: one setup item, three configured-awaiting-data, one verified. Do not
label all targets as needing setup. No original IPC response from the user's old process,
so exact old target states remain unknown. Valid samples do not establish complete history
or other unidentified clients. Shared receiver files checked by client event identity;
indistinguishable VS Code profiles sharing a file cannot each claim verification. External
endpoints retained; unmatched local outputs remain awaiting association. Old data cannot
erase configuration conflicts.

Primary export about 14.1 MB: valid calls near the start, other logs at the tail. Check the
start too when tail has no match, avoiding false “no valid data”. Each sample at most 2,000
lines, tail 2 MiB/start 8 MiB, line 256 KiB, read time 200 ms. Entire real read-only check
about 0.72–1.50 seconds, slower during concurrent builds, on a background thread. Bounded
cache for unchanged-file results; appends invalidate it. Page checks every 30 seconds.

<a id="性能与查询计划"></a>

## Performance and query plans

Same copy: usage_events 3,369 rows, daily_usage 22/hourly_usage 60/period_usage 4/
daily_cost_usage 20. Asia/Shanghai, 2026-09-03 through 2026-10-02. Rust debug, three
runs of the same example. These local measurements do not promise other scales/release P95.

| Query | Before | After |
| --- | --- | --- |
| Costs including current-reference calculation | 1,950–2,162 ms | 106–116 ms |
| Daily/weekday underlying query | Warm 0.71–1.31 ms | Warm 0.54–0.61 ms; first about 10 ms |
| Summary including sessions/duration | 35–41 ms | 34–38 ms |

Weekday query itself did not take seconds. Old cost requests scanned all price rows in a
synchronous Tauri command, blocking other UI requests for about two seconds. Costs/summary/
daily distribution now use read-only connections through spawn_blocking. Panels/tables/curves
share one cost response, rather than one API request per model row. Candidate caching retains
all providers/snapshot preference/effective dates/context tiers, without caching an estimate's
conclusion. Binary search assigns cost dates to periods instead of scanning every period per
model/day.

Read-only EXPLAIN QUERY PLAN, Python SQLite 3.50.4:

| Query path | Plan |
| --- | --- |
| Date-bounded daily summaries/filters | SEARCH daily_usage, primary-key prefix tz_version/local_day |
| Date-bounded hourly summaries/filters | SEARCH hourly_usage, primary-key prefix tz_version/local_day |
| Sessions/duration/live priced details | SEARCH usage_events, idx_usage_events_occurred, both time bounds |
| Daily model costs | SEARCH daily_cost_usage, idx_daily_cost_day; temporary B-tree for GROUP BY |
| Archive/daily revision comparisons | SEARCH period_usage, idx_period_usage_range; correlated SEARCH daily_usage with date-key prefix |

No unbounded full-table scan in this copy. Temporary grouping B-tree does not imply missing
comparison indexes; no new index/migration here. Daily distributions read daily tables, without
summing original messages. Archives that cannot be assigned to days keep coverage gaps.
Agent/provider/model/instance filters for daily/hourly aggregates and retained details execute
in SQL. Weekly/monthly archive comparisons remain period-bounded with existing replacement/
revision rules. Checked SEARCH/ranges/prefixes against [SQLite plans](https://www.sqlite.org/eqp.html)
and [query planning](https://www.sqlite.org/queryplanner.html). No ANALYZE/index/database clearing/
historical rewriting on the original database.

<a id="验证命令与范围"></a>

## Commands and scope

Node.js 24.21.0, Rust/Cargo 1.98.1, installed Microsoft Edge; dependencies from workspace lockfiles.

| Repository-root command | Result |
| --- | --- |
| cargo test --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core --test dashboard_polish | Exit 0; three tests: hourly range/deduplication/unknowns, DST repeated hours, byte limit/partial-line continuation |
| cargo test --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core --test pricing_v29 | Exit 0; ten tests, added model/day/currency amount consistency, multiple currencies/filters/archives/official fallback |
| cargo test --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-desktop telemetry_setup | Exit 0; 37 pass, two ignored; added newline/verification/cache/conflict/shared-profile cases |
| cargo test --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-desktop local_export_verification -- --ignored --nocapture | Exit 0; read-only native output: five target states/30 valid records |
| cargo run --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core --example benchmark_dashboard | Exit 0; three native-copy timing runs |
| python build/dashboard-polish/query_plans.py | Exit 0; read-only plans/row counts |
| npm run test:browser | Exit 0; Edge simulated IPC: partial setup/retry, periodic checks, amounts/curves/full-width layouts, point/zoom/hour/stale replies/reset, languages/existing regressions |
| npm run verify | Exit 0; Rust 793 pass/3 ignored, UI 13/scripts 3; Markdown/assets/Svelte/fmt/Clippy/frontend build pass |
| npm run build:desktop | Exit 0; Windows x64 release/NSIS 3.57 MiB; not installed or started against original DB |
| npm run lint:md | Exit 0; 168 files/0 issues |
| python build/dashboard-polish/check_links.py | Exit 0; seven documents/129 relative links/0 missing |
| git diff --check | Exit 0; existing LF/CRLF notices only, no whitespace errors |

Browser synthetic data does not verify actual Tauri IPC or IDE restarts. Wide tables/amounts/
curves visually checked; narrow layouts/languages/themes covered by existing browser tests.
Zoom/event boundaries checked against [official ECharts events](https://github.com/apache/echarts-doc/blob/master/en/api/events.md),
installed source and actual ECharts instance, without unofficial tutorials. A failure found
byte-limit stops did not report a pending partial line; retained failure log, fixed and reran.

New application desktop/src-tauri/target/release/LLMUsage.exe; installer beneath it at
bundle/nsis/LLMUsage_0.2.1_x64-setup.exe. Build success does not establish installation/native
GUI/IPC acceptance. No new GUI launch or replacement of running old process here. Unconfigured
Codex/awaiting exports/new CLI-JetBrains versions/other unverified outputs are not fully accepted.
No price snapshot edits, online refresh enablement or old-cursor/history cleanup.

<a id="总览暂无数据提示补充2026-10-02"></a>

## Additional overview no-data notice, 2026-10-02

User-requested overview separately counts setup-needed/no-data/verified. Missing automatically
configurable targets remain setup-needed; all others without verified valid data show no-data,
including restricted configuration. No fixed zero “restricted” count. Valid data with real
restrictions retains the restriction notice. Details keep configuration status/reasons/data
results separate; batch setup skips restricted targets. Updated ten languages/interaction rules.
Frontend display only; collection/read-only verification/database/user settings unchanged.

Same Windows/Node/Edge/lockfile environment; logs under build/telemetry-no-data/.

| Repository-root command | Result |
| --- | --- |
| npm run test:ui | Exit 0; 14 tests, ten-language keys/placeholders |
| npm run check | Exit 0; Svelte zero errors/warnings |
| npm run test:browser | Exit 0; Edge synthetic IPC: no-data arrival/disappearance/actual policy restrictions plus existing regressions |
| npm run build:web | Exit 0; production frontend |
| npm run lint:md | Exit 0 |
| git diff --check | Exit 0; no whitespace errors |

Browser assertions cover restricted/no-data overview, retained detail restrictions, valid-data
transition to verified without erasing restrictions, and return to no-data after valid output
disappears. No rebuilt installer/native GUI launch; browser results do not establish actual
IDE settings taking effect or complete historical collection.
