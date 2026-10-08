# Desktop architecture and technical decisions

<a id="桌面架构与技术决策"></a>

Status: design specification. M1–M4/M6 implemented the module boundaries, database and
scheduling core. Resource numbers remain acceptance targets; validation records report
actual measurements. See the [design entry](README.md) for scope.

<a id="桌面方案"></a>

## Desktop stack

Use Tauri 2/Rust with Svelte/TypeScript/Vite. Selectively import ECharts charts/components;
six chart types render as SVG. Node is a build tool, absent from distribution. The Python
prototype is not a sidecar.

Tauri uses WebView2, WKWebView and WebKitGTK on Windows, macOS and Linux respectively.
System WebViews reduce the runtime shipped with the app; this does not establish lower
total memory than alternatives. Windows size measurements distinguish the app, WebView2
download/installation, shared installed runtime and user database. Sources:
[Tauri WebViews](https://v2.tauri.app/reference/webview-versions/) and
[Windows installation](https://v2.tauri.app/distribute/windows-installer/).

| Option | Relevant capabilities | Decision and tradeoffs |
| --- | --- | --- |
| Tauri 2/Rust | System WebView, files, SQLite and local IPC; prototype chart ideas reusable | Adopted; verify WebView differences and Rust maintenance requirements. |
| Electron | Chromium/Node desktop workflow and easy JS collector migration | No existing Node collector; prefer shipping less runtime. No invented package-size comparison. |
| Native UI/egui/Flutter/Qt | Can implement desktop statistics | No existing UI assets/team requirements justify a second chart implementation; compare within a defined scope if Tauri fails acceptance. |
| Python prototype with desktop shell | Collector logic reusable | Adds Python/cross-process distribution; retained for reference/migration, absent from runtime dependencies. |
| Local HTTP server/browser | Extends static dashboard | Not the main deliverable; another persistent service/port conflicts with the current desktop scope. |

See [Electron's process model](https://www.electronjs.org/docs/latest/tutorial/process-model)
and [ECharts selective imports](https://echarts.apache.org/handbook/en/basics/import/).
Measure whether the stack meets project resource limits. M0 fixes dependency versions,
licenses and compatibility requirements; do not use floating latest. Windows 11 x64 is
first; macOS/Linux CI jobs remain. WSL 2 can provide local Linux build checks.
See [platform/CI](platform-ci.md) for artifacts and validation scope.

<a id="模块边界"></a>

## Module boundaries

```text
Local JSONL / JSON / SQLite / exports with verifiable local origin
                          ↓
Discovery → version/format detection → bounded reads → adapters → normalization/attribution/deduplication
                          ↓                                      ↓
                 Source health                    SQLite single-writer transactions/statistics cache
                                                                 ↕
Optional local OTLP receiver → allowlisted fields           Typed query/refresh IPC
                                                                 ↕
                                                    Local static frontend/charts/settings
```

Backend responsibilities are domain (statistics), ingest (reads/scheduling), adapters (formats),
storage (migrations/queries), aggregates/query, and app (IPC/desktop lifecycle). M2/M6 placed
the shared core at `desktop/src-tauri/crates/core` and app code at `desktop/src-tauri/src`.
The shared Rust core also provides scheduled headless collection without a window/WebView.
GUI/tray/system tasks share configuration, cross-process ownership lock, collection queue
and a single database writer; see [scheduling](scheduling.md). UI code cannot read arbitrary
files, run SQL, access Agent credentials or start a shell. Only the backend writes exports
to a location selected through the system dialog.

IPC returns typed DTOs: values, known-field counts, ranges, source states, generation time
and data revision. Large token integers travel as decimal strings to prevent JavaScript
safe-integer rounding. Charts may use scaled floats; tooltips/exports retain exact integers.
The backend limits detail pagination, chart points and allowlisted filters.

<a id="adapter-layout"></a>

<a id="agent-适配器目录与多版本组织"></a>

## Agent directories and historical versions

Each Agent uses `desktop/src-tauri/crates/core/src/adapters/<agent_id>/`, even for one
version. Compatible historical versions remain inside that directory. M2 migrated the former
codex.rs/claude.rs/pi.rs/omp.rs/gemini.rs/qwen.rs single files; V30 structure and before/after
regressions are in m2d. M2–M5 and later F1 follow this layout. Derivative products still own
their Agent directories and support scope.

Illustrative structure; agent_id/format_id are placeholders, not supported-product claims:

```text
adapters/
  mod.rs                     # Public adapters and shared module exports
  framework.rs               # Cross-Agent interface and collection flow
  jsonl.rs                   # Shared bounded reader
  usage_map.rs               # Shared mapping types/helpers; product mappings remain local
  <agent_id>/
    mod.rs                   # Stable Agent entry and shared interface implementation
    detect.rs                # Product/format detection and version dispatch
    common.rs                # Verified reusable Agent-specific logic, when needed
    versions/
      mod.rs                 # Verified format registration and mappings
      <format_id>.rs         # Format implementation; split complex formats into a matching directory
```

Put version-specific parsing, field mapping and lifecycle rules in corresponding versions/
modules. Do not add sibling `<agent>_v1.rs` files or put all historical differences in the entry.
detect.rs selects by file/database version, record type or schema features. Installed Agent
version cannot explain every historical file; one root may contain several formats. Known
versions select their mapped parser. Unknown versions follow compatibility rules below.

Record product release, source format/schema and application parser_version separately.
One parser may cover several releases; each verified release requires its own source/sample
checks. Successful compatibility reading does not register a release as verified. Keep
supported historical implementations/regressions while normalized output and collection
interfaces stay consistent.

Share frameworks/readers/helpers only where tests establish equal behavior. Product mappings
remain local. Directory migrations preserve public entries, source/record identities and
existing cursors/parsing state. Version changed state formats separately and check recovery;
moving Rust files must not create sources or double count usage. Organize test data by Agent
and version/format with exact provenance. Unit tests belong near versions; Cargo-discoverable
integration entries must exercise detection/dispatch/history through the shared adapter entry.
See [M2 migration](execution.md#m2-layout) and [V30](validation.md#adapter-versions).

<a id="unknown-version"></a>

<a id="未知版本的兼容尝试"></a>

### Unknown-version compatibility reading

Application releases can lag Agents. An absent/unregistered version automatically tries that
Agent's latest built-in parser, without an app update or manual compatibility toggle. Confirm
Agent identity, input type and local attribution before selecting the latest parser for that
Agent/input. Do not guess across products/arbitrary files. Each Agent's version registry
explicitly chooses latest, meaning a shipped implementation, never a downloaded executable parser.

Allow additional nonessential fields while validating required structure, types/units, token
relationships, identities and cumulative/per-call meanings. Missing optional values stay
unknown. Never fill zeros or guess units to pass validation. Import valid records into normal
statistics with “latest parser; version compatibility unverified” markers. Retain independently
valid partial records and return coverage limits instead of excluding an entire unknown release.
Skipping unexplained records cannot establish complete success; stop calculations depending
on affected baselines/call associations. Source health follows actual parsing: normal when
key records validate, while malformed lines, illegal tokens, missing keys/attribution retain
confirmed calls and prompt review.

Codex reconcile_mismatch/snapshot_regression retain diagnostics/reconciliation without alone
degrading read health. Show mismatch when comparison differs, without claiming completeness.
Modern per-call records do not depend on cumulative snapshots. Legacy total/last errors needed
for call identification still degrade health. Missing/null TokenCountEvent.info is a legal
notification without usage; count no call and fill no zeros. Do not generalize this rule to
other Agents. Incremental reads of consumed files retain the earlier parser-selection basis;
compatibility must not hide newly encountered errors.

Persist original source version (possibly absent), selected format/parser_version, basis
known_version/latest_fallback, compatibility status and bounded failure reasons for diagnostics,
queries, summaries and exports. Compatibility and field quality are separate. Validating the
current shape verifies this input, not every field/scenario of its release.

On structure/semantic incompatibility or product/format conflict, report failure or partial
availability and preserve prior results. Failed batches commit no untrusted events, cursors
or summaries; never return successful empty output to hide errors. Parser updates, changed
sources or explicit rescans permit retries; do not permanently ban a release. Later native
samples can justify dedicated implementations, correcting prior contributions by stable
identity with regressions instead of adding duplicate usage. M2 implemented shared registry
dispatch for detection/scanning and persisted usage_events.parse_basis/source_files.format_status.
Per-Agent fallback results are in m2d.

<a id="database"></a>

<a id="数据库选择"></a>

## Database choice

Use SQLite bundled through Rust rusqlite, with explicit SQL migrations. M0 verifies actual
crate/SQLite versions. Require the official WAL-reset fix (3.51.3+ or an explicitly patched
branch); a system SQLite name alone does not establish the fixed version.
See [SQLite WAL](https://sqlite.org/wal.html).

| Option | Relevant behavior | Decision |
| --- | --- | --- |
| SQLite | Transactions, unique keys, partial updates, indexes and local files suit incremental writes/interactive queries | Adopted |
| DuckDB | Bulk analysis; many small transactions are outside its main design goal | No second engine now; revisit large export analysis later |
| Key-value database | Stores events but needs application-managed indexes/migrations/aggregation | No sufficient reason for the extra implementation |
| JSON/CSV files | Data exchange | Exports only; do not replace concurrent updates/indexes/transactions |
| PostgreSQL/service databases | Multi-user concurrency | A local desktop does not require a database service |

The DuckDB tradeoff follows its [concurrency documentation](https://duckdb.org/docs/lts/connect/concurrency),
without claiming SQLite is always faster or smaller. Keep app data in the system application
data directory, separate from source/repository directories, network shares and synced disks.
Use one writer, a few readers, foreign_keys=ON, WAL and default synchronous=FULL. Batch
transactions reduce sync overhead without weaker default durability. Bound busy_timeout and
total retries; the writer schedules checkpoints. Limit long reads/WAL growth. Capacity includes
main DB, WAL, backups and staging; successful cleanup must not be required for safe exit.

Source databases have different access rules:

1. Prefer read-only connections and short transactions for active databases; change no journal/schema
   and perform no source checkpoint.
2. Meet read-only WAL file/permission requirements. If opening might create source sidecars, refuse it.
3. When needed, use Online Backup from the read-only connection to make a consistent staged copy,
   with page/time/space limits and cleanup.
4. If consistent reads are unavailable, report busy/unsupported and preserve prior results;
   suggest official exports or a copy taken after shutdown.
5. Copying active .db/-wal/-shm independently does not establish consistency; never use immutable=1
   for an active source.

References: [Online Backup](https://sqlite.org/backup.html), [read-only WAL](https://sqlite.org/wal.html).

<a id="refresh"></a>

<a id="发现与今日刷新"></a>

## Discovery and refreshing today

Initial discovery checks known product roots within limits and shows sources/capabilities;
only enabled sources have usage read. Allow multiple manual roots, explicit environment
overrides and IDE profiles, without recursive whole-disk scans. Normalize symlinks, Windows
case aliases and duplicate configuration before identifying physical files/databases. Local
WSL/containers require explicit roots/identity and are never automatically started. Remote
directories, account reports and synced remote sessions are outside local scope; check origin
before import, under [source requirements](README.md).

Refresh today performs:

1. Return job ID and merge requests for already-running sources; show per-source UI state.
2. Find new/changed files/session directories; read increments and unfinished records.
3. Normalize/update events; atomically commit events, cursors, parsing state and affected caches.
4. Publish data revision; query totals/charts/tables at that same revision.
5. Return added/updated/unchanged/skipped/error and last success time; failure is neither zero
   usage nor a switch to demo data.

Today's refresh may update history, such as a final usage record written today for last night's
request. Read changed sources, not only date-named files or all history on every run. Calls use
source completion/usage timestamps by default; exact [time rules](data-contract.md) apply.

Automatic collection defaults to one hour; 0 disables all automatic triggers. On enabled
startup, reconcile enabled sources. Per-source interval/daily/weekly rules override the global
frequency; manual refresh can still read every enabled source. Windows collection after exit
defaults off. When enabled, minute system tasks run headless against saved intent/due rules;
task existence does not establish collection success. GUI/headless share two source-instance
workers and one database writer. Reads/parsing release the app database lock; ownership,
results and transactions use the same Storage lock. Windows optional file notifications,
power-saving pause, monotonic intervals and cooperative time/retry limits are implemented.
JSON/JSONL and SQLite query/backup paths check scoped cancellation; blocking OS calls can
only check after returning and have no guaranteed immediate interruption. Exact behavior and
acceptance requirements are in [scheduling](scheduling.md). Initial backfill commits chunks
with coverage progress, making today visible before all history finishes.

<a id="各输入的增量策略"></a>

## Incremental input strategies

| Input | Cursor and read strategy | Failure handling |
| --- | --- | --- |
| JSONL | Physical identity, generation, complete-line byte offset and parsing state | Defer partial lines; redetect truncation/same-size replacement/rename, beyond length checks |
| Rewritten JSON | Stable snapshot, content fingerprint and internal record IDs | Retry concurrent change; update by ID rather than file offset |
| SQLite | Schema features, stable keys/update sequence; recheck unfinished rows | Without updated_at, use bounded rereads/periodic reconciliation; an hour window cannot guarantee completeness |
| OTLP | Event/trace-span identity or metric series/start/end/temporality | Handle retries, resets, sampling and cross-day intervals independently |
| CSV/JSON exports | File digest, semantic row identity and report range/revision | Replace a matching report partition; overlapping exports do not add repeatedly |

Initial limits are 8 MiB per line, 4 MiB per chunk (lines can span chunks), and 30 seconds per
source/run, subject to calibration with test data. Report location/reason on excess, permitting
controlled retries without silent drops. JSONL's default per-file window is 32 MiB; larger explicit
line limits enlarge it. Complete-line continuation and two-worker interruption/transaction
rules are in [scheduling](scheduling.md#合并事务与恢复). See [query acceleration](query-acceleration.md)
for optional daily derived tables and invalidation. Diagnostics save field/code/position,
never raw lines.

<a id="仅本机的遥测与导入"></a>

## Local telemetry and imports

The optional M5 OTLP/HTTP receiver supports protobuf; enable JSON only for verified client
requirements. Implement needed logs/traces/metrics paths, without shipping Collector,
Prometheus or Grafana. Default off, loopback only, with per-source random tokens, size/rate
limits and content-free logs. Exporters without authentication use files rather than an
unprotected port. Current receiver caps compressed/decompressed bodies at 64 MiB, uses exact
field allowlists, and admits 120 requests/minute with up to four simultaneous connections,
returning HTTP 429 on excess. These limits do not verify sender versions, origin or completeness.
Windows implements isolated Claude/Codex logs and CodeBuddy CLI 2.98.0 traces authentication
with current-user system credentials. Preview redaction, failure cleanup/revocation and
platform-specific results are in [authentication](receiver-auth.md); remaining native/V22/V25
checks are recorded separately. Unverified authentication never opens an unprotected route.

Bind receiver tokens to user-confirmed local Agent instances. Loopback alone does not
establish origin. Reject SSH tunnels, forwarded Collectors and remote aggregate imports.
Unverified origin is excluded; host.name alone is insufficient. Retain only allowed numeric
and association fields. Some CodeBuddy telemetry includes model input/output; explicitly
disable Qwen prompt logs. Do not copy complete vendor debugging configurations. Provide a
preview of minimal configuration and apply it only through the user's explicit action.
Data sent while no receiver is running is not automatically recovered; Sources explains
collection windows/loss. Schedules can read saved telemetry but intermittent listening cannot
reconstruct streaming history.

Never request remote usage, balance or enterprise-analysis APIs. Costs use bundled/manual
snapshots; optional online refresh defaults off, downloading only public models.dev api.json
without local data. [Pricing](pricing.md) defines caching, failure fallback and historical estimates.
JetBrains AI Assistant/TRAE local-format investigation remains F1 and is not implemented.
Junie CLI/built-in Zed belong to M8; JetBrains Copilot supports the verified manual OTel-file
route. See the [matrix](adapters.md); nonempty native exports require separate acceptance.
Enterprise/account API references explain scope only; downloaded remote reports stay excluded.
Import only selected exports with verifiable local origin. Explain unavailable local usage
without account-login fallback. Receiver tokens/project HMAC keys use system credential stores,
never SQLite, UI, exports or logs.

<a id="数据与-ui-安全"></a>

## Data and UI security

Store statistical allowlisted fields only: no prompts/responses/tool arguments/full diagnostic
logs/API keys/account emails. Optional project grouping uses local-key HMAC to avoid directly
recoverable paths; users choose display names. UI loads shipped assets only, with limited CSP
and Tauri capabilities, no remote pages or HTML constructed from captured content. Prevent CSV
formula injection; render source names as text. The statistics database is unencrypted by
default, relying on user permissions/system disk protection; describe this accurately.

<a id="budgets"></a>

<a id="资源与性能目标"></a>

## Resource and performance targets

Proposed baseline: Windows 11 x64, four CPU cores, 16 GiB RAM, local SSD, existing WebView2,
release build. Record actual hardware and cold/warm-cache results; a development machine
does not verify the proposed baseline. [Current acceptance](../../validation/desktop-usage/current-acceptance.md)
reports measurements and unmet targets.

| Item | Target | Measurement scope |
| --- | --- | --- |
| Compressed installer | ≤20 MiB | Exclude optional WebView2 offline package; report its download/installed size separately |
| Installation directory | ≤60 MiB | Exclude user data; explain incremental shared-runtime space |
| Frontend JS/CSS | gzip ≤1 MiB | Also report uncompressed embedded size; no CDN/remote fonts/full icon library |
| Idle memory | 10-minute mean ≤350 MiB, sampled peak ≤400 MiB | All-process private bytes: app/every app WebView child, default GPU and complete visible Overview; also working set |
| First GUI import peak | All-process private bytes ≤512 MiB | One million nonempty synthetic events, actual parsing/transactions/summaries/UI; no whole-corpus loading; headless separately |
| Idle CPU | 10-minute mean <1% of one logical core | Static page/no source changes; also polling wakeups |
| Initial screen | P95 ≤2 seconds | Existing million-event DB, launch to interactive Overview without awaiting all scans |
| Common queries | P95 ≤200 ms | 366 days/50 models/20 Agents, daily filters; complex detail separately |
| Today incremental refresh | P95 ≤2 seconds | 1,000 new records across discovered local sources, including commit/UI; backfill separately |
| Observation freshness | Configured interval+65 seconds when default tasks enabled | Default one hour; shorter/custom/task frequencies and source delays measured separately |

On 2026-10-04, complete native single-window/SVG/million-record measurements justified new
memory limits: idle mean/peak=299.90/363.30 MiB, app mean=17.01 MiB, GPU=146.24 MiB.
The former 180 MiB cannot serve as a required current Windows GUI limit. The 350/400 MiB
targets leave about 17%/10% mean/peak headroom. This is a project decision, not an official
WebView2 minimum guarantee. First-import target rose to 512 MiB because its former 300 MiB
was below the observed idle GUI peak; actual parsing, transactions and UI still require testing,
without inferring success from idle/headless results.
[Microsoft performance guidance](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/performance)
describes multiprocess/GPU/driver buffers and recommends hardware acceleration, without a
universal MiB minimum. Warm empty-page diagnostics cannot establish fresh-page minimum overhead
or replace the full app. Stop collection/maintenance and stabilize the page before idle tests.
Fix window/DPI/runtime/driver, sample at least 600 seconds, meet both limits and inspect growth.
New limits do not waive leak investigation, baseline-hardware retesting, tray/background or
other native-platform checks. Label this round's development-machine results under the new limits.

Report main DB/index/WAL/throughput/query plans for one million and ten million records
separately. The larger case is not an automatic first-version guarantee. Investigate dependencies,
queries or parsing before proposing reviewed target changes. Do not omit WebView children,
runtimes or sources to improve reported results. Also measure tray/headless peak/time/wakeups;
headless collection must not create WebView children. macOS/Linux use equivalent native metrics,
without treating Windows private bytes as directly comparable cross-platform measurements.
