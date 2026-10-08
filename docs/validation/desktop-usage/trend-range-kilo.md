# Linked chart selection and Kilo source-health acceptance

<a id="选区联动与-kilo-来源健康验收"></a>

2026-10-03–04. Windows 11 Pro x64, Node 24.21.0, Rust 1.98,
Edge/WebView2 154.0.4258.53, application 0.2.1; locked dependencies. Environment details:
[current acceptance](current-acceptance.md).

<a id="当前行为"></a>

## Current behavior

- “Last two calendar days” queries yesterday/today in the configured timezone, rather
  than a rolling 24-hour window.
- Trend token/call/session/reference-price curves, both overview history charts and
  today's hourly chart support horizontal drag selection. Query on release, support
  reverse selection/reset and synchronize highlights across related time charts.
- Trend summaries, model/Agent distributions, model table and current-price reference
  share the selection; curves retain their original range. Today's/history selections
  are independent. Heatmap/weekday distribution move last with their independent scopes explained.
- Kilo independent-snapshot differences remain comparison results. Row errors persist
  as check-required until mutable rows are corrected/deleted and rechecked. Incomplete
  details cannot enter complete comparison; SQL casts do not fill zeros. Unknown versions
  continue compatibility reading.

Brush behavior was checked against installed BrushView/BrushController/BrushModel source
and [Apache ECharts action documentation](https://apache.googlesource.com/echarts-doc/+/24fe90e684ee48fa8ec5ab7e7a9f9c2fd9397e4e/en/api/action.md).
lineX coordRange holds coordinate boundaries; brushEnd signifies completed selection
and triggers queries. Programmatic brush updates create no duplicate requests. Actual
mouse regressions verify these behaviors independently.

<a id="本机-kilo-根因与恢复"></a>

## Local Kilo cause and recovery

Read-only SQL emitted only roles, versions and numeric aggregates, excluding bodies,
paths and session identities. The sole mismatched session was 7.3.42: 25 assistants'
five exclusive fields summed to 706,116 tokens with valid field types, while the five
session snapshot columns summed to zero. Old logic treated reconcile_mismatch as row
parsing failure and incorrectly degraded the whole file. This difference does not verify 7.3.42.

New kilo-message-tokens-4 follows actual discovery → registration → scan → commit,
invalidating/replaying old processing positions automatically. After verifying a copy,
take Online Backup under the application's same single-writer file lock, then correct
the authorized local statistics database. Never directly clear health state/diagnostics.

| Check | Actual local result |
| --- | --- |
| Original source reads | One database, 13,947 rows, 13,376 parsable calls; existing retention lower bound still excludes expired details |
| Health | degraded→active_compat, check-required files 1→0; unknown historical-version compatibility retained |
| Metadata updates | 34 retained Kilo calls, parser 3→4, no additions or real content conflicts |
| Data retention | All 4,050 retained events and summary metrics unchanged; original Agent database bytes unchanged; diagnostic history retained |
| Repeat scan | Zero additions/updates/conflicts; data_revision remains 401 |

Consistent backup, scripts and anonymized output are only in ignored root
build/trend-range-kilo/. Restoring the backup requires the application writer lock;
original sources need no restoration.

<a id="验证"></a>

## Validation

Actual-mouse regressions with simulated IPC on Edge passed: forward/reverse drags,
no query before release, all three trend curves, both overview history charts and
today's hourly chart, model/Agent shares, model-table costs, full reset, stale responses,
calendar-day boundaries and existing ten-language/theme/wide/narrow checks. Images:
build/browser-smoke/.

Rust cases cover valid details differing from an independent snapshot, automatic replay
of old health/consumed positions, unknown versions remaining unverified, persistent
invalid JSON/role/type/per-record totals across incremental windows, recovery after
correction/deletion, and parser 3→4 without token conflicts. Full checks exited 0:
Rust 838, frontend 20, scripts three; zero type errors/warnings. Windows release/NSIS
exited 0, 3,791,588 bytes; headless 11 checks, native ten checks, and 20-startup
first-screen P95=741.5 ms. Native mouse dragged between two actual synthetic hours,
then clicked down to one call and reset. Range labels matched actual IPC/SQLite values;
Tauri command entry points were unchanged. Commands/limits are in
[current acceptance](current-acceptance.md).

Simulated browser IPC does not replace native desktop results. No installation,
commit, push or remote CI run in this batch.
