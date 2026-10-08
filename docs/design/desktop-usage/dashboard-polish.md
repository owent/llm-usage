# Dashboard setup status, cost details and trend interaction

<a id="看板配置状态费用明细与趋势交互优化"></a>

2026-10-04. Existing local-source, reference-pricing and history-retention rules apply.

<a id="配置与采集状态"></a>

## Setup and collection status

Overview retains one status line with enable/details actions. Only missing configuration
eligible for automatic setup counts as waiting to enable. Other items without verified
valid data display “No data”, including those currently ineligible for automatic setup;
valid incoming data changes status to “Verified”. Hide zero-count configuration-limit
items. Show remaining configuration conflicts separately even with valid data. Details
always distinguish configuration state, restriction reasons and data checks; “No data”
does not permit otherwise restricted writes. Details inspect actual output targets
automatically and read-only. File presence does not establish valid data: existing
adapter field rules check parsable telemetry with read-size/range limits, without
importing bodies or creating calls. Wait when data is absent; valid samples verify
observed data, without clearing current configuration conflicts. Preserve external
targets and do not claim success for shared output whose client cannot be identified.
Fix duplicated JSONC insertion newlines while retaining comments, BOM, newline style and other keys.

Recheck every 30 seconds and merge concurrent checks. Inspect the last 2 MiB first;
if no matching call appears, also inspect the first 8 MiB. Each sample allows at most
2,000 lines, 256 KiB per line and 200 ms reading time. Identify the actual client;
another Agent's records in shared files do not verify it. If length, mtime, creation
time and first/last fingerprints are unchanged, cache checks for at most five minutes
and 128 entries. New output invalidates cache. Valid samples establish observed export,
without guaranteeing complete historical or current-day coverage.

Poll status every 500 ms during collection and refresh usage afterward. Idle polling
remains ten seconds and checks status without redrawing. After a manual request is
accepted but before running is visible, retain the collection indicator and disable
repeat requests until running or completion time changes. A short task finishing
between polls still refreshes; do not revert prematurely to idle waiting.

<a id="费用和布局"></a>

## Costs and layout

Today's hourly model, Agent and Agent-plus-model groups all plot token curves, including
one-hour ranges with the same axis/curve logic. Do not switch to call counts or bars.
Unknown totals remain gaps. Tooltip rows show only name/value (≥ for lower bounds, —
for unknowns); explain lower bounds once. Full coverage explanations remain available
on demand. Limit tooltip width and wrap long model names. Cost cards show compact
amounts, with coverage counts in tooltips. The cost panel normally shows amounts, gap
counts and curves, expanding explanations on demand so currencies/long text do not
make it much taller than neighboring components.

Costs are off by default. The dashboard shows current-price “API usage-based price
reference”; historical event-time estimates/source amounts remain in the database/API
without repeated dashboard display. Unknown amounts remain gaps; currencies never
merge. Full-width model details add reference amounts and per-currency totals, retaining
unpriced/partially covered usage without filling unknown tokens with zero.

One cost request returns provider/model subtotals and current-price amounts grouped by
local day/provider/model for panel, table and curve reuse. Switch curves by currency
and compare models without mixing currencies. Cost curves are daily even when usage is
hourly; never distribute a daily amount into one hour. Weekly/monthly curves sum relevant
days. Unknown days remain gaps; verified zero amounts may display zero. Usage/call charts,
costs and model table use full width; two share charts sit side by side. Activity heatmap
and weekday distribution, unaffected by chart selection, move last; preserve other
order/sizes/visibility. Heatmap covers the selected whole year and weekday distribution
the full queried date range, with independent scope explanations. Pie charts reserve
separate legend space to prevent clipping/overlap; stack vertically on narrow screens.

The collapsible cost panel lists each used model's actually matched current unit price
per million tokens: provider, currency, input/output/cache-read/write tiers, context
threshold, channel and price snapshot. Reuse each event's matched price_id rather than
averaging or selecting tiers again. Unmatched models show unavailable prices. Original
model attribution and reference provider remain separate; unknown currency/rates stay
unknown. The reference model table uses one amount column and per-currency totals.
Today's/current-range summaries include reference amounts over the same range when
enabled. Selected hours filter retained individual calls by exact local hour, including
both DST occurrences; never place whole-day costs in hourly cards. Seven metrics and
costs use compact equal-height grids; input-coverage explanation remains in tooltips.
Quota uses a separate compact row with expandable scope, snapshot time and explanation
that account quota is independent of local tokens.

Local read-only checks found input/output on VS per-request spans for 2026-10-01/02,
but the old adapter had not derived total. Derive only when both fields in the same
span are known; missing fields, failed calls or overflow stay unknown. Rule correction
replays consumed unchanged cursors; only complete valid replay marks completion.
Keep the database/history. Old VS Code turn-input lower bounds plus whole-turn output
yield only an observed lower bound, without replacing complete total. Keep Agent/model
names in legends; tooltips use “name: ≥ value” or “name: —”. Explain lower bounds once,
with full semantics available from the short note's tooltip below the chart rather than
repeated after every series.

<a id="查询和选择"></a>

## Queries and selection

Measure real-database-copy timing, SQL query plans and call counts before optimization.
Weekday distribution reuses daily summaries restricted by date/user filters, without
reading raw messages. Archives unassignable to days retain unknown coverage. Avoid
repeated per-model cost requests. Narrow price candidates by model while retaining
priority/tier decisions. Add indexes based on query plans and preserve old-database
compatibility; do not clear the database or rewrite history for an index.

“Last two calendar days” means yesterday/today in the selected timezone, using date
boundaries rather than a rolling 24-hour window. This range and today default to hourly
periods; a DST calendar day is not assumed to last 24 hours.

Trend usage, calls/sessions and cost curves, plus overview history/today's hourly charts,
support point/x-axis-label clicks and horizontal mouse-drag selection. On release query
the continuous range by corresponding period boundaries, including reverse drags.
Dragging selects rather than pans; wheel/slider zoom still chooses visible ranges.
Show highlight, start/end and reset; reset clears every related highlight. Trend selection
restricts summaries, model/Agent distribution, model-table usage and current-price
references together. Overview history uses the same summary/distribution rules; today's
hourly selection independently restricts today's summary/distribution/model/Agent table.
History selection does not change today's query. Daily cost points in hourly views select
all hours of that day; hourly usage selections query exact call-hour costs, without
allocating daily amounts. Bound selection to the queried range and keep cross-day timezone/
week-start consistent. Stale responses cannot overwrite new selections. Filter/period/user
changes clear selection. Charts retain the complete query range; selection neither
refetches curves nor creates query loops. During waiting/failure, show loading/gaps
rather than the previous range's distribution/model table.

summary accepts optional first_period/last_period label boundaries to restrict period
rows after date/instance filters. Sessions/duration still use retained details from the
same range, without adding period DISTINCT counts. Select hours by local labels; the
chart's single repeated-DST-hour label includes both actual occurrences. Weekly/monthly
selections show actual ranges clipped by queried date boundaries. Pending summaries
retain card layout and loading values to prevent jumps.

<a id="键盘选区"></a>

### Keyboard selection

Time charts are focusable with localized accessible names. Arrows move, Home/End choose
first/last period, Shift extends contiguous range, Enter applies the query. Mouse and
keyboard use the same range/revision rules.

<a id="验证与回滚"></a>

## Validation and rollback

Test configured/waiting/valid/invalid/shared mixed data, current conflicts despite historical
samples, newline preservation, model subtotals equaling currency totals, curves equaling
current-price summaries, unit prices matching selected tiers, date/user/model filters,
archive coverage, actual forward/reverse mouse drags, no query before release, rapid
clicks/stale responses, consistent selected distribution/model usage/cost, calendar-day
boundaries, full reset and wide/narrow layouts. Original sources/configuration stay
read-only; scripts/copies belong under the task's root build/ directory. Record final
commands, environment, exit codes, performance measurements and gaps. Rollback removes
this batch's code/optional indexes without deleting sources, statistics or price snapshots.
