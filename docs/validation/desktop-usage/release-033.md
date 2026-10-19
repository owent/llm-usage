# 0.3.3 local data repairs

Date: 2026-10-19. Application/npm/Cargo/Tauri versions are 0.3.3. No dependency ranges
changed; the pnpm lock has no project-version field. Work covers three reported symptoms,
using read-only local Agent sources and independent copies of the statistics database.
No original Agent files or live statistics were rewritten, and no new model requests were made.

## Trends: three records requiring review

All three were Codex `codex-rollout-5` records from native version 0.163.0. A child rollout's
second-line `compacted.payload.latest_token_usage_record` carried another thread's response.
The old reader assigned the child session, the compaction time and no model. The owning
thread contained the same response ID and identical usage, with its original time and
`codex-auto-review` model. Reading both created three ownership/model conflicts.

| Record | Input including cache | Cache read | Output | Reasoning subset | Total |
| --- | --- | --- | --- | --- | --- |
| 1 | 174,049 | 169,728 | 72 | 14 | 174,121 |
| 2 | 66,172 | 63,232 | 78 | 19 | 66,250 |
| 3 | 162,642 | 160,512 | 76 | 8 | 162,718 |

Parser 6 excludes foreign-thread carries from new calls. Recovery requires the complete
old event digest reconstructed from that carry and a matching native owner response, with
identical usage, quality and revision. It corrects ownership/model/time transactionally,
preserving original observation time and diagnostics. Distinct native candidates survive
separate scan windows so later matching data cannot erase a real contradiction.

The final consistent old-database copy started with 11,208 Codex events and three conflicts.
Three passes processed 136 existing source files; the first stopped at the bounded file window,
and the second completed. All three false conflicts became zero. Every preexisting event
remained; changes to all eight token fields, quality, cost, creation time and observation time
were zero. Three old `update_conflict` diagnostics remain, alongside three
`inherited_compaction_corrected` entries. The scan also collected 240 newer Codex records
from active local sessions; these are separate from the three ownership corrections.

## Source notices

The initial database had 18 Codex files and one Kilo file marked `active_compat`, with
otherwise normal source health. A version not listed in the exact registry alone no longer
causes a compatibility badge when the expected record structure is valid and complete.
Internal versions, parser references and compatibility diagnostics remain. Malformed records,
missing required ownership and unsupported formats still produce concrete notices.

“Source files cleaned up” was based only on a recorded path not existing. The initial snapshot
contained 354 missing Codex paths and one missing VS Code Copilot path; this does not prove
which program moved or deleted them. At the user's request the label now says
“Source files cleaned up or archived”, in all ten languages. Its tooltip explains the
observed missing path without attributing deletion to an Agent. Access-check errors are
no longer classified as missing paths.
Collected history remains subject to retention; missing originals and uncollected content
cannot be recovered by rescanning. Keep a statistics backup before clearing data.

## OpenCode v2

The local 2.0.32 database has `session_v2` and `session_message`, without the legacy
`session/message/part` layout. The previous reader intentionally rejected that unverified
projection, producing the one unrecognized-file notice. Official v2.0.32 schema, writer,
normalization, fork and compaction code now support an independent reader;
[source references and field rules](../../design/desktop-usage/opencode-runtime.md).

The read-only native snapshot contains two sessions and 15 messages: seven assistant, three
user, three idle, one system and one model-switch. All seven completed assistants import
once, with their own model/provider and native completion time. Independent native sums:

| Field | Known amount |
| --- | --- |
| Uncached input | 78,811 |
| Cache read | 93,632 |
| Visible plus reasoning output | 7,888 |
| Cache write / complete input / complete total | Unknown; default-zero buckets cannot establish complete totals |

The [redacted fixture](../../../desktop/src-tauri/crates/core/tests/fixtures/opencode/real-2.0.32-windows/projection.json)
keeps only allowlisted metadata, replacing session/message IDs and excluding paths and bodies.
Its test rebuilds the database and uses the full adapter registry with a parent manual root.
The old statistics copy recovered the formerly unsupported file, importing seven events;
two additional passes added zero. Kilo's existing totals also remained unchanged.

Synthetic tests separately cover same-timestamp pages, mutable earlier IDs, persistent
invalid-row health, correction, fork inheritance/missing boundaries, incomplete/failed
assistants, compaction observations, unknown zero, old unsupported registration, complete
old Codex summaries, real conflicts, distinct scan windows and transaction rollback.
The native sample does not verify v2 Provider/API responses, actual fork/compaction activity,
all versions, migration overlap or other platforms. Two populated v1/v2 layouts remain
explicitly unsupported until their overlap is independently verified.

## Verification and first failures

### Follow-up: OpenCode unknown fields

The 2026-10-19 read-only recheck still found seven completed calls, no provider state or
message metadata on those rows, and an empty durable `event` table. Allowlisted SQL extracted
only token/model/time fields and JSON key/type names. All seven cache-write zeros, one
cache-read zero and one reasoning zero lack original field-presence evidence. The inspected
OpenCode Chat route retains raw usage in a nested object that the session writer does not
persist; the [write-chain analysis](../../design/desktop-usage/opencode-runtime.md) links each
official v2.0.32 function. This establishes a possible loss mechanism and the actual missing
local fields, not the historical protocol of each request or absent Provider responses.

Tracing the output getter also found an overconservative rule: positive visible output can
recover inclusive output even without the reasoning split. Parser 2 now uses that inverse;
zero visible output with positive reasoning cannot establish complete output. Known input
and recovered output can form a complete total without inventing reasoning zero. The UI
judges output completeness independently of input/overall completeness. The native seven
calls retain their values, displaying input ≥ 172,443, output **7,888** and total ≥ 180,331.
The source capability explanation distinguishes missing detail from unidentified calls.

New regression cases cover the inverse, all-zero usage, possible clamping, consumed old
positions, complete old hashes, repeats, first observation, real conflicts before/during
correction and transaction rollback. A mismatched old summary is retained as a conflict.
No input cache zero is converted to a reported value. Source-retention loss remains a limit:
these historical input details cannot be reconstructed by scanning the same local rows.

A fresh consistent statistics copy passed three scans against the real local OpenCode source:
the first updated seven parser references, the next two kept seven unchanged. Additions,
errors and conflicts were zero. Independent SQL found no lost events and no changes to any
existing token, quality, cost, creation or observation fields; source health remained `ok`.
Targeted Rust checks passed nine cases, frontend unit checks passed 17, and Edge browser
checks passed with exact output cells and no missing-output tooltip. English/Chinese docs
and Astro checks passed; the static build validated 1,389 pages and 2,911 output files.
The final `npm run verify` exited 0: Rust 1,066 passed / 0 failed / 8 platform-conditional
ignored, frontend 24 and script 13 tests passed, with format, Clippy, types and web build.
The Windows 0.3.3 release executable rebuilt successfully with `--no-bundle`.
The final isolated WebView2/Tauri IPC run also exited 0 with seven calls, exact output 7,888,
unchanged input/overall bounds and no ambiguous unknown-field rows. Its light-theme screenshot
was inspected. This does not certify an installed upgrade or reconstruct missing Provider data.

### Follow-up: today's OpenCode usage was invisible

A fresh read-only check found all seven native calls already present in the live statistics
database on local 2026-10-19. This was a display defect after import: the three models had
one, four and two missing complete-input values, exactly matching the “unknown fields” rows.
Share charts filtered null overall totals; total curves also plotted null even though the
independent input/cache/output components were known. No source rescan was needed to recover
those components.

Overview/trend cards, tables, shares and curves now use conservative known bounds, retaining
stored unknown totals and parser rules. OpenCode shows seven calls, input ≥ 172,443 and total
≥ 180,331. Model totals are ≥ 5,275 (DeepSeek), ≥ 93,211 (Kimi) and ≥ 81,845 (GLM).
Coverage counts moved to each affected cell's tooltip. Shares explain that percentages cover
known usage only. Hourly/grouped IPC carries the same components and coverage as summaries.
No event migration, new price calculation or original-database write is part of this repair.

The native-fixture regression now checks hour/day/week/month queries, all four chart dimensions
and hourly coverage. Frontend cases cover mixed complete/partial records, overlapping totals
versus buckets, true zero, entirely unknown usage and integers above JavaScript's safe range.
Browser acceptance checks both pages, OpenCode/model shares, curve values, field-specific
coverage tooltips, the requested source-file wording and a narrow viewport.

The final Windows 0.3.3 release executable passed real WebView2/Tauri IPC checks using
a fresh application database and isolated native fixture. Both pages showed seven calls
and the bounds above; stored complete totals remained null. Native light-theme and browser
dark/narrow screenshots were inspected. This verifies the local executable and fixture,
not an installed-client upgrade or Provider responses. The isolated environment has no
home configuration; its telemetry-setup check is outside this acceptance. Final headless
checks also passed (11 checks). Each final run exited 0.

Environment: Windows x64, Node 24.22.0, Rust 1.99.0, locked dependencies. Task logs, source
references, probes and independent databases live under ignored `build/release-033/`;
the browser harness uses `build/browser-smoke/`. The live application database was read
through SQLite's consistent backup API, including WAL state.

| Check | Result |
| --- | --- |
| `cargo test --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core --test release_033 --locked` | Exit 0; seven tests passed, including the redacted native projection. |
| Full core test suite | Exit 0 before the final native-fixture/conflict-candidate additions; final workspace gate recorded below. |
| Final three-pass local-source replay and independent SQL comparison | Exit 0; three corrected false conflicts, no preexisting field loss, OpenCode seven calls with exact known sums and no duplicate additions. |
| `npm run verify` | Exit 0; Rust 1,064 passed / 0 failed / 8 platform-conditional ignored; frontend 24 and scripts 13 passed; assets, Markdown, types, format, Clippy and frontend build passed. Subsequent frontend-only fixes passed types, browser regressions and the final release build. |
| `npm run test:browser` | Exit 0; Edge with synthetic IPC. Healthy compatible sources have no warning; real error counts and the revised missing-path notice remain visible. Ten locales and existing chart/interaction checks passed. The Chinese Sources screenshot was visually inspected. |
| `npm run build:desktop -- --no-bundle`, `npm run test:headless` | Exit 0; Windows 0.3.3 release executable built, then 11 isolated executable/SQLite checks passed. No installer or native GUI acceptance. |
| Native OpenCode v2 fixture replay | Exit 0; the actual release executable read an independent fixture database twice, with exactly seven calls, the known sums above, normal health and zero duplicate additions. Environment roots and application data were isolated under `build/release-033/`. |
| `npm run check:docs`, `npm run test:docs`, `npm run build:docs` | Exit 0; 207 repository pairs / 21 guide pairs, 39 tests, zero Astro errors/warnings/hints; 1,389 pages and 2,909 output files passed link/publication checks. No publication performed. |

First failures and their resolution are retained:

- The output-recovery follow-up first ran `cargo fmt` without `--all`, leaving core workspace
  tests unformatted; the full gate rejected them. Formatting the complete workspace fixed
  the cause. A sandboxed Node unit-test launch also returned `EPERM`; the unchanged test
  passed in the authorized normal Windows process environment.

- Initial probe job IDs collided across adapters; the probe now includes adapter/pass in each
  run ID and each acceptance run uses a fresh database. Failed zero-file runs are not acceptance.
- Bounded pages with only recognized non-counting records lost compatibility context. The
  framework now accepts explicit adapter evidence without treating empty sessions as usage.
- A native-fixture assertion initially expected no diagnostics at all; internal `latest_fallback`
  is intentionally retained. The check now rejects all other diagnostics and verifies exact totals.
- The ten-language key check caught missing English/Chinese override keys; all ten catalogs
  now carry the same keys. Clippy rejected a complex test tuple; a fixed-size array replaced it.
- The old OpenCode capability assertion required unavailable latency. It now explicitly checks
  v2 native duration, v1's continued absence and no TTFT. The fixed documentation inventory
  count was updated from 225 to 227 with explicit assertions for both new routes.
- Sandboxed Node child-process creation failed with `EPERM`; the unchanged checks were rerun
  in the authorized normal Windows process environment.
- A final concurrent Astro check/build shared `astro-cache/data-store.json.tmp`; the check failed
  with a rename `ENOENT`. The remaining owned build was stopped and both commands were rerun
  sequentially. Maintenance guidance now requires serial Astro commands in one checkout.
- A browser attempt timed out finding a Trends chart. Its saved Vite log records an HMR update
  during the run and the failure snapshot is back on Overview, with no page exception. The
  full harness passed after frontend edits stopped; the original failure and HMR evidence remain.

- The follow-up browser initially rejected the requested source-file label through an obsolete
  assertion; it now checks the label and factual tooltip separately. Screenshot review found
  one remaining ungrouped null-total path; a new regression requires 180,331 there too.
  BigInt formatting preserves decimal integers above 2^53 in the actual table/tooltip formatter.
  The final complete browser run passed.

### Tag release notes

The tag-triggered Draft Release uses the complete outer annotated tag message as its body.
Lightweight tags, empty messages and targets that differ from the built commit are rejected.
The existing draft identity, package revision/size/digest checks and asset replacement remain;
post-upload verification also compares the body with the captured annotation.

On Windows x64 with Node 24.22.0 and Rust 1.99.0, `npm run verify` exited 0: Rust 1,066
passed / 0 failed / 8 platform-conditional ignored, frontend 24 and script 20 tests passed.
The seven new script cases cover multiline/literal content, nested annotations, rereads,
lightweight/empty tags, changed targets and GitHub read failures. A separate simulated GitHub
API run executed both actual workflow scripts: create/update, seven package checks, thirteen
assets, exact body comparison and rejection of a modified body all passed. This is local
workflow validation, not a completed remote publication.

The first documentation check rejected the changed workflow's stale comment-review hash.
Reviewing the unchanged English/Chinese comments and updating the reference resolved it;
`npm run check:docs` then exited 0 with 207 repository pairs, 21 guide pairs and no Astro findings.
The initial sandboxed Node test launch returned `EPERM`; the complete verification above used
the authorized normal Windows process environment.

### Windows CI timeout and test performance

[Run 39268764091](https://github.com/owent/llm-usage/actions/runs/39268764091) built all six
platform packages successfully. The Windows Rust job exceeded its 30-minute limit;
GitHub's job annotation confirms the timeout. Its test step spent 216 seconds compiling,
then completed 77 test executables with 1,394 seconds of test execution before cancellation.
No test assertion had failed. The job could not save its Rust cache during timeout cleanup.
The Release job was skipped because a required check did not finish.

Two avoidable costs were identified in source and measured locally:

- A new statistics database executed 42 schema writes plus `user_version` in separate
  implicit transactions. Creation now commits the schema and version in one transaction.
  Failed creation rolls back all new tables and can be retried after removing the cause.
  WAL, `synchronous=FULL`, foreign keys, version checks and existing-database behavior remain.
  This follows SQLite's [transaction rules](https://www.sqlite.org/lang_transaction.html).
- Kilo sample setup now commits its reconstructed schema and rows together. The 0.3.0/0.3.2
  native sample checks query the UTC date range spanning every retained event instead of
  repeatedly traversing 2020–2100. They still verify all events, token fields, versions,
  full-registry discovery, old processing positions, conflicts, diagnostics and repeat reads.
  No tests or durability checks were removed. Production query behavior is unchanged.

CI separates `cargo test --locked --no-run` from `cargo test --locked` so compile and execution
times are visible independently. All three Rust platforms and the 30-minute limit remain.

Windows x64, Node 24.22.0, Rust/Cargo 1.99.0 measurements use fresh independent file databases
under `build/ci-windows-perf/`. A temporary harness calls the actual `Storage::open` 40 times
before and after the transaction change, including one reopen per database:

| Measurement | Before | After |
| --- | --- | --- |
| New database, median / P95 | 70.907 / 79.997 ms | 10.923 / 11.866 ms |
| Existing database reopen, median / P95 | 4.028 / 4.545 ms | 3.786 / 4.243 ms |
| `compatibility_release_032`, two tests | 44.89 s | 1.56 s |

The suite's intermediate transaction-only result was 42.55 seconds; removing irrelevant
empty dates accounts for most of that suite's improvement. The final targeted run also
passed both `codex_release_030` tests and all five `migration_v16` tests, including WAL/FULL,
repeat opening, version mismatch and the new rollback/retry case. The rollback case first
failed against the original code, which left partially created tables. These are local
measurements; hosted-runner performance requires the new tagged CI result.

Final local commands exited 0: `npm run verify` (1,067 Rust tests passed, eight existing
platform-conditional tests ignored; 20 script and 24 frontend tests passed; Markdown,
assets, types, fmt, Clippy and frontend build passed), `npm run check:docs` (207 repository
pairs, 21 guide pairs and no Astro findings), `npm run test:docs` (39 passed), and
`git diff --check`. The targeted command was
`cargo test --manifest-path desktop/src-tauri/Cargo.toml --locked -p llm-usage-core --test compatibility_release_032 --test codex_release_030 --test migration_v16`.

The first full verification stopped at a rustfmt difference in a shortened test assertion;
`cargo fmt --all` corrected it before the final run. The initial sandboxed GitHub annotation
request timed out; the approved read-only retry returned the explicit 30-minute failure.
Logs retain both first failures and the later successful checks.

No installer lifecycle or installed-client upgrade is implied. Native GUI
acceptance above is limited to the isolated OpenCode fixture. Recovery runs when the rebuilt
0.3.3 collector reads the existing sources; large
histories may require several bounded scans. Clearing the database is unnecessary.
