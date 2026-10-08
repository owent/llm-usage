# Three-platform CI acceptance for this batch

<a id="本批三平台-ci-验收"></a>

Date: 2026-10-06. The user explicitly authorized a separate test-branch commit/push and
three-platform CI acceptance. main retains its existing working-tree changes; test worktree,
logs and downloads are under root build/ci-plan-validation/. Authorization covered this test
branch/CI; merging main, publishing, signing or notarizing require their own authorization.

<a id="受测范围"></a>

## Tested scope

Baseline: c2c8f6f1c44f3dc9e398842d9dca2c93da76680c. Changes include real-local-sample rule
corrections, old processing-position/complete-summary upgrades, source routing/credentials,
Windows/Linux lifecycle and assistive-technology tools, synchronized tests/redacted samples,
Plan, AI rules and documents. Raw databases/WAL/conversation bodies/client settings/temporary
models remain in ignored build/ and outside commits.

.github/workflows/ci.yml defines eight jobs: documents/frontend and Rust fmt/Clippy/test
plus release builds on windows-2022, ubuntu-22.04 and macos-15. Linux Rust additionally
checks isolated D-Bus/temporary keyring; Windows release checks isolated headless collection.
61223e591815a4369a85a00fa23ff1ab2819d6d5 adds two existing explicit macOS credential tests:
cross-process round trip and HTTP revoke. They create random owned entries only, send tokens
to children through stdin, clean exact entries and disable synchronization/authentication UI.
That commit reruns the whole matrix. Test-branch pushes do not trigger CI: dispatch the
workflow for that branch, then verify branch and full head_sha. A passing main baseline
cannot establish this batch's result.

<a id="执行结果"></a>

## Results

Test branch: [codex/plan-validation-20261006](https://github.com/owent/llm-usage/tree/codex/plan-validation-20261006).
First tested commit: 819ca6eda6a6f3e88bc2e62e08f2b8ea15c0c766, 223 files. Before commit,
203 Markdown files, 269 local references and git diff --check passed. Scanning 68 new
sample files/171 JSON objects found no credential fields or unredacted private paths.
[Run 37485522153](https://github.com/owent/llm-usage/actions/runs/37485522153), workflow_dispatch,
matched the branch/full head_sha on readback and all eight jobs passed. Windows default
Rust tests: 1,007 passed, eight ignored; all platform release artifacts generated.
[Run 37486581699](https://github.com/owent/llm-usage/actions/runs/37486581699) for the macOS
addition also passed all eight. Counts below come from job logs, separating default tests
and explicitly run native credential tests without duplicate counting.

| Job | Result |
| --- | --- |
| Documents | 203 Markdown files, no issues |
| Frontend | No type errors/warnings; 4 script tests, 21 UI tests, Playwright Chromium checks, resources/build pass |
| Windows Rust | fmt/Clippy pass; 1,007 default tests pass, 0 fail, 8 ignored |
| Linux Rust | fmt/Clippy pass; 1,004 default tests pass, 0 fail, 9 ignored; 6 separate D-Bus/keyring native checks pass |
| macOS Rust | fmt/Clippy pass; 1,003 default tests pass, 0 fail, 6 ignored; 2 additional Keychain cross-process/real-HTTP-revoke checks pass |
| Windows release | Release/NSIS, 11 real headless regressions, report/upload pass |
| Linux release | Release/deb/AppImage/report/upload pass |
| macOS release | Release/.app archive/report/upload pass |

macOS desktop and specific hardware were outside this batch.

<a id="下载制品"></a>

## Downloaded artifacts

All three GitHub archives downloaded: Windows 3,969,947 bytes; macOS 4,660,995; Linux
88,869,759. Local SHA-256 matched API digest; ZIP CRC passed. Extraction held only four
packages below and three bundle-report.json files. Each report revision is
61223e591815a4369a85a00fa23ff1ab2819d6d5; package sizes/recomputed SHA-256 match reports.

| Package | Bytes | SHA-256 |
| --- | ---: | --- |
| Windows x64 NSIS | 3,984,826 | 8bf368b7ae1a0e494e6851ccbb7e65be2ab1fa1cdc6a1c0a1a309c92f69feb83 |
| Linux x64 deb | 5,542,774 | 759ce028ffab4e1359b2ef661b7b4d8cccebaffc23adbdf7539d32511cafbec4 |
| Linux x64 AppImage | 84,007,416 | 5729705271795aab9023979007b87bd6b18205a5c1620e90fd18e6e83f116780 |
| macOS arm64 .app.tar.gz | 4,661,581 | 95def48f9a4399d9f3e23dd89fff6712a6b1c9cd6874ede00a0a9027a078393e |

These are CI outputs without release signing/notarization/GitHub Release publication.
Download integrity is separate from existing local GUI/install acceptance and does not
establish installation or macOS desktop success.

<a id="首次失败与恢复"></a>

## First failures and recovery

First deeply nested worktree creation hit Windows Git path-length limits, exit 128.
Readback found no worktree, only a test branch at baseline. Command-local core.longpaths=true
allowed copying/staging/commit/push; main/HEAD remained unchanged, without global Git changes.
GitHub CLI refused completed-job logs while the run was unfinished, so the official
[job logs endpoint](https://docs.github.com/en/rest/actions/workflow-jobs#download-job-logs-for-a-workflow-run)
was read instead. ANSI protection was relaxed only for file-captured calls; control sequences
filtered before display. A running Windows-job endpoint returned 404; this was not reported
as product failure and tests were not rerun.

CLI artifact download timed out after 300 s; no extracted result was accepted. Official
archive endpoints then streamed each archive under build/, with credentials/signed URLs
in memory only and byte progress recorded. Windows/macOS took 36/38 s; Linux took 502 s,
beyond the original limit. Completed archives were checked against API/local digests, CRC
and package reports. A temporary extraction script joined the result path incorrectly
after two archives were extracted; result writing was fixed and existing files rechecked
against ZIP members without overwrite/deletion. First Linux final-log download hit EOF;
diagnostic retained and one read-only retry succeeded without rerunning product tests.

<a id="记录同步与边界"></a>

## Record updates and scope

Later commits update only Plan/acceptance documents/Skill references. Source, locks and
workflow must match the tested revision; Markdown/local links/diff checks run separately.
This record uses 61223e5 as remote tested revision; record commits are not another CI run.
Windows/Debian native/container results are in [current acceptance](current-acceptance.md);
remaining real versions/protocols/host-desktop conditions stay in [Plan](../../../Plan.md).
