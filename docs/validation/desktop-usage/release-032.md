# Version 0.3.2 compatibility and telemetry wording checks

<a id="032-兼容读取与遥测文案核验"></a>

Scope: repair Codex/Kilo compatibility notices using version-specific native samples,
clarify the local OTel description and set application manifests to 0.3.2.
The local checks below preceded tagging and remote CI; subsequent tagged-build results
are recorded separately. Native source extraction is read-only and follows
the [redaction requirements](../../design/desktop-usage/implementation-readiness.md).

## Compatibility notices

The local application database contained 66 Codex files and one Kilo database marked
`active_compat`. This notice means the reader recognizes the format while the owning
client version remains unverified. It is separate from a record error. Codex versions
0.159.0-alpha.12.1, 0.160.0, 0.160.1 and 0.162.0-alpha.17.2 were missing from the registry.
Kilo had 27 unregistered session versions, including 7.8.3 and 7.8.7. Earlier empty
7.8.3 sessions did not verify usage; the present native sessions contain actual calls.

Each added version has an independently extracted native sample and calculated
expectations in `tests/fixtures/{codex,kilo}/releases-032/` under the core crate:

| Codex version | Calls | Input including cache | Cache read | Output including reasoning | Reasoning | Total |
| --- | --- | --- | --- | --- | --- | --- |
| 0.159.0-alpha.12.1 | 3 | 81,454 | 58,368 | 756 | 71 | 82,210 |
| 0.160.0 | 1 | 22,600 | 4,864 | 203 | 140 | 22,803 |
| 0.160.1 | 1 | 35,556 | 17,152 | 138 | 62 | 35,694 |
| 0.162.0-alpha.17.2 | 14 | 392,480 | 350,720 | 1,443 | 345 | 393,923 |

Kilo's 27 complete native sessions contain 787 assistant calls. Exact added versions:
7.3.42, 7.3.45, 7.3.46, 7.3.49, 7.3.50, 7.3.54, 7.3.63;
7.4.1, 7.4.2, 7.4.5, 7.4.7, 7.4.11, 7.4.13, 7.4.15, 7.4.16, 7.4.17,
7.4.20, 7.4.22, 7.4.23; 7.5.6, 7.5.14; 7.6.0, 7.6.2; 7.7.5, 7.7.12; 7.8.3, 7.8.7.
The per-version expectations retain all five exclusive buckets, derived totals,
missing reported totals and native session snapshots. Twenty-six snapshots match;
7.3.42 has a zero cumulative snapshot despite positive detailed usage. Its mismatch
remains an audit diagnostic and is not used to discard valid calls or verify a version.

Bodies, paths and identifiers were removed or anonymized before samples entered Git.
Provider identifiers in the new Kilo samples are replaced with a constant; model names,
times, token numbers and the owning session version are retained. These samples verify
the native CLI/database formats only, without extending IDE or telemetry support.

`compatibility_release_032.rs` creates genuine prior event hashes using fallback parsing
references, consumes cursors, then reruns discovery through the complete built-in registry.
It checks parent-directory discovery, each token field, known-version references, unchanged
statistics/first observations, retained diagnostics, no conflicts and idempotent repeats.
Unknown/missing versions, mixed databases, empty sessions and invalid rows retain their
existing checks. Registering one version does not certify adjacent versions.

The real old-database check also exposed a file-continuity defect: a stable 63,974,896-byte
Codex file repeatedly stopped at byte 33,554,261. The stored creation fingerprint differed
from the file's current creation time. A reread saved its size, modification time and
content fingerprints but omitted the new creation time, causing another generation reset
on every scan. The framework now saves creation time with the other fingerprints, events
and cursors in the same transaction. An independent regression failed with one call
instead of three before the fix; it now verifies one reset, progress after database
reopening, all three calls and unchanged source bytes.

## OTel wording

The Simplified Chinese description now reads:

```text
读取本机遥测文件。统计同一会话、同一天的 Copilot 用量时，优先使用已核验且匹配的遥测导出数据，避免与会话记录中的用量重复计算。
```

“Native contribution” described usage selected for statistics. “Native extraction” names
an operation and does not explain the selected data. The revised text identifies exported
data and usage from session records, retaining verification, matching and deduplication
conditions. All ten UI locales and the paired telemetry/client guides are synchronized.
Selection still requires the same original host, user, session and local day; native records
remain stored, sealed partitions keep their existing selection and other unverified exports
are isolated. There is no collection or deduplication behavior change.

## Version and validation

Root and desktop npm manifests/locks, both Rust package manifests, their Cargo lock entries
and Tauri configuration use 0.3.2. Dependency versions remain unchanged; the pnpm lockfile
does not encode the local package versions and requires no content change.

Environment: Windows x64, Node 24.21.0, Rust/Cargo 1.99.0, locked dependencies.

| Command/check | Result |
| --- | --- |
| `npm run verify` | Exit 0: Markdown/assets, 13 script tests, 22 UI tests, clean Svelte/fmt/Clippy, 1,057 Rust tests; eight platform tests ignored; frontend build |
| `npm run test:browser` | Exit 0: Edge interactions, ten locales and themes |
| Native source/old application DB | Exit 0: consistent independent backup; all 493 Codex files and one Kilo database active, no compatible files; all 10,888 prior events retain token fields, occurrence times and first observations; zero conflicts |
| Native repeat | Exit 0: zero additions, updates or conflicts; usage unchanged after complete reads |
| `npm run build:desktop` | Exit 0: Windows x64 0.3.2 executable and `LLMUsage_0.3.2_x64-setup.exe` |
| `npm run test:headless` | Exit 0: 11 real-executable/SQLite checks; isolated synthetic sources, three events/75 tokens |
| `npm run check:docs` | Exit 0: 205 repository translation pairs, 21 guide pairs, source-comment review; no Astro errors/warnings |
| `npm run test:docs` | Exit 0: 39 tests, including the new validation page's navigation entry |
| `npm run build:docs` | Exit 0: 1,383 pages and 2,897 files; local links, search, language roots and publication markers |
| pnpm frozen lock check | Exit 0 with installed pnpm 11.13.0: `pnpm with current install --lockfile-only --frozen-lockfile --ignore-scripts --offline`; original lock content unchanged |

The project still pins pnpm 12.10.1. Its launcher could not fetch that executable from
the configured registry; the offline check above used the installed version and does not
verify a fresh install with the pinned launcher. Bootstrap-added lock metadata was removed.
No dependencies were upgraded. At the time of these local checks, Windows installer
lifecycle, native GUI/IPC, remote CI and release publication had not been rerun;
browser and headless checks are reported separately.

The first regression run reused an ingestion job identifier; the next run assumed an
unchanged JSONL scan would always produce an ingestion result. The test harness now gives
each scan a separate identifier and accepts a skipped unchanged scan while asserting
unchanged data and revisions. A capability assertion and unknown-version synthetic inputs
were updated to reflect the new exact registry. These were verification setup issues.
Real-source checks use independent consistent application-database backups under
`build/compat-032/`; no original Agent file or application database is modified.
Before the creation-fingerprint fix, those reads repeatedly hit the same 32 MiB window
and the complete-read assertion failed. After the failing regression and fix, a fresh
backup completed in five collection rounds and the next round was idempotent. The initial
documentation check also rejected an unfenced Chinese runtime quotation in the English
record; it is now a literal code example. The navigation count was updated from 224 to
225 with an explicit assertion for the new validation page.

## Tagged CI and Release

The `v0.3.2` tag points to `12207ea1a6c710c67ef681a6adc44b3041dc9128`.
[CI run 38156835286](https://github.com/owent/llm-usage/actions/runs/38156835286)
completed successfully: Markdown, frontend, three native Rust jobs, six portable builds
and Draft Release, for 12 successful jobs. The release job waited for those builds;
it completed at 01:13 on 2026-10-12, Asia/Shanghai.

Draft Release ID `409806776` contains six portable archives, the Windows x64 NSIS installer
and six platform reports: 13 assets. The release job verified their sizes and SHA-256
digests after upload and checked the tagged source revision. Its `draft: true` setting
keeps it out of the public Release list until it is published. Draft creation and public
publication are separate results; the tagged workflow has succeeded.

During the subsequent checks, this same Release was published at 01:16 on 2026-10-12,
Asia/Shanghai. The current API state is `draft: false`, with the same tagged source and
13 assets, at [v0.3.2](https://github.com/owent/llm-usage/releases/tag/v0.3.2).
Six downloaded platform reports match their API digests, the tagged revision and version;
all seven package API sizes/digests match those reports. The initial follow-up probe
expected a draft and rejected the changed state; a fresh API read established publication
before the public-state check was rerun. No release metadata was changed by these probes.
The [Release-triggered documentation job](https://github.com/owent/llm-usage/actions/runs/38158980685)
and its [main-branch refresh](https://github.com/owent/llm-usage/actions/runs/38158986481)
both completed successfully on the same source revision. This does not verify a real
installed-client upgrade.

## Actions version references

The subsequent maintenance changes both workflows to upstream numeric version tags,
following the user's 2026-10-12 instruction. Latest stable upstream releases and tag
contents were checked through the official GitHub API:

| Action | Workflow reference | Verified stable release |
| --- | --- | --- |
| [actions/checkout](https://github.com/actions/checkout/releases) | `v7` | `v7.0.1` |
| [actions/setup-node](https://github.com/actions/setup-node/releases) | `v7` | `v7.1.0` |
| [actions/upload-artifact](https://github.com/actions/upload-artifact/releases) | `v7` | `v7.0.2` |
| [actions/download-artifact](https://github.com/actions/download-artifact/releases) | `v8` | `v8.0.2` |
| [actions/github-script](https://github.com/actions/github-script/releases) | `v9` | `v9.0.0` |
| [pnpm/action-setup](https://github.com/pnpm/action-setup/releases) | `v6.1.0` | `v6.1.0` |
| [dtolnay/rust-toolchain](https://github.com/dtolnay/rust-toolchain/releases) | `v1` | `v1` |
| [Swatinem/rust-cache](https://github.com/Swatinem/rust-cache/releases) | `v2` | `v2.9.2` |
| [xresloader/upload-to-github-release](https://github.com/xresloader/upload-to-github-release/releases) | `v1.6.2` | `v1.6.2` |

The pnpm `v6` tag trails `v6.1.0`; the upload Action has no `v1` tag, so both use the
latest full numeric version. The Rust Action's `v1` commit has the same files as the
previously referenced revision. Input names, toolchain/target settings and the existing
tag-only draft publication rules were checked. Root instructions, the operating guidance,
coverage records and English/Chinese platform/release documents use the same convention.
These edits follow the tagged build; run 38156835286 verifies its own source revision.

Follow-up local checks on Windows/Node 24.21.0: all 26 Action references match nine latest
stable upstream releases, and all supplied inputs exist in their published manifests.
The public Release/report audit, `npm run lint:md`, `npm run check:docs` and
`npm run test:docs` exited 0; 39 documentation tests passed. The generated scripts, API
snapshots and downloaded reports are under ignored root `build/actions-tags-032/`.
`npm run build:docs` also exited 0: 1,381 pages and 2,893 published files, with local
links, language roots, search and publication markers checked.
