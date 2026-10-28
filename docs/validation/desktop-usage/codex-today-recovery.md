# Recovering missing current-day Codex usage

<a id="codex-当天漏采集修复"></a>

2026-10-07. Windows 11 Pro x64 10.0.26300, statistics timezone Asia/Shanghai,
application 0.2.1, Node 24.21.0, Cargo 1.98.1; existing dependency locks.
Scripts, logs, consistent backups and independent calculations are under root
`build/codex-today/`; native sources remain read-only.

<a id="原因与修复"></a>

## Cause and correction

Before correction, the statistics database's latest Codex details were on 2026-10-06.
Today's native rollouts already contained individual token_usage_record entries.
The existing parser could read all six usage fields from 0.160.1 / 0.162.0-alpha.2;
both retain latest_fallback rather than being registered as completely verified versions.
Codex was enabled. The most recent startup collection was interrupted with
`source_window_budget_exhausted; complete lines retained for next scan`.
Registered file dates ended at 10-05; today's files had not been visited.

Scanning previously sorted ascending by date. One historical file reaching the default
32 MiB read window ended this source's scan. It still preceded newer files in later
scans, so historical backfill delayed current-day import. The correction retains bounded
reads/interruption rules and enables framework file rotation for Codex using persisted
last-visit times. Unvisited files come first; ties use descending date paths. Other
adapters retain their original order. Confirmed events, fingerprints, generations and
complete-line cursors still commit together; failures do not consume files early.
No database clearing, original rollout edits or addition of cumulative repeats.

<a id="回归与真实核对"></a>

## Regressions and native comparisons

- Three new codex_file_fairness.rs cases cover today's small file alongside large
  historical data, and reopening the database after a newer large file exhausts its
  window so an unvisited small file comes first and all files eventually complete.
  The desktop parallel entry also tests forwarding this rotation rule. Both initial
  cases failed before correction (exit 101); all three passed afterward.
- The first targeted run covered file rotation, Codex incremental reads, parallelism,
  commit rollback and interruption: 19 passed, exit 0. Repeat reads add no duplicate
  calls; incomplete lines, replacement/truncation/rename and rollback rules remain.
- SQLite Online Backup includes valid WAL data. First run the corrected Windows release
  `LLMUsage.exe --scan-once --data-dir <backup-directory>` on a consistent backup,
  enabling only Codex. The first run still had an unfinished file window; the second
  source scan succeeded. Process exit 0 alone was not treated as complete source success.
- An independent script deduplicated existing native per-call records by stable response
  identity and selected completion times on Beijing date 10-07. Cutoff
  2026-10-07 14:01:37.124 +08:00: 719 calls, input 69,722,873, cache read 65,866,240,
  cache write 0, output 408,863, reasoning output 115,244, total 70,131,736.
  Cache read belongs to input and reasoning to output; these six values are not all
  added together. All 719 identities, timestamps and six fields matched imported rows individually.
- With the application stopped, the corrected client's single-writer lock and native
  manual collection restored the actual statistics database. Both processes exited 0;
  Codex's second scan succeeded. The same cutoff matched every row. All 4,650 pre-existing
  Codex detail digests/conflict flags remained; added historical backfill was counted
  separately. PRAGMA quick_check=ok; foreign_key_check returned no rows.
- Final client comparison at 2026-10-07 14:12:53.983 +08:00: 781 calls, input 75,709,478,
  cache read 71,722,496, cache write 0, output 435,339, reasoning output 127,681,
  total 76,144,817; all identities/times/six fields matched. Another native manual scan
  retained 781 calls/76,144,817 tokens in that cutoff and all 4,650 original details.
  Both source scans succeeded with no updates/errors. Their 47/two added events include
  later calls/new backfill; ongoing-session additions are not duplicate collection.

This verifies existing local rollouts and database recovery only. Ongoing sessions'
post-cutoff usage is separate. GUI display, release installation/upgrade, other Codex
versions and calls never written to disk were not verified by this comparison.

<a id="命令与交付检查"></a>

## Commands and delivery checks

Targeted command:

```powershell
cargo test --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core --locked --test codex_file_fairness --test codex_incremental_v12 --test scan_commit_rollback --test scan_controls --test parallel_scans
```

`npm run build:desktop` exited 0 for Windows release/NSIS, with artifacts in the
existing desktop/src-tauri/target/release/. Building does not establish installation or publication.

The first full check found an oh-my-pi fork test depended on existing result order:
old file unchanged, new file complete. Rotation was explicitly restricted to Codex,
whose verified files can be read independently. Other adapters retain discovery/conflict
order, and the regression assertion was unchanged.

Final npm run verify exited 0: Rust 1,028 passed, eight platform-dependent tests ignored;
frontend 22 and scripts four passed; Svelte zero errors/warnings; fmt, Clippy,
documentation and frontend build passed. npm run test:headless exited 0: 11 checks on
the actual executable with isolated synthetic data, three calls/75 tokens. Final
documentation lint and git diff --check exited 0, without temporary artifacts in tracked files.

Initial terminal startup failed with CreateProcessAsUserW failed: 5. Retrying with the
execution environment's permissions restored it. This tool-startup error was not a
product-test result. No new GUI/IPC, cross-platform CI or release-installation acceptance.

<a id="astra-appended-session-recovery"></a>

## Astra usage appended to an existing session

On 2026-10-28, native Codex 0.164.0-alpha.11.1 records for Beijing date 2026-10-27
contained 83 `gpt-6-astra` calls: input 11,504,926, cache read 11,247,104, cache write 0,
output 81,528, reasoning output 24,139 and total 11,586,454. The statistics database
contained none of these calls. The existing six-field parser and preceding
`turn_context.model` could read them; the model name and unregistered client version
did not prevent ingestion.

The session's committed cursor was at byte 16,357,786 / next line 3,772, while its
current file had 32,295,275 bytes. The model switched to Astra at line 6,013,
2026-10-27 21:00:11.425 +08:00. Recent startup scans ended with
`source_window_budget_exhausted`; 462 registered Codex files still had status `new`.
The previous rotation prioritized that backlog over newly appended usage in a
previously consumed session.

Rotation now first visits previously consumed files whose size or modification time
changed, ordered by confirmed visits within that group. Other files retain the existing
rotation. A partial window receives no continuing priority merely because its cursor
has not reached EOF. Per-file/source budgets, parser identity, atomic acceptance and
diagnostic history remain in effect. The new model-switch regression failed against
the old implementation (exit 101). Regressions cover reopening an old database, both
complete and incomplete appended windows, and yielding back to historical files.

SQLite Online Backup preserved the original WAL state. The corrected native core
collected 1,749 additional Codex events in that copy; a repeat added 1,147 other events,
while Astra remained exactly 83 calls / 11,586,454 tokens. All 16,602 original Codex
event digests, conflict flags and creation times were preserved. The rebuilt Windows
`LLMUsage.exe --scan-once --data-dir <copy>` exited 0 and retained the same Astra
identities, completion times and six token fields. Its source report remained interrupted
because historical file windows still need backfill; exit 0 does not establish completion.

The corrected core then collected only Codex into the actual statistics database under
the same OS file lock used by the application: 1,772 added, zero updated. Read-only
comparison matched all 83 native Astra records, retained all 16,602 original Codex rows,
and returned `quick_check=ok` / no foreign-key violations. The product's day-summary
query filtered by Codex and `gpt-6-astra` returned the same calls, input, cache read,
output and total for Asia/Shanghai date 2026-10-27. Original sources and user settings
were unchanged. The rebuilt executable was checked on the backup; actual database
recovery used the native core probe, without GUI/IPC or installation acceptance.

On Windows x64, `cargo build --manifest-path desktop/src-tauri/Cargo.toml --locked
--release -p llm-usage-desktop` exited 0. The root `npm run verify` passed Markdown,
assets, script/UI tests, Svelte, formatting and Clippy, then stopped at a Copilot test's
sandbox temporary-directory rename (`PermissionDenied`, Windows error 5). That unchanged
test passed with normal user permissions; the full `npm run test:rust` rerun passed
1,105 tests with nine platform-dependent ignores. Frontend production build and
`npm run check:docs` also passed. That recovery run did not replace installed binaries or publish a release.

### Version 0.3.6 review

The release incorporates the published 0.3.5 baseline. Review found two additional cases:
unknown modification times could promote unchanged files, and EOF with an unfinished
JSONL line could leave its later completion behind historical backfill. The comparison now
uses known timestamps only. A transactional `file_read_complete` cursor marker distinguishes
EOF from exhausted read windows; legacy cursors still use offset equal to saved size.
Subsequent bounded windows clear the marker, so further appends cannot keep an unfinished
window ahead of historical work. No schema migration or usage/parser identity change is needed.

Eight added regressions bring `codex_file_fairness` to 12 tests. They cover complete and
partial appended windows, newline completion across reopen, missing modification times,
same-size rewrites, old cursors, failed-commit visit/cursor rollback, the parallel application
entry and model-filtered daily totals. The focused collection/rollback/control/parallel suite
passed 21 tests. AI maintenance guidance and both scheduling specifications follow these rules.

Version 0.3.6 passed `npm run verify` on Windows x64 / Rust 1.99.1: 1,135 Rust tests,
nine conditional ignores, 20 script tests, 32 frontend tests, Svelte, formatting, Clippy
and production frontend build. `npm run check:docs`, all 41 documentation tests and
`npm run build:docs` passed (1,407 pages / 2,947 published files checked). Both npm
and frozen pnpm restores passed; package, Tauri and Cargo versions agree. These checks
do not add GUI/installation acceptance; native release packaging runs in tag CI.

Task artifacts are under `build/astra-recovery/`. Original Agent files are read-only;
only allowlisted usage/model/time/version fields enter validation output.

### Native paths in scan priority

On 2026-10-28, [issue #6](https://github.com/owent/llm-usage/issues/6) identified that
`files_in_scan_order` used a normalized database identity as the metadata path. Stripping
the Windows verbatim prefix turns a canonical UNC identity into `UNC/server/share/...`,
which is not the original filesystem path. A failed metadata read then loses append priority.

The ordering map now retains each discovered `PathBuf` alongside its last visit and reads
metadata through that original path. Stored identities, checkpoints and rotation rules remain
unchanged. The regression uses a real Windows verbatim path with a trailing-dot directory;
it first confirms that the original path can be read while its identity string cannot. The
original implementation failed because history was read before appended usage. After repair,
the test verifies reopen, stable identity, the appended model's call/tokens, later historical
rotation and no duplicate call. The Unix variant uses a literal backslash in a directory name.

The Windows collection/rollback/control/parallel suite passed 22 tests, including all 13
Codex file-fairness tests. The initial repeat-scan assertion needed a second unvisited historical
file: equally recent visits intentionally retain discovery order. No SMB share was mounted or
changed; real UNC-share access and the Unix variant were not locally executed. Logs are under
`build/issue-6/`.

Final Windows x64 / Rust 1.99.1 checks on both issue repairs passed: `npm run fmt:check`,
`npm run clippy` and `npm run test:rust` exited 0, with 1,139 Rust tests passed and nine
conditional ignores. Markdown lint and `npm run check:docs` also exited 0. This adds no
GUI or installation acceptance.

### Inherited-only files in parallel collection

On 2026-10-28, two real Codex files from 0.164.0-alpha.11.1 and 0.165.0-alpha.1.2
were marked incompatible. Each contained nine complete native records, including a
foreign-thread compaction carry, but no new per-call usage. Direct parsing correctly
validated the inherited record without counting it. The desktop parallel runner's
`RootAdapter` did not forward `has_validated_usage`, so its default check rejected
the valid non-counting result and left no checkpoint.

The wrapper now delegates that hook to the underlying adapter. No version registration,
parser identity or token accounting changes are required. Malformed carries still fail
validation, and inherited values remain excluded from the receiving session's totals.

Three `codex_parallel_compat` regressions exercise the full registry with isolated native
discovery and the parallel entry: both versions alongside malformed usage, reopening an
unchanged old incompatible state, and failed-commit rollback followed by recovery. They
preserve first observations and diagnostic history and verify that repeated reads add
no usage. The old-state recovery and rollback checks failed before the wrapper repair.
The fixtures use native `.codex` discovery and distinct per-adapter run IDs, matching
the desktop entry.

On an independent SQLite Online Backup copy including WAL, both files became readable;
a second pass reported unchanged. All 25,163 original usage rows retained their content
digests, conflict flags and creation times. Settings, first observations and diagnostics
were preserved; `quick_check=ok` and no foreign-key violations. The Windows x64 / Rust
1.99.1 targeted parallel, Codex ownership and Qoder suite passed 35 tests, exit 0.

The same native core then recovered only those two files in the actual database under
the application's OS file lock. Both passes succeeded with zero added or updated usage;
the incompatible count fell from two to zero. All 25,163 original rows, settings, first
observations and diagnostic history were retained. Source-file SHA-256 values were
unchanged, and integrity/foreign-key checks passed. Recovery used the parallel core
entry; it did not replace an installed executable.

`npm run fmt:check`, `npm run clippy` and `npm run test:rust` exited 0: 1,142 Rust tests
passed and nine conditional tests were ignored. Markdown lint, `npm run check:docs` and
all 41 documentation tests passed.

`npm run build:desktop` exited 0 and produced the Windows x64 executable and NSIS
package. Three real executable `--scan-once` runs with isolated synthetic sources
confirmed two readable inherited-only files, one still-incompatible malformed file,
old-state recovery, retained first observations/diagnostics and zero counted usage.
Source bytes and database integrity remained intact. These executable checks used the
local 0.3.6 repair build before the version bump; no installed executable was replaced.

Version 0.3.7 includes this correction, the Qoder detection-cache retry-lock repair from
issue #5 and the native-path scan-priority repair from issue #6. Root/desktop manifests,
npm locks, Tauri configuration and both Cargo package versions/lock entries agree.
The pnpm lock has no application-version entries and requires no dependency change.
On Windows x64 / Rust 1.99.1, `npm run verify` passed for 0.3.7: 1,142 Rust tests,
nine conditional ignores, 20 script tests, 32 frontend tests, Svelte, formatting, Clippy
and the production frontend build. `npm run check:docs` also passed. Release preparation
logs are under `build/release-037/`; cross-platform packages are built by tag CI.

Task artifacts are under `build/codex-incompatible/`; source inspection is read-only and
outputs only allowlisted structure/version/usage fields. Sensitive backup copies were
removed after verification. This does not establish GUI, installation or other-platform
acceptance.
