# Release 0.3.5 preparation

Date: 2026-10-25 (Asia/Shanghai). The user authorized the version bump, commit, annotated
tag and push to trigger packaging. The candidate includes the existing dependency/source
maintenance and Kimi K3.1 Pro pricing repair on parent revision
`f2df0a6e1e45ba1cef96494459f3a0e035dd1417`.

## Changes and version consistency

- Add independent K3.1 Pro/Highspeed CN and global offline prices, explicit Kimi Code model
  aliases, old-database import and current-reference regressions; retain historical amounts.
- Include the compatible dependency and source fixes documented in the
  [maintenance record](maintenance-2026-10-24.md): SQLite cancellation, source pagination,
  Codex conflicts, bounded Kimi reads, Qoder detection and the NSIS template update.
- Synchronize root/desktop npm manifests and locks, both Rust package versions and Cargo.lock,
  Tauri configuration and the active Plan version at 0.3.5. The shared pnpm lock contains no
  workspace version field requiring a change; dependency resolutions remain the maintained set.

The packager's `releaseVersion()` check confirms every version source is 0.3.5. Earlier
0.3.4 native/source validation remains historical; a version bump does not rerun those checks.
The [pricing record](kimi-k31-pricing.md) preserves its original verification dates and results.

## Validation and publication

Windows x64, Node 24.21.0 and Rust 1.99.1. Checks ran from the repository root and exited 0:

| Command | Result |
| --- | --- |
| Packager `releaseVersion()` | All npm, Tauri and Rust versions/locks agree on 0.3.5 |
| `npm run verify` | 1,127 Rust tests passed, nine opt-in/platform tests ignored; 20 script and 32 frontend tests passed; Markdown/assets, Svelte, rustfmt, Clippy and frontend build passed |
| `npm run check:docs` | 211 repository translation pairs and 21 guide pairs; Astro: no errors, warnings or hints |
| `npm run test:docs` | 41 tests passed, including the new pricing/release navigation entries |
| `git diff --check` | Passed; added files reviewed separately |

The first documentation test run failed because its fixed menu count was 229 while the two
new records made 231 pages. The inventory expectation and explicit assertions for both new
routes were updated; the complete 41-test rerun passed. Disposable logs and independent test
databases are under ignored `build/release-035/`; no temporary files are staged.

The annotated tag is `v0.3.5`. Its nonempty annotation supplies the Release body.
The existing `.github/workflows/ci.yml` runs on tag pushes, checks documentation/frontend/
Rust and builds six native platform/architecture packages. After every prerequisite succeeds,
it creates or updates a draft Release with six portable archives, the Windows x64 NSIS
installer and six reports. Revision, sizes and SHA-256 checks precede draft completion.

Tag push and workflow trigger are distinct from successful remote packaging or a public
Release. Remote completion and package download verification are not established by this
local preparation record. The workflow retains its existing draft publication behavior.

## macOS cancellation-test repair

The user-reported [main CI job](https://github.com/owent/llm-usage/actions/runs/40112357943/job/120926546175)
at `03443085d4900f8f268ebead2b21a465a2e3693d` failed on macOS 15 with exit 101:
281 core unit tests passed and `cancellation_rolls_back_sql_and_stops_the_interrupt_worker`
failed. Formatting, Clippy and test compilation passed. The Windows/Linux Rust jobs and
all six portable build jobs passed. The separate [tag run](https://github.com/owent/llm-usage/actions/runs/40112357959)
at the same commit passed, consistent with an intermittent test failure. These results
do not verify downloaded release packages.

The test cancelled an operation and expected a fixed 100,001-row recursive insert to fail.
The cancellation worker polls every 20 ms; the statement could finish before the next
interrupt. [SQLite documents](https://www.sqlite.org/c3ref/interrupt.html) that nearly
completed statements may finish and that an interrupt with no running statements has
no effect on later statements. SQL workload size cannot establish the required ordering.

Both SQL cancellation tests now use a scalar-function callback and a condition variable
to keep SQL running until the real worker has called `sqlite3_interrupt()`. A short
recursive step then checks the interrupt flag. A test-only counter is locked across the
interrupt and notification so an earlier idle interrupt cannot satisfy the callback's
wait. The tests require `OperationInterrupted`, automatic transaction rollback, preservation
of preexisting rows and connection reuse after the worker is joined. The earlier-cancellation
test also waits for an idle interrupt before starting the later statement. Production
cancellation and polling behavior are unchanged; the two-second wait bounds a missing
worker notification rather than requiring SQL to run for a minimum duration.

The first local callback-only insert still completed successfully after the interrupt,
because it had no later execution step checking the flag. Adding the short recursive step
made both tests pass. A mutation disabling only `interrupt.interrupt()` made both tests
fail with exit 101 despite successful notifications; the source was restored byte for byte.
Final repair checks ran from the repository root on Windows x64, Node 24.21.0 and Rust
1.99.1. Every command below exited 0; the intentional mutation failure is recorded above.

| Command | Result |
| --- | --- |
| Core test executable, `cancellation::tests --quiet`, 200 runs | 600 tests passed |
| `npm run test:rust` | 1,127 passed, nine opt-in/platform tests ignored |
| `npm run clippy` / `npm run fmt:check` | Strict workspace Clippy and formatting passed |
| `npm run lint:md` | 475 files, no issues |
| `npm run check:docs` | 211 repository pairs, 21 guide pairs; no Astro errors, warnings or hints |
| `npm run test:docs` | 41 passed |
| `git diff --check` | Passed; only the repair and synchronized records changed |

Logs are under ignored `build/ci-macos-40112357943/`. These local checks do not establish
macOS execution of the repair; post-push CI must be checked against the repair commit.
