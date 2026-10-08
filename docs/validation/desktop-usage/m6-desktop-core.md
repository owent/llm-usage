# M6 desktop implementation: interface, refresh, scheduling, settings and export (partial acceptance)

<a id="m6-核心实现桌面界面刷新调度设置与导出主体功能部分验收"></a>

This record covers M6 implementation/executed checks. Actual Windows desktop operation-by-operation
GUI acceptance for V13–V18/V23–V25 had not run at the original stage; see unfinished items.
The 2026-09-25 snapshot is retained with dated later additions. Heatmaps now use dates and the
language catalog has ten languages; see [2026-09-27 results](review-2026-09-27.md#逐日热力图与多语言扩展).
Historical implementations/unfinished statuses below do not replace current acceptance records.

<a id="元信息"></a>

## Metadata

| Item | Value |
| --- | --- |
| Date | 2026-09-25 |
| Environment | Windows 11 x64, rustc 1.98.0, Node v24.21.0; WSL2 Ubuntu rustc 1.98.1 |
| Code revision | Uncommitted tree, after M1a plus this batch |
| Requirements | execution.md M6; architecture.md refresh/IPC/resources; scheduling.md; F3 i18n requirements |

<a id="命令与结果"></a>

## Commands and results

| # | Command / cwd | Exit | Result |
| --- | --- | --- | --- |
| 1 | npm run verify / root | 0 | lint:md 110 files/zero issues; svelte-check zero errors/warnings; fmt/clippy; cargo test 283+2; vite build 650.86 kB/gzip 220.32 kB, within gzip <=1 MiB |
| 2 | cargo run -p llm-usage-m0 -- --headless / desktop/src-tauri, temporary APPDATA | 0 | All three succeeded: codex 2,597 + pi 37 + omp 8,767 = 11,401 events; input 1,322,967,993 = pi 2,805,788 + omp 1,021,931,700 + codex 298,230,505, matching m2bc/m2d |
| 3 | Repeat preceding command | 0 | added=0, still 11,401 events; repeated incompatible-old-file diagnostics follow existing rules |
| 4 | WSL cargo check -p llm-usage-core / -p llm-usage-m0 | 0 | Core/app compile on Linux, one part of V27 |
| 5 | WSL cargo test -p llm-usage-core | 0 | All 36 test binaries pass on Linux |

<a id="实现清单"></a>

## Implementation inventory

- Backend desktop/src-tauri/src/: app_state single-writer Mutex+Storage/settings/host initialization;
  scanner six-adapter registry/all-source refresh/interval thread/bounded retention; commands
  summary/heatmap/list_sources/set_source_enabled/refresh_*/get/set_settings/app_info/export_data.
  Retire M0 sqlite_probe/read_sample_file. IPC large tokens use decimal strings; errors code+message.
- core/query.rs adds agent_breakdown/hourly_breakdown (today, permitted quality excluding unknown)/
  heatmap_cells (weekday×hour); calendar adds local_hour_of/local_weekday_of.
- Frontend desktop/src/: App overview/trend/sources/settings tabs; SummaryCards, model/Agent
  BreakdownTables, TodayHourly, daily/weekly/monthly TrendChart, UsageHeatmap, SourceList with
  compatible-attempt/incompatible/enabling, SettingsPanel export. On-demand ECharts bar/line/heatmap
  and one Canvas renderer; debounced range/granularity/Agent/model queries; empty/error/in-progress/
  partial-history/unknown states; coalesced refresh button and three-second status polling.
- F3 i18n implementation src/lib/i18n.svelte.ts: small custom catalog, complete zh-CN plus en;
  preference settings=>system=>default, missing-key default fallback/warnings, {name} interpolation,
  Intl numbers/percentages; language changes immediately without changing statistics.
- --headless/--scan-once collects once without WebView and exits, the V24 task collection path.
- Export summary-csv with formula-injection protection and M1a lossless exchange JSON containing
  source identity/revision/parse_basis. Hostnames redacted by default; write app-data exports/ or
  caller directory and return full path.
- Global interval initially 60 seconds, zero pauses; one startup backfill; one coalesced catch-up
  after wake. Settings apply next round; source enable/disable on sources page.

<a id="发现并修复的缺陷"></a>

## Defects found and corrected

| Location | Problem | Fix |
| --- | --- | --- |
| scanner | Six adapters share run_id prefix, conflicting ingest_runs primary key; pi/omp instances fail | Unique adapter-index prefix scan-{ts}-a{n} |
| main.rs db_path | Missing filename points DB to existing APPDATA llm-usage placeholder file and writes data there | Separate llm-usage-desktop/llm-usage.sqlite; incident below |

<a id="事故记录roaming-占位文件"></a>

### Incident: Roaming placeholder file

%APPDATA%\llm-usage was an existing empty SQLite file with no tables, unknown origin/third-party
remnant. The db_path bug wrote the first headless app DB there, 4.1 MB containing app schema
only. No preexisting external contents were damaged because the file had no tables. Restore
the empty SQLite placeholder without deleting the third-party file; use the separate
llm-usage-desktop directory thereafter.

<a id="修订ui-查询为空缺陷2026-09-26-发现并修复"></a>

## Empty UI queries: found and corrected 2026-09-26

User reported failed queries/blank page; query_probe reproduced against the app DB.

| Location | Problem | Fix | Check |
| --- | --- | --- | --- |
| scanner.rs | Hardcoded UTC daily partitions differ from UI Asia/Shanghai tz_version although data is ingested | RunConfig.timezone uses settings per V04/V12 | UTC commit has zero Shanghai-visible calls; recompute gives one on local 2026-09-26 |
| App read/write lock | Long scans, including minutes-long Kilo first scan, block every UI command | Independent Storage::open_readonly WAL readers; writer locks per adapter | Reader works during uncommitted write; uncommitted data invisible |
| Existing DB | UTC partitions unavailable in new timezone | Init/timezone changes recompute_days_in_tz from retained events; skip sealed days; max 750 days | User-DB copy repair: 14 periods, 12,954 calls, 1.55B tokens, 18 models/seven Agents |

New examples/query_probe.rs reproduces app query setup with optional repair mode. Public core
APIs: Storage::open_readonly and ingest::recompute_days_in_tz.

<a id="追加加固2026-09-26-用户复验后"></a>

### Further hardening after user recheck, 2026-09-26

Actual dev mode intermittently reported db_readonly: ... disk I/O error; screenshots show
successful queries/revision chip and error banner in one session. Unit/probe checks, including
stale WAL/SHM copies, did not reproduce it. The historical assessment suspected open-time
environmental conflicts such as antivirus/hot handles; it did not establish a specific cause.

- read_conn retries read-only open three times with 40 ms waits, then falls back under the
  writer lock; queries may queue briefly instead of returning a hard error.
- Remove unnecessary foreign_keys pragma on read-only open to reduce possible failures.
- Found crashed-instance remnants: stuck running run and 18 MB WAL. Cross-process exclusion
  was still unimplemented V24; users then needed to avoid concurrent instances.

User-requested rename: Cargo llm-usage-m0=>llm-usage-desktop, binary/installer LLMUsage
(LLMUsage_0.1.0_x64-setup.exe), npm llm-usage-desktop, window title LLM Usage. Database directory
llm-usage-desktop unchanged; no migration. Real user-DB preflight via headless-equivalent restart:
Asia/Shanghai repairs to 180 rows, stuck runs zero; 14 periods/13,272 calls/1.65B tokens/
18 models/seven Agents.

<a id="分级归档与设置页要求2026-09-26-用户需求实施"></a>

<a id="分级归档与设置页合同2026-09-26-用户需求实施"></a>

## Tiered archives and settings requirements, 2026-09-26

- Schema v5 adds hourly_usage recomputed for affected days inside commits, and period_usage
  week/month/year materializations. enforce_tiered_retention initially uses detail seven/hour
  30/day 365/week 3650/month 10950 days/year lifetime. Protect current periods: daily cutoff
  cannot cross current week/month/year starts, otherwise unfinished periods are permanently
  incomplete. Effective retention may exceed settings by up to one period.
- Weekly/monthly queries use materializations outside daily retention; choose one representation
  per period, preferring fresher daily data. Hour charts use persisted hours after detail cleanup
  for 30 days. Fix old never-assigned hour field that put all events at midnight.
- Settings DTO: optional week_start, None follows language region (zh Monday, en-US/CA Sunday);
  refresh_interval_secs default 3600; add RetentionTiers/hostname_alias.
- Ordinary-user Windows commands: set_auto_start HKCU Run, set_refresh_task hourly schtasks
  headless LLMUsageDataRefresh, system_task_status, pick_save_path rfd native save dialog for
  user-selected exports per architecture requirements.
- Exchange becomes aggregate source registry/all daily/period/hour partitions without session
  details; reimport reconstructs history. Document existing JSONL byte cursors/SQLite positions
  (Kilo/OpenCode part.id update ordering)/file identity-generation for rename/truncation/same-size changes.
- tests/tiered_retention.rs four cases cover cleanup/materialization/query merge/hour retention/
  policy validation; all 57 test binaries pass.

<a id="多用户导入闭环codex-旧版与-ui-补全2026-09-26-第三轮"></a>

## Multiple users, complete import flow, old Codex and UI additions (third round, 2026-09-26)

<a id="v6-多用户"></a>

### v6 multiple users

- users table/source_instances.user_id default default. Multiple users' sources may share an
  origin host through many-to-many origin_hosts/user. Filters.instances receives the app-resolved
  current user's source set; core does not interpret users.
- list/create/set_current/assign_source commands; top-bar user selector.
- Real DB-copy preflight: schema six, seven sources default-owned, correct query filtering.

<a id="导入闭环m1a-要求实施"></a>

<a id="导入闭环m1a-合同实施"></a>

### Complete import flow: M1a implementation

Add ExchangeHourlyPartition. import_aggregate compares same-key revisions: higher replaces,
equal changes nothing, lower conflicts/diagnoses. Source registration preserves ownership;
imported hosts register external. Settings export/import buttons use pick_open_path=>
import_exchange=>displayed counts.

<a id="分级归档新默认与手动清理"></a>

### Revised archive defaults and manual cleanup

Second adjustment reduces aggregation work: hours three/days 90/weeks three years/months ten
years/year lifetime; details stay seven days. Details may outlive hours without data loss.
storage_stats returns layer counts/DB-WAL bytes; manual_cleanup(days_before) cuts every layer
by age while protecting current periods. Archives page displays statistics/cleanup.

<a id="codex-01390151-旧版支持m2-d-遗留清零"></a>

### Codex 0.139–0.151: completing old-version support left from M2-D

- Read all 238 files/13,481 token_count records into field groups under build/codex-legacy-forensics/.
  Cumulative-total increments define semantics: delta>0 new call, last is latest; delta==0 with
  unchanged last is duplicate; delta==0 with changed last follows compacted in 85/85 cases,
  a carried compaction echo; negative delta diagnoses source rollback/resets baseline.
- rollout_legacy.rs registers 21 versions; incompatible files 238=>0. Real Codex events
  2,578=>15,955, with 13,358 legacy events matching Python. Reconciliation 262 matched/37
  mismatched, all classified/explained.
- Fresh empty-DB headless imports 40,472 events then immediately applies seven-day detail
  retention. Aggregates retain complete history: Codex daily total 1,835,125,364 exactly matches
  real_verify, 56 days, visible query 14 periods/13,194 calls.

<a id="前端补全"></a>

### Frontend additions

- #app height:100vh/overflow:hidden prevented scrolling; min-height:100vh restores native scrolling.
- Overview today cards/cache stack/Agent-model pies in two columns; trend four metrics/weekday bars.
- Typora-style vertical settings menu/archive statistics/manual cleanup/import button.
- Add 43×2 i18n keys, total 161×2.

<a id="第四轮2026-09-26-用户需求"></a>

## Fourth round: user requirements, 2026-09-26

- clear_all_data removes usage_events/hourly/daily/period/diagnostics/checkpoints/aliases/
  aggregates/runs/quotas, resets source_files to new for a full next scan, retains hosts/users/
  settings. Red button/confirmation UI.
- RefreshState adds progress_percent by adapter index/count and eta_seconds from completed-
  adapter mean duration; top progress bar.
- Equal-revision imports now replace different content instead of skipping missing updates.
  Regression same key/revision values 100=>200=>200 retains one 200 row without loss/double-counting.
- First AppState::init renames v6 default user to OS name from USERPROFILE/USER; user_id/source
  ownership unchanged.

<a id="第五轮修复2026-09-26-用户反馈"></a>

## Fifth round: user feedback, 2026-09-26

- Hour duration AVG returns REAL but Rust expected `Option<i64>`; CAST(AVG(...) AS INTEGER).
- query_summary hour branch reads hourly_usage; DailyRow hour labels YYYY-MM-DD HH:00.
- chart_series groups total/model/Agent/Agent+model from daily_usage with frontend selector.
- v7 idx_usage_events_instance_time (source_instance_id, occurred_at_ms DESC); event_details
  uses requested date range instead of 0..now+1d, max page_size 500.
- diagnostic_logs settings tab; v7 idx_diagnostics_created_code index.

<a id="数据库简化2026-09-26-用户决策预发布阶段"></a>

## Database simplification: prerelease user decision, 2026-09-26

- Remove seven v1–v7 incremental migrations; use one FULL_SCHEMA SQL definition.
- Matching user_version opens normally; mismatch returns SchemaMismatch and GUI rfd asks
  permission to delete/rebuild. Approved removal covers .sqlite/.sqlite-wal/.sqlite-shm; decline exits.
- Move tauri.conf.json dragDropEnabled from app to window to prevent WebView2 intercepting HTML5 drag.
- Rewrite migration_v16/review_regressions/multi_user_import/v28 tests for creation/repeat opens/
  PRAGMA/version-mismatch rejection.

<a id="第六轮修复2026-09-26-用户反馈"></a>

## Sixth round: user feedback, 2026-09-26

- kimi-code registry updates left consumed active_compat cursors skipped as unchanged.
  Historical run_refresh fix cleared active_compat cursors/status before scan; event deduplication
  prevented repeat usage. Measured two=>zero. This historical procedure is not a general current
  rule for discarding processing history.
- setup uses primary monitor resolution: 72%×78%, bounded 900–1600 by 600–1000.
- Historical frontend sub-agent work: filters in history panel, timezone dropdown, export
  checkboxes, chart legend/axis spacing, segmented dimensions, token lines, resize handles,
  detail loading skeletons.

<a id="第七轮修复2026-09-26-用户反馈"></a>

## Seventh round: user feedback, 2026-09-26

- Persist permitted operation messages settings_changed/import_completed/export_completed/
  manual_cleanup/clear_all_data/scan_completed in diagnostics; diagnostic_logs code_filter,
  frontend filter dropdown.
- Also retain preceding historical kimi-code active_compat cursor/status replay fix.

<a id="第八轮2026-09-26-用户需求"></a>

## Eighth round: user requirements, 2026-09-26

- hideDelay:3000 wrongly kept tooltips visible three seconds after leave. Use hideDelay:0,
  showDelay:0: instant hover/leave, axis trigger remains visible while inside chart.
- AppSettings theme system/light/dark; themes.css variables, data-theme/prefers-color-scheme,
  settings selector/global styles/cards/ECharts themes.

<a id="图表粒度修复与时间点汇总2026-09-26-用户反馈"></a>

## Chart granularity and selected-period summaries: user feedback, 2026-09-26

- chart_series formerly grouped only daily_usage.local_day, ignoring granularity/filters.
  Only total usage through query_summary changed on hour/week/month; model/Agent/combined stayed
  daily and ignored filters. Shared period_key_of aligns labels. Hour reads hourly_usage with
  YYYY-MM-DD HH:00; day/week/month calendar-aggregate daily rows; week/month include period_usage
  only for labels not already covered by daily rows. Agent/provider/model/allowed-instance filters
  use Filters::matches. Group token merging follows SQL SUM: wholly unknown remains null.
- Clicking overview historical call/token points displays period-summary cards: calls/input
  total-cache/uncached/output/total/cache ratio/sessions; weekly/monthly also active days. Model/
  Agent pies load chart_series dimensions filtered by label using total_tokens proportions;
  SharePie optional height is 190px in selection. Same-point click/clear cancels; changed range/
  granularity/filters clear selection. Cards use summary.periods label lookup without extra
  queries, grouped labels align with periods. Pies lazily make two aggregate-table chart_series
  requests. ECharts click returns raw labels for total/grouped modes.
- tests/chart_series_grouping.rs five cases: hourly rows/summary labels, daily-to-week merge,
  monthly labels, model/instance filters, materialized periods/covered-label dedup/allowed instances.
- Root npm run verify exited 0: Svelte zero errors/warnings, fmt/clippy/all Rust including five
  new cases, vite 798.75 kB. Add four×two keys cards.activeDays/overview.periodSummary[.clear/.hint].

<a id="闪烁修复x-轴点击与设置默认项2026-09-26-用户反馈"></a>

## Flicker, x-axis selection and restored defaults: user feedback, 2026-09-26

- pollRefresh checked !running && last_finished_ms>0 every three seconds, permanently true after
  first refresh: repeated summary/source loads/notMerge whole-chart rebuilds. Unconditional
  summary assignment also recomputed unchanged data. Fixes: requery only running=>finished or
  changed completion times, including tasks/headless; idle polls update status only. Query key/
  revision guards retain unchanged summary references; user switch/import/settings force loads.
  Poll three seconds while collecting, ten while idle.
- Top automatic UI-refresh selector off/30 seconds/one/two/five/ten minutes, localStorage.
  Initially 60 seconds, user changed default to 300 the same day. llm-usage-auto-refresh-v2
  resets old stored defaults; independent from backend 3600-second collection.
- CallsChart/TokenChart/TodayHourly/SharePie render by structural signature: subplot/dimensions/
  labels/series/theme/language unchanged merges setOption; structural changes rebuild notMerge.
- zr canvas clicks use containPixel('grid')/convertFromPixel nearest category; no need to hit
  points; legends/outside-axis regions excluded.
- General/retention restore-defaults buttons modify drafts until save; general preserves manual
  roots. Timezone defaults via system Intl, collection 3600, retention 7/3/90/1095/3650/lifetime.
  Interval hint says 3600 hourly; fix incorrect monthly hint 10950=>3650 to match backend defaults.
- Period selection flicker: nulling pie data collapsed rows, text-loading height differed from
  pies, and empty periods collapsed cards. Keep old pies until new results, same-size dual skeleton
  cards, fixed min-height for empty states; SharePie signatures use name sets so value changes
  merge/animate. Selection updates cards/pies in place without layout jumping.

<a id="清空重采数据丢失分析与-zcode-db-回填2026-09-27-用户反馈"></a>

## Clearing/recollection data loss and historical ZCode DB backfill: user feedback, 2026-09-27

Retain the initial diagnosis/implementation below. Claims of permanent complete DB history,
time/token deduplication and an independent backfill button were corrected by
[same-day review](review-2026-09-27.md): DB cleanup also has a 30-day limit. Unified collection
now chooses the DB and atomically replaces source/day contributions without approximate matches.
Passing earlier commands did not establish correct deduplication requirements.

- Clearing/recollecting greatly reduced September 25–26 details/charts, mainly zcode/GLM-5.3.
- Read-only real DB found rolling model-io JSONL: sub-agent model-io-sess_subagent_agent_*.jsonl
  deleted after sessions; ten of 13 registered files absent. Main model-io-sess_6842*.jsonl
  repeatedly compacted, generation 139; September 25 records gone. Cursor at full length 19,853,928
  yielded events from September 26. Clearing can reread existing disk contents only; cleaned/
  compacted source history is lost. App had 409 ZCode events for September 26–27. cli/db/db.sqlite
  model_usage then retained 8,031 rows back to 2026-08-29, including deleted sub-agent portions:
  September 25 2,633 calls/592.8M tokens; September 26 2,734/1091.2M; all GLM-5.3. This observed
  range did not establish permanent retention.
- Initial comparison: DB input_tokens matches cache-inclusive input_total, output_tokens matches
  output_total, input+output matches computed_total; DB completion 3–32ms later than JSONL.
  407/409 events approximately matched by session/absolute time difference <=2s/equal input-output.
  Same-day review later rejected that approximation as a call-identity rule.
- Historical zcode_db_backfill core adapters/zcode/db_backfill.rs/command/settings button read
  DB and matched existing events by that rule. ID zcodedb:{logical_request_id}:{attempt+1};
  canceled/error rows retained error_status; unverified compact querySource unknown/one diagnostic;
  AI SDK mapping derived uncached. Repeats included backfill rows in matching; two integration
  cases tested dedup/repeats/tolerance/mapping/diagnostics. This old path was superseded.
- Prevention: clear_all_data first VACUUM INTO `backups/llm-usage-backup-<ts>.sqlite`, retain three,
  report path. clear_all_preview warns about N source files cleaned by their Agents and unavailable
  for recollection, explains backups. Historical capabilities distinguished DB validation/backfill
  and added rolling-window limitations.
- Same-day read-only cross-source missing/rewrite inventory: ZCode alone ten/13 missing,
  generations 128/139; Codex 299 files all present/no rewrites, no config cleanup, logs_2 logs only,
  state_5 metadata/cumulative values, thread_history UI representation without per-call usage;
  Kilo no compaction/messages complete since June 2026; Kimi Code/Kimi Work/pi/omp all files present/
  no rewrites; omp agent.db usage_history quota windows/client_usage empty, neither recovery data.
  adapters.md records historical recollection/rolling risks; uninstalled products remain unchecked.
  Generic SourceList disk-existence checks now show cleaned-file counts and tooltip explaining
  history exists only in app and cannot be recollected after clearing.
- npm run verify exited 0. Add seven×two cleanup.clearAllMissing/Backup/BackupAt and
  settings.zcodeBackfill[.hint/.done], plus two×two sources.missingFiles[.hint].

<a id="tooltip-离开隐藏加固2026-09-27-用户反馈三轮浏览器复现定论"></a>

## Tooltip leave handling: three rounds, browser reproduction, 2026-09-27

- User screenshots twice showed tooltips persisting after mouse leave.
- Third-round browser reproduction/ECharts 6.1.0 source found hideDelay:999999 with manual
  globalout hideTip ineffective: TooltipView.manuallyHideTip calls tooltipContent.hideLater(
  tooltipModel.get('hideDelay')), so manual hiding waits about 16.7 minutes too. Prior two rounds'
  globalout/document mouseout/blur/mousemove handlers dispatched correctly but actions were delayed.
  A same-version independent build/tooltip-repro page with real CDP mouse events showed hover/
  leave/direct dispatchAction({hideTip}) and even setOption({tooltip:{show:false}} failed to hide;
  hideDelay:0 hides immediately. Its server was cleaned up.
- Six components change 999999=>0, retain transitionDuration 0. Tooltip now hides in canvas legend/
  margins outside grid, stays during grid movement. Keep setupTooltipAutoHide globalout/document
  movement outside container/document window leave/blur handlers for layout shifts (selection
  expands cards/moves canvas without mouseout) and WebView2 missed events. Zero delay makes these
  hideTip actions immediate; pies/heatmaps keep hideTooltipOnBlank; trends hideTip after selection.
- Reproduction checks hover/legend-hide/grid-return/leave-hide/select-hide and redisplay when
  pointer remains in chart after movement; npm run verify exited 0. ECharts 6 must avoid large
  hideDelay; manual hiding requires zero delay.
- npm run verify exited 0; eight×two keys: five header.autoRefresh.* plus settings.interval.hint/
  restoreDefaults/defaultsPending. Backend collection already defaults 3600; unchanged.

<a id="未完成项显式遗留"></a>

## Explicit unfinished items at the original stage

| Item | Status | Follow-up |
| --- | --- | --- |
| Actual Windows GUI operations, V13–V18/V23–V25 | Not run | Web DOM alone cannot prove IPC; native operations/GUI automation records required |
| Export location dialog via tauri-plugin-dialog | Unimplemented | Then writes app-data/caller paths; dialog integration required later |
| Windows tasks/cross-process exclusion/uninstall cleanup, V24 | Unimplemented | Headless ready; task registration/read-back separate |
| Per-source interval/fixed schedules | Unimplemented | Global interval/source enabling; extraction_schedules integration pending |
| File notifications | Unimplemented | Polling permitted initially; watching an optimization |
| Cleanup preview/default restoration/session import/rescan preview | Unimplemented | Later M6 work |
| V20 initial million-event query percentiles | Not run | Before M7 |
| kilo/zcode/kimi/kimi-work scanner registration | In progress | Interruption below |

<a id="kilozcode-适配器中断记录2026-09-25"></a>

## Kilo/ZCode interrupted adapter work, 2026-09-25

Two historical parallel implementation sub-agents hit API limits at 17:12 without leaving source,
but finished redacted samples: tests/fixtures/kilo/ two real sessions/expectations and zcode/
two real/nine synthetic/expectations. UUID/path checks passed, Markdown fixed; implementation
awaited restored limits under M3/M4. M3/M4/M5 local inventory, 2026-09-25, ignored
build/desktop-usage-validation/m345-inventory-2026-09-25.md: Kilo 495 MB active/ZCode 140 MB active/
Kimi Code 18.2 MB/Kimi Work 31.5 MB, first verified real data after path migration. Copilot CLI usage
storage disappeared within a day, suspected upgrade migration requiring relocation; this was
not confirmed. cline/opencode/mimo/zoo/dsh/openclaw/hermes/codebuddy uninstalled.

<a id="证据文件"></a>

<a id="验证产物"></a>

## Validation artifacts

- Backend desktop/src-tauri/src/{app_state,scanner,commands,main}.rs;
  frontend desktop/src/{App.svelte,lib/api.ts,lib/i18n.svelte.ts,components/*}.
- Core query.rs agent/hourly/heatmap and calendar.rs.
- Historical temporary validation DB: C:/Users/owt50/AppData/Local/Temp/llm-appdata-test/,
  in the system temporary directory, disposable.

<a id="2026-09-30-清理全部数据并重新采集改后台执行不冻结-ui-阶段进度"></a>

## Background clearing/recollection and stage progress, 2026-09-30

<a id="问题与根因"></a>

### Problem and cause

- Clicking clear-all/recollect froze the UI.
- clear_all_data full VACUUM INTO backup/ten-table DELETE and refresh_sources full rescan,
  potentially minutes after clearing, were synchronous Tauri commands on the main thread.
  They blocked the Windows event loop/window/status polling.

<a id="实施"></a>

### Implementation

- clear_all_data becomes a background task. Atomic AppState.clear_job_running prevents reentry;
  catch_unwind resets the flag/emits failed after panic. Wait for running collection to prevent
  concurrent writes restoring cleared data, then backup/clear through extracted clear_all_tables/
  trigger complete rescan. clear-all-progress emits waiting/backup/clearing/cleared/rescan/done/failed.
- refresh_sources also starts background work; manual refresh no longer blocks UI. Top-bar
  refresh_status continues three-second progress/ETA polling during rescan.
- api.ts onClearAllProgress uses @tauri-apps/api/event listen. SettingsPanel confirmation displays
  ten-language cleanup.clearAllPhase.* and indeterminate role=status/aria-live progress; cleared
  refreshes statistics, done closes confirmation/displays cleared-table totals.

<a id="验证"></a>

### Validation

- cargo test --manifest-path desktop/src-tauri/Cargo.toml --locked -p llm-usage-desktop clear_all:
  new clear_all_tables_clears_resets_status_and_bumps_revision passes clearing/source_files new/
  revision advance/operation-log write.
- npm run test:browser adds synthetic transformCallback/plugin:event|listen/
  __TAURI_EVENT_PLUGIN_INTERNALS__; warning/stage waiting=>clearing updates/done auto-close/
  cleared summary regressions all pass.
- npm run verify exited 0: Markdown/assets/scripts/UI/Svelte/fmt/clippy/Rust/web build. Actual
  native long-database clearing responsiveness was not automated in this batch and remains
  part of M6 desktop acceptance.
