# Native interactions, incremental archives and scale acceptance

<a id="原生交互归档增量与规模验收"></a>

2026-10-04, version 0.2.1; cwd D:/workspace/projs/github/owent/llm-usage.
Windows 11 Pro x64 10.0.26300; Ryzen 9 9950X3D, 16 cores/32 threads, about 125 GiB RAM.
Node 24.21.0, Rust 1.98, Tauri CLI 2.12.0, WebView2 154.0.4258.53; lockfiles identify dependencies.
This preserves the earlier batch that day. Later query acceleration/resource/parallel-collection
results are in [current acceptance](current-acceptance.md) and [remaining execution results](plan-execution.md);
[Plan.md](../../../Plan.md) owns active status. Temporary logs/executables/synthetic DBs stay
under root build/query-next/, build/plan-finalization/ or build/plan-completion/; private source
bodies are not copied.

<a id="当前实现"></a>

## Implemented behavior

- Retention cleanup/clearing share background maintenance with precommit cancellation. Atomic
  state decides whether cancellation or commit wins; SQLite progress callbacks interrupt
  expensive SQL. Full rollback preserves events/revisions/retained positions/summaries without
  rescanning. Cancellation is rejected after commit begins; accepted cancellation disables
  the button until completion. Incomplete backups delete only owned files. Exhausted filename
  attempts fail before modifying existing snapshots.
- System-task failure tests inject only at OS calls. Actual AppState/SQLite checks verify
  intent persisted before calls, queryable errors, no collection from residual triggers after
  disabling, and retry recovery. Manual-only scanning defaults off; enabling restricts both
  GUI/background default-source and quota discovery.
- Charts use arrows/Home/End, Shift continuous selection and Enter queries with existing
  mouse ranges/model/Agent/cost interactions. Ten languages save through actual controls;
  HTML lang follows them, settings controls have accessible names and forms wrap by container
  width. CSS scaling does not verify OS DPI or assistive technology.
- Daily summaries/Agent distributions/day charts merge rows, retaining result groups/active
  days only. Sessions/durations group by exact local-day boundaries in SQLite, then merge
  complete source/session identities across days without adding daily DISTINCT counts.
  Hour/DST/unknown-field/retained-archive coverage rules remain unchanged.
- Schema remains 11. Opening old DBs adds attribution-exclusion/source-coverage/detail-coverage
  indexes without clearing data/rebuilding summaries/changing revisions. Covering indexes
  add disk/write costs. Final query/startup tests use indexed databases; startup results do
  not measure first index installation on old databases.
- Each connection caches at most four summaries with 2 MiB retained string/vector capacity;
  this is not process memory. Keys include complete filters/selection/timezone/dates/grouping/
  today/retention limits. The same read transaction checks revision/data_version/total_changes;
  same-connection writes/other-connection commits invalidate, failed/oversized queries are not
  cached. Two read-only connections are reused under locks, with short-lived fallback when
  busy; completed queries leave no read transactions.
- When only today's changes advance global revision, stable history is no longer materialized
  repeatedly. The transaction stores date/policy/completed-partition revision/row counts;
  completed weekly/monthly/yearly partitions select the latest of their day boundaries.
  Late data/deletion/missing archives/date-policy changes/invalid markers/future revisions
  trigger complete rebuilds. Clearing removes markers; cancellation rolls markers/statistics
  back. Highest source versions/unchanged row counts do not establish compatibility.
- Collection polls state every 500 ms, returning to ten-second checks when idle. Accepted,
  pending tasks retain completion waiting; short collections completed between checks still
  refresh the UI, avoiding delayed notices after returning to idle polling.

Cache references: [SQLite data_version](https://www.sqlite.org/pragma.html#pragma_data_version),
[rusqlite 0.40.2 total_changes](https://docs.rs/rusqlite/0.40.2/rusqlite/struct.Connection.html#method.total_changes).
A small cross-connection WAL check confirmed data_version remains with the held read snapshot;
after that transaction ends, the next read sees external commits. Rust regressions separately
cover same/external-connection writes without revision changes.

<a id="工程与原生检查"></a>

## Engineering and native checks

| Command/method | Exit/results | Scope |
| --- | --- | --- |
| npm run verify | 0; Rust 852/frontend 20/scripts three; five explicit/environment tests ignored by default | 177 Markdown files, assets/types zero errors/warnings, fmt/Clippy -D warnings, requirement/integration tests and frontend build |
| npm run build:desktop | 0; NSIS 3,810,444 bytes, 3.63 MiB | Final release/embedded frontend; not installed |
| npm run test:headless | 0; 11 checks, final three events/75 tokens | Actual release/SQLite/isolated nonempty synthetic sources/persisted rules |
| npm run test:desktop -- --runs 20 | 0; 13 checks, 20 small-data startups, P95 724.3 ms | Actual WebView2/Tauri IPC, mouse/keyboard/offline/restart/exchange/ten languages/system minute task |
| npm run test:browser | 0 | Edge simulated IPC, five pages/selections/interactions/ten languages/themes/filters/pagination/short-collection handoff/idle polling |
| Explicit Windows tests | 0; one COM task/one HKCU test-key round trip | Test-namespace registration/query/drift repair/repeated deletion; actual permission denial unverified |
| ai-maintenance quick_validate.py | 0; Skill is valid | Static format only; description unchanged, model triggering/quality evaluation unverified |

Logs in build/query-next/: verify-final.log, desktop-build-final.log, browser.log,
headless-final.log, native-final.log. Headless/native results respectively:
build/plan-completion/headless/1791056946337/ and native/1791056947059/result.json.
Frontend JS 1,065.74 kB/gzip 357.03 kB; CSS 51.73/9.50 kB. Current-version explicit Windows
native_task_roundtrip/registry_roundtrip used cargo test --offline --ignored, logged as
windows-task-final.log/windows-registry-final.log. Python -X utf8 avoided system-default
misreading of Chinese during Skill validation.

The 13 native regressions save every language and use CSS scales 1/1.25/1.5/2, checking no
horizontal overflow, visible Save, accessible visible settings forms and unchanged statistics
across languages. A current-user minute task ran without a GUI: after closing the UI, a third
synthetic event was added. OS execution time/Ready/exit 0 and read-only SQLite before reopening
confirmed three events/75 tokens. Restart retained results; cleanup deleted only the test task.
Actual ordinary-user execution and injected failures are recorded independently.

<a id="完整查询"></a>

## Complete queries

Existing bench_v20 synthetic DBs were populated through actual EventPipeline/transactions/
daily-hourly aggregation in batches of 50,000. Coverage: 366 days/50 models/20 Agents/20
source instances. Sessions repeat across days; 20% of each source's events reuse two large
session keys, while different sources stay independent. Multi-Agent synthetic instances test
grouping load only. Final executable reads existing DBs through actual query_summary, including
metrics/sessions/durations/coverage/model-Agent distributions, asserting total calls/group
counts rather than timing a standalone SUM/COUNT.

Each group clears connection caches before 20 complete queries, followed by 20 hits. Details:
100 queries of 200 rows. Complete-query P95 is sorted item 19; details P95 item 95. OS file
caches were not cleared; misses refer to application summary caches. Repository-root commands,
all exit 0:

```powershell
cargo build --offline --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core --release --example bench_v20
desktop/src-tauri/target/release/examples/bench_v20.exe build/plan-finalization/million-1 1000000 --query-only
desktop/src-tauri/target/release/examples/bench_v20.exe build/plan-finalization/ten-million 10000000 --query-only
```

| Events | Main DB/WAL MiB | Miss P50/P95 ms | Hit P95 ms | Details P95 ms |
| --- | --- | --- | --- | --- |
| 1,000,000 | 906.3/0.3 | 926.5/964.6 | 0.033 | 1.29 |
| 10,000,000 | 8,034.1/0.0 | 4,093.8/4,177.5 | 0.049 | 1.23 |

Logs: build/query-next/million-final.log/ten-million-final.log. First index installation on
owned synthetic DBs made Storage::open take 904.4/15,245.0 ms, including opening checks; this
is not native startup. Complete empty-DB import throughput/memory was not remeasured with
current indexes, so earlier-index import figures are not reused. Misses exceed the
[200 ms target](../../design/desktop-usage/architecture.md#budgets); cache hits do not satisfy it.

<a id="原生首屏与资源"></a>

## Native startup and resources

`node desktop/tests/native-scale.mjs --runs 20 --idle-seconds 600 --ui-cancel` exited 0 with
the final release. Result: build/plan-finalization/native-scale/1791056317247/result.json;
log: build/query-next/native-scale-final.log. Before startup, read-only checks resolve the
actual path beneath root build/ and require one million events/20 synthetic-bench sources;
maintenance IPC cannot target real Agent DBs. --help launches no GUI. Help/invalid-directory
rejection/one valid native-cancellation run passed in native-scale-guard.log; these do not
replace 20 independent startups/the complete ten-minute resource measurement.

| Item | Measurement |
| --- | --- |
| Million-event startup | 20 runs, P95 1,223.1 ms; process launch until overview values/history chart render, including CDP waits |
| Cancellation | One actual IPC cleanup/clear each plus settings cancellation button; events/revisions unchanged, no rescan |
| Idle | 601.4 continuous seconds/118 samples; root process alive, static page, automatic collection paused |
| Process scope | Root/all descendant WebViews, at most seven processes, about five-second samples; includes last visible cumulative CPU for exited processes |
| Private bytes | Mean 333.6 MiB/peak 483.9 MiB; 180 MiB target unmet |
| Working set | Peak 524.4 MiB, independently reported from private bytes |
| Idle CPU | 0.309% of one logical core, below this machine's 1% target; not divided by 32 threads |

Startup uses the indexed million-event DB with mixed first/later OS caches. Idle follows two
IPC and one UI cancellation on the same overview/CDP-enabled application; it does not represent
fresh startup/tray/headless states. This development machine is not the proposed four-core/
16 GiB reference hardware. Five-second samples do not measure instantaneous allocator peaks.
Observed connected-WebView requests had no external HTTP/script exceptions; this is not a
whole-process network audit.

<a id="百万库增量刷新"></a>

## Incremental refresh on the million-event DB

`node desktop/tests/native-incremental.mjs --runs 20 --enforce-budget` exited 0. Result:
build/query-next/native-incremental/1791056283753/result.json; log incremental-final.log.
[Node SQLite online backup](https://nodejs.org/api/sqlite.html#sqlitebackupsource-db-path-options),
minimum Node 22.16, copies the read-only million-event DB to a fresh isolated directory,
first checking all sources are synthetic/bench. Baseline DB/native Agent files stay unchanged;
actual Node version was 24.21.0.

Discover/consume one synthetic Codex record, then append 1,000 records in each of 20 rounds.
Timing starts at actual collection-button click and ends with today's visible updated card,
including source reads/parsing/commit/daily aggregates/tiered retention/polling/native IPC/UI
updates. Each round also verifies exactly +1,000 calls/+15,000 tokens, advancing revision and
DB event counts; final repeat scans add no usage. Synthetic versions do not verify native
new client versions.

- P95 1,386.7 ms, maximum 1,406.3 ms; passed the two-second limit, final 1,020,001 events.
- Visible cards used actual IPC/query results, without injected replacements. Observed
  requests had no external HTTP/script exceptions.
- Stable-archive skips separately regress late data/deletion/missing archives/date-policy/
  abnormal revisions. An isolated complete materialization took 1,770.7 ms; two checks with
  stable history/advancing global revision took 147.5/147.7 ms and materialized zero rows.
  These do not replace complete incremental-refresh timing.
- ingest_runs start/end use the task's common time; their difference cannot measure separate
  parsing/aggregation durations.

<a id="最终文档与剩余条件"></a>

## Final documentation and remaining conditions

Final checks exited 0: 177 Markdown files/zero issues; 95 local references/anchors in 12
changed documents; Skill static format/git diff --check. Read-only cleanup found zero test
tasks/application processes/test startup entries; temporary files stay outside Git status.
Outputs in build/query-next/: docs-final.log, links-final.json, skill-final.log, diff-final.log,
residue-final.json. Engineering tests do not replace reference checks; simulated IPC does
not verify native behavior.

Miss queries/whole-process memory still miss targets. Proposed hardware, full-import peaks
with current indexes, wake counts, OS DPI/assistive technology, macOS/Linux native GUI and
ongoing CI remained unverified at this stage. Installation was skipped under earlier
instructions; upgrade/uninstall/logout/rollback unverified. Default Gemini/Qwen local paths
were absent; Agents were not launched to manufacture samples. Other source-version/telemetry
coverage, tray/power/file watching/application time limits/retries remain in
[Plan.md](../../../Plan.md). Passing this stage does not complete every planned item.
