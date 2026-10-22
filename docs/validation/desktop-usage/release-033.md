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

<a id="oversized-codex-records"></a>

## Oversized Codex records

The 2026-10-22 read-only check reproduced one degraded Codex file, with native
`cli_version=0.163.0-alpha.11` and a retained `line_too_long` diagnostic. Three
`response_item/custom_tool_call_output` lines were 10,309,637, 17,985,371 and
10,427,395 bytes. The 8 MiB reader stopped before subsequent usage. All inspected
usage records had valid required counters; registering a version would not fix this.

The Codex reader now streams and validates complete non-usage `response_item` and
`world_state` records up to 64 MiB without retaining their bodies. It validates the
whole JSON object and UTF-8, including ignored strings, before advancing. Positional
arrays cannot establish a native envelope. Each such
record ends the scan window. Unknown types, usage records, malformed JSON, invalid
UTF-8, missing newlines and records above the cap retain their held cursor/error.
Cancellation and deadlines remain effective. Parser 8 automatically rechecks old
positions and preserves parser 6's complete-summary ownership correction.
After a held line is completed and validated, its limit error no longer keeps file
health degraded; earlier malformed-record errors and all diagnostics remain.

The initial parser-7 validation used a SQLite Online Backup including the live WAL. Only the independent statistics copy
was written; a bounded native probe used normal Codex discovery filtered to the one
affected file, with its original source read-only. Three bounded windows and one
completion window reached EOF; the fifth scan was unchanged. The file became
`active_compat`, without registering the client version. The other 563 Codex paths
were not rescanned in this probe. It added 772 events and preserved all 10,025 prior
events, including token/quality/model/ownership/cost, first observation and creation
fields. Parser references and update bookkeeping were excluded from that comparison.
Conflicts remained zero and the original `line_too_long` diagnostic remained.

An independent Python calculation compared native records through the committed
byte offset, excluding newer concurrent appends. All 1,097 unique native calls matched
the stored six counters: input 209,623,066; cache read 204,910,208; cache write 0;
output 1,033,033; reasoning 454,809; total 210,656,099. Another 49 already collected
session events absent from that retained stream remained in history.

The new integration test failed on the original reader because the post-output call
was never imported. Final targeted checks pass: two streaming validation unit tests,
six oversized-record integrations, five health/replay and nine 0.3.3 regressions.
They cover both readers, old parser metadata/unchanged cursors, exact byte positions,
repeat reads, unknown/usage/malformed types, invalid UTF-8, CRLF, escaped text, missing
newlines, partial-line completion, array rejection, transactional rollback/reopen and the byte cap.
Sanitized logs and probe code are under ignored
`build/codex-qoder-review/`; the native statistics backup/WAL/SHM were removed after comparison. This is local parser/SQLite acceptance, without a rebuilt
installed application, GUI/package lifecycle or other-platform acceptance.

<a id="source-derived-qoder"></a>

## Source-derived Qoder support

On 2026-10-22 the user confirmed that no Qoder client was installed and authorized
source-derived synthetic data. Official distributions were downloaded/unpacked only;
no installation scripts, client, login, hooks or model requests were run. This is
compiled-distribution source inspection, not a claim that the full product is open source.

| Artifact | Pinned evidence and result |
| --- | --- |
| npm CLI 1.1.76 | Verified npm SHA-512 in the [field contract](../../design/desktop-usage/adapters.md#qoder-local-support). Traced `Mm` → `SessionTranscriptWriter` → `KLo` → `bxu/kU`, and `uot/cU` normalization. Token field names alone were not used as persistence proof. |
| npm Agent SDK 1.0.55 | Checked optional assistant usage declarations independently; those declarations do not establish IDE/plugin storage. |
| IDE 1.35.0 Linux distribution | Official `qoder-ide_amd64.deb`, SHA-256 `bafe5243bc3b4fee2a37477f185365506daeed93b4e5cee52deaaeb7545d0a9c`; backend config commit `2c56c21`. Workbench context counters concern context occupancy; the native Go backend's per-call token writer remains unverified. This does not establish absence of local usage. |
| JetBrains 2026.1021.81866741 | Official plugin ZIP, SHA-256 `66e88ca23cfb42a054e6dc7d3012ea5e04504eb7cfac48ae19fddc36ccc20104`; backend 2026.1021.1 commit `275db5b`. Inspected `ChatRecord`, ACP adapter bytecode and native database column strings. The `extra` payload and native token persistence remain unverified; shared CLI integration does not certify them. |

The CLI reader first verifies a native custom-model or Credits marker within 32 MiB/200 ms,
reading complete lines of at most 8 MiB and retaining only one line. Long initial messages,
BOMs and preceding metadata do not impose a separate 32-line/64-KiB cutoff. Beyond the
bounded probe, files remain unverified. It
then imports completed native assistant responses from that file, with positive inclusive
input/output/cache buckets. Official responses need no `modelSource`: it is assigned only
for the direct custom route. Indistinguishable chat-only transcripts retain the probe.
Default or redacted zeros remain unknown. Official-channel
responses can therefore count calls without known tokens. Credits never supply cost,
currency or provider. Reasoning and source totals are not reconstructed from properties
lost during native serialization. Persisted observation times and each record's version
remain explicit. Partial/synthetic/error responses, inherited fork copies, context stats
and compaction records add no calls or tokens. Subagents retain their category.

[Source-derived fixtures](../../../desktop/src-tauri/crates/core/tests/fixtures/qoder/source-cli-1.1.76/provenance.json)
contain no native user data. Independent expectations are four completed calls, known
input 170, output 36, cache read 30, cache write 10 and complete total 197; the remaining
output-only response cannot establish a complete total. One official response has entirely
unknown tokens. Duplicate content records do not increase counts. Each row retains its
own version; the compatible 1.1.99 test is not certified by the 1.1.76 source.

Sixteen integration tests cover the full registry, shared-format routing, default/manual
roots, old probe cursors, an old misrouted owner with real Junie parent-directory
promotion and a retained invalid sibling file, partial writes and bounded windows,
field errors, unknown ownership/interfaces, repeat reads, genuine conflicts and atomic
checkpoint rollback. They also cover long/BOM prefixes, metadata/error-only first windows,
manual filenames containing subagents/segments, and unchanged parser-1 health recovery
after SQLite reopen with first observation, real conflicts and diagnostic history retained.
Parser 2 triggers this replay. Source identification permits bounded progress but does not
certify token fields or erase malformed-record diagnostics.
Ownership transfer requires no old statistical history. Source
bytes are unchanged. Initial regressions exposed a test-fixture foreign-key violation,
an internal compatibility diagnostic assertion and reused test run IDs; corrected fixtures retain real
constraints and the compatibility history. IDE/plugin negative fixtures prove rejection,
not successful token support. All real-client and other-platform runtime acceptance
remains pending.

### Codex and Qoder verification

Before the follow-up review, Windows x64, Node 24.22.0 and Rust 1.99.0: `npm run verify` exited 0 with 1,093 Rust
tests passed, nine existing tests ignored, 20 script tests and 24 frontend tests passed.
Markdown/assets, Svelte types, rustfmt, Clippy and the frontend build passed. Afterward,
the same eleven Qoder tests were strengthened with actual parent-directory discovery
and invalid sibling retention; all eleven, rustfmt and workspace/all-targets Clippy
passed again. The original 19-test M8 suite also passed with the new reader.

`npm run check:docs` checked 207 repository pairs and 21 guide pairs with no Astro
findings; `npm run test:docs` passed 39 tests. A native-environment `npm run build:docs`
produced 1,401 pages and checked 2,935 published files, local links, fragments, search,
sitemap, language roots and publication markers. Nothing was published.

The first checks retained their failures: one Markdown line exceeded 160 characters,
and two Option APIs exceeded MSRV 1.77.2. Wrapping the paragraph and using compatible
APIs fixed those causes. Existing native credential tests were denied in the sandbox;
the unchanged tests passed with isolated temporary directories and native access.
Pagefind similarly returned PermissionDenied in the sandbox and passed in the native
build environment. Logs retain these first failures and subsequent results under
`build/codex-qoder-review/`, including `verify-qoder-final-4.log`,
`qoder-tests-promoted-final.log` and `clippy-promoted-final.log`.

The follow-up review reproduced Qoder's long-prefix detection failure, stalled metadata/error-only
first windows and filename-based exclusions. It also reproduced Codex accepting positional arrays
as record objects. These checks failed before their fixes. Seven additional integration tests and
expanded streaming unit assertions cover the repairs, rollback, SQLite reopen and unchanged old
processing positions. Final `npm run verify` exited 0: 1,100 Rust tests passed, nine ignored;
20 script and 24 frontend tests passed, along with formatting, Clippy, types and frontend build.
Logs are under `build/review-codex-qoder/`, including `qoder-before.log`, `corrupt-before.log`,
`codex-before.log` and `verify2.log`. The first full verification stopped at workspace formatting;
`cargo fmt --all` corrected the test files before the passing rerun. One new Codex integration
initially expected a held-line result for an unlisted version that correctly returned incompatible;
the fixture now uses a registered version to test the held cursor independently.

The installed collector was not rebuilt/upgraded, no statistics database was cleared,
and no GUI, installer or real Qoder request acceptance is implied. Codex recovery runs
when the rebuilt collector scans existing sources, potentially over several bounded
windows.

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
