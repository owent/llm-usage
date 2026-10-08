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
