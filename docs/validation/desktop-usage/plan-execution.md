# Implementation and acceptance results for remaining work

<a id="剩余计划执行与验收"></a>

Date: 2026-10-04; version 0.2.1, schema 11; cwd repository root. Windows 11 Pro x64
10.0.26300; Ryzen 9 9950X3D (16 cores/32 threads), about 125 GiB RAM; Node 24.21.0,
Rust 1.98, Tauri CLI 2.12.0, WebView2 154.0.4258.53. Development-machine results do
not verify the proposed four-core/16 GiB baseline. This is a completed development
validation record; [Plan.md](../../../Plan.md) maintains the active execution plan.

<a id="实施内容与要求"></a>

<a id="实施与合同"></a>

## Implementation and requirements

- Daily queries add rebuildable derived tables. Read-only connections use them only for valid
  partitions or partitions established empty by authoritative tables. Provider filters merge
  identical source/session identities before DISTINCT; model/Agent filters use expression
  indexes. Original events/revisions/cursors/unknown fields remain. Older writers invalidate
  derived data and remove derived identities. Regressions cover old-layout preparation,
  cross-connection writes, rollback, DST, retention/clearing and optional-SUM overflow.
  See [query design](../../design/desktop-usage/query-acceleration.md).
- Six chart types use SVG; features/themes/selections retain actual query results.
- Windows optional close-to-tray, explicit exit, power-saving pause and directory notifications
  are implemented. Watch at most 32 directories; coalesce after two quiet seconds; pause releases
  handles. Fixed daily/weekly sources are excluded. Failures retain interval polling.
- GUI per-source intervals use a monotonic clock while UTC deadlines remain persisted. Global
  pause covers startup, every source and residual system triggers. Resume performs one catch-up
  scan; manual collection still reads every enabled source.
- Cooperative time limits default to 30 seconds per source and 300 seconds per automatic round.
  Transient I/O/SQLite locks retry at most three times; waits of 5/15/30 seconds run only when
  remaining time permits. Interrupted final state can persist and later resume from the cursor.
  Pause takes effect before settings wait for a write lock; saves run asynchronously. Retry waits
  and subsequent reads check pause/deadlines. Two source-instance slots run concurrently, including
  distinct roots of one Agent. Jobs for the same instance merge before parsing; parsing occurs
  outside the database lock and short writes remain serial. Bounded JSON/JSONL reading/parsing,
  SQLite VM/backup pagination, authoritative archives, retention and cost transactions cooperate;
  blocking OS calls cannot be forcibly interrupted. Default file window: 32 MiB; complete lines
  may commit before continuation. Interrupted/unvisited/merged jobs do not advance deadlines.
  Fingerprints/generations and events/cursors share a transaction; same-size replacements after
  failure are detected again. Prior results/revisions remain.
  See [scheduling rules](../../design/desktop-usage/scheduling.md).
- Telemetry retains explicit attribute keys only. Real HTTP checks verify bad gzip returns 400,
  oversized bodies/decompression bombs 413, global 120 requests/minute/four active connections,
  overflow 429; rejected requests are not persisted. Windows Claude/Codex logs and isolated
  CodeBuddy CLI 2.98.0 traces use per-source tokens/current-user system credentials. CodeBuddy
  requires the npm manifest beside the first PATH launcher and fixed-release generic headers
  with %20 encoding; other versions receive no inferred support. Previews show placeholders;
  background checks are read-only. Authentication precedes continued body reading; duplicate
  headers/wrong paths/Origin/forwarding headers are rejected. Failure cleanup, revocation,
  source isolation and reading system storage after restart are implemented. Real exporters,
  other platform stores, sampling and cross-format association remain incomplete; V22/V25
  are unfinished. See [authentication requirements](../../design/desktop-usage/receiver-auth.md).

<a id="已执行检查"></a>

## Checks executed

Query-specific cargo test --offline --manifest-path desktop/src-tauri/Cargo.toml
-p llm-usage-core --test query_cache exited 0: seven checks. Receiver-specific
cargo test --offline --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-desktop
otel_receiver::tests passed with the full suite: 17 checks, including real HTTP limits,
compression and authentication. Pause checks: seven Codex regression cases and one pause-intent
case, exit 0. Pause after transient failure avoids the next retry wait and adds no usage/cursor;
resume reads that source successfully. Multiple pending pause-save requests take effect
independently while a database write lock is held; failure/cancellation releases only its request.

Nine focused checks cover parallelism, bounded reads/SQLite/maintenance rollback and same-size
replacement recovery after failure. Full regressions also cover interrupted paginated backup,
ZCode authoritative-archive recovery, Codex continuation and Kilo lock-wait deadlines.
npm run test:receiver exited 0: eight checks using actual release/IPC/HTTP/Windows credentials.
Port conflicts retain original configuration/intent; three independent credentials apply and
authenticate. CodeBuddy protobuf reaches isolated traces only, excluding logs/main spans.
Revoking one leaves others usable; after revoking all, old tokens fail and original configuration
returns. Owned credential remnants: zero. Installation/exporter inputs are synthetic; no Agents
or model calls were started and real user configuration was untouched. This does not verify
real product export. cargo test ... -p llm-usage-desktop windows_credential_store_roundtrip
-- --ignored exited 0: one native system-store create/read/path-restriction/exact-delete check.
Preview redaction/failure cleanup and HTTP missing/duplicate headers/wrong routes/forwarding/
revocation checks also passed.

Full npm run verify exited 0: Rust 889, frontend 20, scripts three, no type errors/warnings,
fmt, Clippy -D warnings and frontend build passed. Six explicit/environment tests remain ignored
by default. npm run build:desktop and npm run test:headless exited 0; headless: 11 checks,
three events/75 tokens. npm run test:browser exited 0: Edge with simulated IPC checked grouped
hour charts/tooltips/currency cards/narrow details/partial-data pie/configuration batch apply,
retry/revoke/ten languages/themes/timezones/filters/pagination. Final NSIS: 3,901,754 bytes
(3.72 MiB); executable: 9,856,000 bytes; frontend JS/CSS gzip: 363.77/9.50 kB. Built, not installed.

npm run test:desktop -- --runs 20 exited 0: 17 actual WebView2/IPC checks; small-database
first-screen P95 over 20 runs: 770.09 ms. Windows UI Automation checked control names at native
DPI 144 (150%); native WM_CLOSE verified tray hiding and disabling the option restored the
window. The script's --force-renderer-accessibility flag is not a product default and does not
verify Narrator/NVDA. Real file notifications imported appended records despite a global one-day
interval; pause prevented automatic collection while manual refresh still read them. An ordinary-user
OS minute task independently verified new usage before GUI startup.

node desktop/tests/native-incremental.mjs --runs 20 exited 0: append 1,000 records per round
to a million-row database until visible card update; 20-round P95 717.98 ms, final 1,020,001
events. Token totals/cache invalidation/no new usage on repeat passed. An isolated copy disabled
retention to prevent day rollover cleanup changing fixed benchmark counts; the original million-row
baseline was untouched.

<a id="查询与导入"></a>

## Queries and import

Run release bench_v20 &lt;synthetic-directory&gt; &lt;1000000/10000000&gt; --query-only --filtered from
the root. Add --prepare-indexes for old databases; this mode verifies a nonempty synthetic/bench
source before writing and rejects real databases. Benchmark: 366 days, 50 models, 20 Agents;
20 runs per query clear application-connection caches, without clearing OS file cache. Final
measurements ran serially after full checks/build.

| Query P95 / rows | One million | Ten million |
| --- | ---: | ---: |
| No dimension filter | 47.05 ms | 133.86 ms |
| Model | 35.83 ms | 87.95 ms |
| Agent | 50.02 ms | 196.62 ms |
| Provider | 52.83 ms | 163.33 ms |
| Model + Agent | 24.60 ms | 145.96 ms |
| Application-cache hit | 0.024 ms | 0.026 ms |
| 200-row detail page | 1.15 ms | 1.16 ms |

These queries meet 200 ms on this development machine; the ten-million-row Agent case has
little margin. They do not verify every filter combination/hour/week/month/proposed hardware.
Initial old-database preparation took about 8.56/49.00 seconds including indexes/derived tables;
subsequent preparation/planning checks: 286/2,269 ms. Database sizes 1,079.4/9,461.8 MiB, WAL
zero; increases of 173.1/1,427.7 MiB over 906.3/8,034.1 MiB retain the indexing cost. Initial
upgrade preparation is excluded from repeated native startup measurements on prepared databases.

node desktop/tests/bench-import.mjs exited 0. Current release core commit_batch imported
1,000,000 normalized events in 50,000-row batches: 184.5 seconds, 5,421 rows/second. All child
processes sampled every 500 ms: private-bytes peak 53.45 MiB, working set 62.96 MiB. No WebView
was started; bounded normalized writes do not verify all native formats or full GUI import peaks.

Full concurrent regression found early Winsock closure could lose rejected-connection responses.
The fix sends the closing sequence, then discards remaining input within cumulative 100 ms/64 KiB,
without parsing/persisting it. JSON responses consistently use CRLF/Content-Length. Closure
reference: [Microsoft Winsock](https://learn.microsoft.com/en-us/windows/win32/winsock/graceful-shutdown-linger-options-and-socket-closure-2).

<a id="原生资源与内存目标"></a>

## Native resources and memory targets

node desktop/tests/native-scale.mjs --runs 20 --idle-seconds 600 --ui-cancel exited 0.
Prepared million-event database: 20-run first-screen P95 1,260.72 ms. Two actual IPC cancellations
and settings-button cancellation preserve events/revisions/cursors. Default GPU rendering sampled
601.19 seconds, 118 samples, at most seven processes including the executable and all application
WebView children.

| Metric | Measured | Target status |
| --- | ---: | --- |
| Whole-process private bytes, mean / peak | 299.90 / 363.30 MiB | Old 180 MiB unmet; new mean 350 / peak 400 MiB limits met on development machine |
| Whole-process working-set peak | 525.79 MiB | Separate metric; does not replace private bytes |
| Idle CPU, one logical core | 0.244% | Below 1% |
| First-screen P95 | 1,260.72 ms | Below two seconds; excludes initial old-database preparation |
| Application scheduling-loop count | 1,202 | About twice/second; not all OS wakes |

| Process role | Mean private bytes | Role peak |
| --- | ---: | ---: |
| Executable | 17.01 MiB | 17.49 MiB |
| WebView browser | 43.38 MiB | 45.38 MiB |
| WebView GPU | 146.24 MiB | 187.21 MiB |
| WebView renderer | 71.13 MiB | 92.04 MiB |
| WebView utility | 19.21 MiB | 19.79 MiB |
| Other WebView | 2.93 MiB | 3.37 MiB |

Role peaks may occur in different samples and cannot be added into a whole-process peak.
Each sample asserts role sum equals whole-process usage; raw command lines/paths are not saved.
GPU accounts for about 48.8% of mean usage, the largest role locally. This measurement cannot
separate runtime/driver overhead from page overhead. An earlier --disable-gpu diagnostic had
one startup/31 seconds, mean 183.48/peak 185.54 MiB, still above target; it does not replace
default configuration/20 startups/full-duration acceptance.
[Microsoft WebView2](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/webview-features-flags)
documents these flags for testing/debugging, without justifying product-default changes.

On 2026-10-04 the user authorized adjusting resource limits using current features/measurements.
The application has one window, full overview, SVG charts, SQLite and bounded query caches;
no concurrent multiple WebViews or bundled Node/Python services. Forty-one adapters/postprocessing
do not imply loading every source into idle memory. The executable uses about 17 MiB; GPU/browser/
renderer dominate. A hard 180 MiB limit does not fit the current Windows GUI.
[Microsoft performance guidance](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/performance)
describes multiprocess/GPU-driver/buffer/content overhead and recommends retaining hardware
acceleration, without setting a universal minimum. The local GPU footprint cannot all be labeled
unoptimizable fixed overhead. Revised engineering limits: ten continuous minutes of whole-process
private bytes mean <=350 MiB, sampled peak <=400 MiB, allowing about 17%/10% above measured values;
the current machine meets them. Initial GUI import has a separate <=512 MiB peak. Working set,
headless and tray-background results stay separate. Original measurements/old-target failures
are retained. native-scale.mjs --enforce-budget checks mean/peak/full duration; software rendering
or warmed blank pages do not verify product limits. Proposed four-core/16 GiB hardware, fixed
runtime/driver/DPI, sustained growth and other platforms need rechecks. Relaxed limits do not
waive leak/unbounded-loading investigation. The same raw samples show first-120-second mean
309.06 MiB and last-120-second mean 299.93 MiB: no sustained growth observed in ten minutes,
without proving long-term absence of leaks. New assessment: build/plan-continuation/memory-budget.json;
original samples remain unchanged.

The million-row first GUI import separately uses npm run test:import: empty database, 100
isolated Codex roots, 10,000 nonempty synthetic events per root, default product retention,
real button through visible cards/charts, independent 500 ms process sampling. The initial
600-second limit expired at 950,000 committed rows/97 registered sources while work was still
running; exit 1, not accepted as a peak check. Failure:
build/plan-continuation/native-first-import/1791123652310/failure.json. After confirming ongoing
commits, the complete observation window was extended to 900 seconds. This changes observation
duration, preserving product per-source time limits/two-second incremental target/512 MiB peak.
Script initialization also fixed retention parameters, Windows inline sampling, reloading UI
language after backend save, and waiting for the empty-database refresh button. A Chinese number
abbreviation misread after one million commits was a test-script failure, not product usage
failure; its record remains.

Final npm run test:import exited 0: empty database to visible cards/charts in 668.55 seconds.
Independent expectations of 1,000,000 calls/15,000,000 tokens match actual summary; repeat scan
still one million rows. Continuous sampling: 669.86 seconds/1,031 samples/configured 500 ms
interval including overhead/at most eight application processes. Private bytes mean 303.42/peak
371.09 MiB meet the 512 MiB import limit; working-set peak 586.97 MiB is separate. Executable
private-bytes peak 72.57 MiB, GPU 165.85 MiB, renderer 80.96 MiB; role peaks are not added.
No script exceptions or observed external page HTTP; this does not establish OS-wide process
outbound behavior. Result: build/plan-continuation/native-first-import/1791125560874/result.json.
Synthetic Codex JSONL with 100 input roots/default retention does not bypass parsing like
normalized bulk writes. Their throughput is not directly comparable and does not verify other
real Agents/versions/very large native files.

node desktop/tests/native-scale.mjs --runs 1 --idle-seconds 600 --blank-page also exited 0.
After warming the full page on the same million-row database, it navigated to about:blank and
requested GC with default GPU. Sampling 600.36 seconds/118 samples/at most seven processes:
private bytes mean 270.63/peak 283.63 MiB; GPU mean 148.12, renderer 41.81, executable 16.26 MiB;
idle CPU 0.083% of one core. Application scheduling counts cannot be read after navigation and
are explicitly null. Usage still exceeds 180 MiB, showing a high warm-WebView GPU footprint;
it does not establish minimum cold blank-page overhead or proposed hardware performance, and
blank pages do not count as product acceptance. Fresh minimal pages/proposed-machine native
rechecks remain needed. Result: build/plan-finalization/native-scale/1791121549201/result.json.

Native/browser checks observed no external page HTTP/script exceptions. Only observed WebView
requests are covered; these are not whole-process outbound audits. Final results:
build/plan-finalization/native-scale/1791109899513/result.json; other native/incremental results:
build/plan-completion/native/1791123291549/ and build/query-next/native-incremental/1791121379123/.
Final build/full-check logs: build/plan-continuation/build-final.log, verify-final.log; receiver
authentication: build/plan-continuation/native-receiver/1791123187714/result.json.

<a id="本机来源条件"></a>

## Local source conditions

Read-only actual discover through 41 built-in adapters used known roots, outputting file counts
and bounded format/version references without paths/session identities/bodies. Read-only Zed
SQLite COUNT found zero threads; an empty DB does not verify usage. Thirty absent default roots:
claude, gemini, qwen, cline, dsh, hermes, openclaw, opencode, mimo-code, zoo, aider, junie, xum,
droid, amp, grok, roo, goose, crush, jcode, workbuddy, gajae-code, commandcode, continue, atomcode,
kiro, antigravity, qoder, copilot, otel. Existing real manual WorkBuddy checks remain valid;
this probe describes discovery/read conditions. Highest database versions, readable formats
and directories do not verify per-call usage.

No Agents/model requests/WSL/containers were started; real IDE settings were not changed to
manufacture samples. Nonempty real samples, other native platforms, proposed hardware baseline,
installation/upgrade/uninstall/signing/publication remain unverified. Installation was skipped
under earlier instructions; no push/deployment/publication ran. F1/full detail Merge/spending
reminders remain deferred. Trace SQLite still needs fixed full schema/attributes and nonempty
samples. Fixed GitHub/raw pages could not be retrieved; independent curl timed out at 30 seconds.
Table names did not justify attribute/incremental/source-selection inference; earlier C04
documentation references do not establish complete implementation/real acceptance here.

Closing checks: npm run lint:md exited 0, 180 Markdown files; 180 local paths/anchors in 19
changed documents had zero errors. python -X utf8 .../quick_validate.py .agents/skills/ai-maintenance
exited 0, Skill is valid. Description unchanged; no model-routing evaluation claimed.
git diff --check exited 0. Read-only inspection of owned build namespaces found zero application/
WebView processes/scheduled tasks. User objects were untouched; git status had no temporary
artifacts; other tasks' wording edits were preserved.
