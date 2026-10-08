# Current API reference prices and compact-summary validation

<a id="当前-api-价格参考与紧凑摘要验证"></a>

2026-10-02, Windows 11 x64. Existing user authorization covers corrections/read-only
local checks and the [interaction specification](../../design/desktop-usage/dashboard-polish.md).
All pre-existing changes were retained.

<a id="实施和根因"></a>

## Implementation and causes

Cost panel, model table/summaries and curves share current-price “API usage-based price
reference”. Event-time estimates/source amounts remain in the database/API. One cost
query returns the actually matched price_id's input/output/cache-read/write prices,
context tiers, currency, channel, provider and snapshot; expansion adds no query.
Unknown prices stay unknown; currencies do not merge. Costs remain off by default,
with existing online-refresh behavior unchanged.

Today's/current-range summaries include the same current-price reference. Point/x-axis/
zoom selections update summary amounts; discard stale responses and reset without
recalculating the complete chart. Hourly selections filter retained details by local
hour, including both DST occurrences under one label. Cost curves remain daily.
Expired archived details explicitly limit coverage.

Seven trend metrics occupy one wide-screen row or four/two narrow-screen columns;
quota does not stretch them. Input decomposition is in tooltips. Copilot quota has its
own compact row with expandable snapshot time, remaining quota, shared-account scope
and history. The full-width model table has one amount column and bottom totals.

Native read-only SQL confirmed that unknown referred to total tokens, not model names:

| Date / local source | Input | Output | Complete total before fix | Cause |
| --- | --- | --- | --- | --- |
| 10-01 VS Copilot, two calls | 17,470 | 219 | Unknown | Per-request spans had both fields; adapter omitted derived total |
| 10-02 VS Copilot, one call | 16,950 | 95 | Unknown | Same |
| 10-01 native VS Code Copilot observations | 3,373,952 | 312,616 | Unknown | Input is last-call lower bound, output whole-turn total; differing coverage prevents addition |

Derive VS total only when both fields in the same request are known; missing/overflow
stay unknown. Replay unchanged consumed EOF cursors. Accept completion only when the
old parser v1/v2 complete field digest exactly matches after removing the new total;
other conflicts retain original handling. Agent/model curves add no unknown Agent
suffix. Missing totals display the complete-total-unavailable explanation while retaining
observable input/output.

<a id="实际核验"></a>

## Actual checks

Actual IDE configuration, original Agent files and user database were unchanged; no
model calls. SQLite mode=ro backed up the original into ignored build/dashboard-reference/;
correction wrote only the copy. Scripts/programs/logs also remain there. Anonymized output
contains only date/Agent/model/count/amount, without identities/bodies.

Copy rescan: three VS calls, total changed from unknown to 34,734, updated=3,
zero additions/conflicts/errors, data revision=131. Second rescan updated=0, revision
still 131; three records/calls retained. Actual Agent and Agent-plus-model curve queries
both returned totals 17,689 on 10-01 and 17,045 on 10-02. Series names contain actual
Agent/model, without unknown suffixes. This relies on surviving TEMP sources; deleted
history is not guaranteed recoverable.

Copy cost query: 09-03 through 10-02, Asia/Shanghai, all retained instances, default
reference channel. Eleven models, ten matched prices, 23 daily-amount rows. Three timings:
221.567 / 210.951 / 180.603 ms. Model subtotals and daily curves each match currency
totals: USD 19,572 cents, 3,758 priced records, 1,111 unpriced. These are reference values
for this copy/filter, without treating priced-record counts as calls. Its data range
exceeds the previous copy's, so timings are not a same-database performance comparison.
Current-price querying remains one request, without per-model IPC additions.

<a id="验证命令"></a>

## Validation commands

Node.js 24.21.0, Rust/Cargo 1.98.1, PowerShell 7.6.6, installed Edge.
All final commands exited 0; complete logs are under ignored build/dashboard-reference/.

| Command | Result |
| --- | --- |
| npm run verify | 169 Markdown files/no issues, 82 assets, three script tests, 14 frontend tests; Svelte zero errors/warnings; fmt/Clippy passed; Rust 796 passed/three ignored; production frontend build passed |
| npm run test:browser | All Edge regressions passed, including model-price expansion, today/selection amounts, compact summaries and narrow windows |
| npm run build:desktop | Windows x64 release succeeded in 3m 59s; LLMUsage.exe 9,419,264 bytes, NSIS 3,746,310 bytes (3.57 MiB) |
| git diff --check | Passed; existing LF/CRLF notices only, no whitespace errors |
| This batch's relative-document-link check | Six files, 112 links, none missing |

Installer output: desktop/src-tauri/target/release/bundle/nsis/. No installation or GUI
startup in this batch. Browser IPC used simulated responses; actual correction/queries
used a real database copy. Build/browser checks do not verify each native desktop
operation. Running the new build and collecting again replays still-existing old VS
sources automatically. The original user database stayed read-only during validation.

Eleven cost tests passed: different current/historical prices, actual multi-context-tier
matches, model/curve summaries, hour selection, invalid labels, multiple currencies and
expired details; added DST selection covers both occurrences. VS old-EOF integration
checks records/revisions for complete/incomplete calls and retains conflicts for actual
field changes. Invalid/incomplete lines cannot mark upgrade complete. Derived totals
over the token limit retain known components/calls and unknown total. Edge regressions
passed today/trend current amounts, seven-column model table, price expansion with zero
extra requests, current-price curves, point/zoom/rapid stale responses, quota default
height<80 px and details, and existing ten-language/theme/filter checks.

Rollback removes only this batch's code; databases/snapshots have no new schema or deletion migration.
