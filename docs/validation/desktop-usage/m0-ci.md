# M0: Establish the CI matrix for three platforms

<a id="m0三平台-ci-矩阵建立"></a>

Later Linux Rust dependencies, macOS app archives and artifact-report tests are recorded in
[M0/M1 review](m0-m1-review.md). GitHub execution was initially awaiting registration.

<a id="元信息"></a>

## Run information

| Item | Value |
| --- | --- |
| Date | 2026-09-24 |
| Environment | Written locally; GitHub Actions had not run (no push in this stage) |
| Code revision | Uncommitted working-tree changes |
| Design reference | platform-ci.md, GitHub CI requirements |

<a id="实际建立的内容"></a>

## Configuration added

.github/workflows/ci.yml runs on pull requests, main pushes and manual dispatch, with
`permissions: contents: read`.

| Job | Runner | Steps |
| --- | --- | --- |
| docs | ubuntu-22.04 | Root npm ci and npm run lint:md |
| frontend | ubuntu-22.04 | Desktop npm ci, npm run check and npm run build |
| rust | ubuntu-22.04 | cargo fmt --check, cargo clippy --locked -D warnings, cargo test --locked |
| build | windows-2022 / ubuntu-22.04 / macos-15; fail-fast=false | Frontend and npx tauri build; NSIS/deb+AppImage/.app; sizes and SHA-256; uploaded artifacts retained 14 days |

- Actions use complete commit SHAs: checkout d23441a4… (v6), setup-node 24997072… (v6),
  upload-artifact b7c566a7… (v6), dtolnay/rust-toolchain 6bed0761… (then-current stable),
  swatinem/rust-cache 6323deb1… (v2). GitHub API resolved each SHA to a commit.
- Node 24 and Rust 1.98.0 are fixed. Cache keys include OS, architecture, toolchain and lock files.
- Linux jobs install Tauri dependencies including WebKitGTK 4.1; commands match desktop locks.
- No publishing, signing or automatic-update steps; CI artifacts are separate from GitHub Releases.

<a id="未执行项"></a>

## Initially unexecuted work

| Item | Status | Reason |
| --- | --- | --- |
| Actual three-platform jobs | **Executed and recorded on 2026-09-30, below** | — |
| Test-data job | Placeholder | M0 had no parser tests; M1 adds core cases to cargo test |
| Desktop WebDriver integration | Not configured | M6 scope; assess the macOS embedded path then |
| Release resource/package regressions | Not configured | M7 scope |

<a id="三平台首次登记运行2026-09-30"></a>

## First registered three-platform runs (2026-09-30)

| Run | Trigger/HEAD | Result | Inspection |
| --- | --- | --- | --- |
| [36444880100](https://github.com/owent/llm-usage/actions/runs/36444880100) | Push titled `当前进度暂时完成`, 7831f89, 2026-09-28 | **success**, 6/6 jobs: three release builds, Markdown, Rust fmt/clippy/test and frontend checks/build; 5m18s | gh run view, 2026-09-30 |
| [36707037687](https://github.com/owent/llm-usage/actions/runs/36707037687) | F2 cost-engine push, 118c442, 2026-09-30 | **failure**: Linux kilo busy_writer exposed an existing race: Busy backup retries counted as pages, reaching the 20.5s page limit before the 30s timeout and falsely reporting the space cap. Windows reached timeout first and passed. Fixed in 36709419197 | gh run view --log-failed and WSL reproduction |
| [36709419197](https://github.com/owent/llm-usage/actions/runs/36709419197) | Fix Busy retries being counted as pages, cb28454 | **success**, 6/6; Windows/WSL reruns agree | gh run list/view |
| [36709463931](https://github.com/owent/llm-usage/actions/runs/36709463931) | Remove accidentally committed debug worktree, 00b9643 | **success**, 6/6 | gh run list/view |

Historical note: failed runs 36256404241 and 36378875396 on September 26/28 preceded
36444880100 and were intermediate repair states, not passing baselines. The 36707037687
failure exposed the cross-platform test race introduced by 438261d: backup-limit comments
and code differed, and changing 2s to 30s changed which exit happened first. cb28454 fixed
the race and all jobs passed.
