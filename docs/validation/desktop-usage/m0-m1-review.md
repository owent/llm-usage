# M0/M1 commit review and regression fixes

<a id="m0m1-提交审查与回归修复"></a>

Reviewed 3588345 (M0) and 9394264 (P1/M1) on 2026-09-24, starting with a clean working
tree. These changes were not committed or pushed. Environment: Windows x64, PowerShell
7.6.6, Node 24.21.0, Cargo/Rust 1.98.0; existing dependency locks retained.

<a id="已修复的问题"></a>

## Fixed problems

| Priority | Trigger and effect | Fix and regression |
| --- | --- | --- |
| P1 | Daily/native interval summaries counted estimated fields as known usage; derived total lost estimated quality | Sum reported/derived per field; preserve other reported fields and call counts; validate value/quality consistency |
| P1 | Codex/ZCode/Kilo invented known splits when cache creation/read/reasoning was missing; inconsistent source total became normalized total; cache addition could overflow/panic | Keep missing fields unknown; separate normalized total/source_total; checked addition; i128 integer ratio pairs |
| P1 | Event API accepted cumulative/interval/quota records and added them daily; mixed-source batches advanced the wrong cursor; absent/ended jobs could submit | Dedicated storage APIs; verify batch source, job ownership and running state inside transaction |
| P1 | Identity components containing # could collide; changing only observed_at on rescan caused conflicts | Escape identity components; ignore observation time in content comparison; retain v1 hash/alias compatibility |
| P1 | Same-revision interval conflicts overwritten; diagnostics/usage not atomic; data revision unchanged | Keep old value and record uncertain updates as conflicts; atomic summary/diagnostic/revision; unknown call totals return None with known/unknown row counts |
| P1 | After explicit reset, a new cumulative value above the old value produced a false delta; late values moved baseline | Check sampling order/reset information first; keep baseline for late/same-time conflicts; separate new series |
| P1 | Retention failed on event_aliases foreign keys; missing restart cutoff restored expired data; hard deadline missed diagnostics/intervals | Delete expired-event aliases first; persist forward-only retention floor; apply hard deadline to diagnostics, quotas, native intervals and rescans |
| P2 | Week/month tokens respected selection but sessions used full natural period; days without sessions lost; detail/exclusion filters differed | Same date range for sessions/active days; activity independent of session; consistent filters/read snapshot |
| P1 | journal_mode changed before rejecting newer schema; old daily totals retained calculation errors | Read version before settings changes; transactional v2 identity/alias migration and recomputation where details remain; full rollback on failure |
| P1 | Installed M0 discovery depended on build-machine source paths; concurrent probes shared temporary DB | bundle.resources and Tauri resource_dir; independent temporary SQLite directory per probe, cleaned afterward |
| P1 | Linux Rust job lacked Tauri libraries; artifact checker treated .app as a file and missed it | Add dependencies; Node checks actual outputs and records size/SHA-256/revision; archive .app before upload to retain permissions/symlinks |

review_regressions.rs adds 27 core regressions, desktop entry adds two tests and
bundle-report.test.mjs adds three. Core tests use real temporary SQLite files, covering
concurrent reads/writes, v1 upgrade, failed-migration rollback, sealed missing details and
failure triggers. All first 17 cases failed on the original implementation and passed
after fixes. Later interval-conflict/atomicity cases likewise reproduced failures first.

<a id="兼容与核验范围"></a>

<a id="兼容与证据边界"></a>

## Compatibility and verification scope

- v2 recomputes old days with retained details. Sealed estimated partitions cannot recover
  known per-field values: keep calls, mark tokens unrecoverable and record
  sealed_estimate_unavailable, without guessing zero.
- Under a hard deadline, native intervals with unknown start or crossing the cutoff cannot
  be split exactly; remove the whole interval and block ordinary rescan restoration. v1
  did not save cleanup cutoffs; deleted history without sealed rows cannot reveal an old cutoff.
- Adapters, cross-source aliases/deduplication, timezone rebuilds, consistent pre-migration
  backups/space checks, managed-backup cleanup and capacity/performance tests remain planned
  work. Table tests do not count as acceptance of those features.
- Resource mapping was checked against locked tauri-utils 2.9.3 configuration definitions
  and tauri 2.11.6 path/desktop.rs, including platform resource directories.
- Three-platform GitHub CI had not run; local archive tests do not verify macOS desktop
  execution or downloaded artifacts.

<a id="命令与结果"></a>

## Commands and results

Working directory is repository root unless stated. Initial Windows sandbox launch failed
with CreateProcessAsUserW failed: 5. Database reads/tests began after the permitted execution
path worked; this launch error was not a product-test failure.

| Command | Exit | Result |
| --- | --- | --- |
| Before fixes: cargo test --manifest-path desktop/src-tauri/Cargo.toml --locked -p llm-usage-core | 0 | Original 72 tests pass |
| First cargo test ... --test review_regressions | 1 | All 17 new cases reproduce failures |
| Final cargo test ... --test review_regressions | 0 | 27 passed / 0 failed |
| npm run verify | 0 | 99 core + 2 desktop + 3 script tests pass; 34 Markdown files clean; 67 generated/82 total resources verified; svelte-check clean; fmt/clippy/frontend build pass |
| npm run build:desktop -- --bundles nsis | 1 | npm 12 rejected nested-script --bundles forwarding; use locked CLI directly |
| node node_modules/@tauri-apps/cli/tauri.js build --bundles nsis (desktop) | 0 | Windows x64 release/NSIS; installer 1,894,207 bytes |
| 7z l desktop/src-tauri/target/release/bundle/nsis/llm-usage-m0_0.1.0_x64-setup.exe | 0 | Includes 190-byte sample-data.txt; bundled/source sample SHA-256 match |
| node desktop/scripts/bundle-report.mjs desktop/src-tauri/target/release/bundle | 0 | Actual NSIS size/SHA-256 in bundle-report.json |
| git diff --check | 0 | No whitespace errors |

Installer SHA-256: 4e382336b14980fd5a400061a8d958bd402ff7d0886ffa714e43187a78cf43da.
Outputs remain in ignored desktop/src-tauri/target/release/bundle/.

No real agent started/called, no private session read, and the package was neither installed
nor published.
