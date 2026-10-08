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

At 61223e5, .github/workflows/ci.yml defined eight jobs: documents/frontend and Rust fmt/Clippy/test
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

## Draft Release publication

On 2026-10-08, the user authorized tag-triggered draft publication, replacement of release
metadata/same-name assets and deletion/recreation of v0.2.1. The old annotated tag pointed
to bda55d3b49959e9390c5d5c48fff91c3574b8724. The recreated tag and tested workflow/source
point to a16ae4e74a7b3a9f2365b1991500f12555b66407.

Local Windows x64 checks used Node 24.21.0, npm 12.0.2 and PowerShell 7. All commands exited
0: node --test desktop/scripts/*.test.mjs (5 tests), the temporary workflow extraction/check
script (10 synthetic release cases without network calls), npm run lint:md (451 files),
npm run check:docs (199 repository pairs, 21 guide pairs and 32 Astro files, no diagnostics),
npm run test:docs (31 tests) and git diff --check. These local simulations are separate from
the GitHub publication below. The preceding main CI and local VS inventory test failed
because an empty Measure-Object result has no Sum property in strict mode; the fix guards
empty file arrays and the isolated test asserts zero file count/bytes without warnings.

[Tag run 37749146960](https://github.com/owent/llm-usage/actions/runs/37749146960) passed all
nine jobs on its first attempt, including three platform builds and draft publication.
[Main CI](https://github.com/owent/llm-usage/actions/runs/37749146944) and the
[documentation workflow](https://github.com/owent/llm-usage/actions/runs/37749147002) also passed.
The draft has release ID 406615500, name/tag v0.2.1, the tested target commit and seven assets:
four packages and three platform reports. The release job checked package revisions,
sizes and digests before upload and GitHub asset sizes/digests after upload. Three reports
were independently downloaded and matched the four published packages' API sizes/SHA-256.

| Package | Bytes | SHA-256 |
| --- | ---: | --- |
| Windows x64 NSIS | 4,046,322 | 5c51b575d14cafe8744ab08915eb701445ec7a2dca8ebaba7169644b0788e6f3 |
| Linux x64 deb | 5,631,732 | 6dfe3750cb9ea1781db706b1f67366586f82a783d3ad0bebdff0cf82f9188d9c |
| Linux x64 AppImage | 84,097,528 | 9508a3e65dcf8217e9750e89df9c4ab9db8a15096298f8ef4897d09f691f0b9e |
| macOS arm64 .app.tar.gz | 4,740,489 | 5b01cfee42fcf95f367a949b723c0393cc4ec0f996ef4e4f60155df9ff3af9ca |

The first overwrite-test marker PATCH omitted tag_name. GitHub changed the original draft's
tag to its generated untagged name, so attempt 2 correctly found no v0.2.1 draft and created
extra draft 406616857. Its exact source/assets were checked before deleting only that
test-owned draft. The original draft was restored with explicit tag_name and marker title/body;
readback confirmed v0.2.1 before rerunning the release job. Attempt 3 passed: the original
release ID was retained, title/body were restored, all seven asset IDs changed, and every
size/digest stayed equal to the first upload. Only one v0.2.1 draft remains. The workflow's
own create/update calls always include tag_name.

Network EOFs affected status queries, not CI jobs. A read-only jobs query recovered;
completed logs were captured under ignored build/github-release/ with ANSI removed before
display. Local npm 12 initially rejected locked mirror tarballs; command-local
--allow-remote=all with installation scripts disabled succeeded without changing locks or
global configuration. Documentation/comment hashes were reviewed and synchronized.
Publication remains a draft and does not establish signing, notarization or new GUI/install
acceptance. Later record-only commits leave the tested source/workflow unchanged.

## Portable release v0.2.2

On 2026-10-08, the user authorized Windows/Linux/macOS x64 and arm64 portable archives,
no Debian package, increased zstd compression and publication through a new v0.2.2 tag.
Root/desktop npm manifests and locks, Tauri configuration, both workspace Cargo manifests
and their own Cargo.lock entries now agree on 0.2.2. The annotated tag points to
39e74d09fa2b4e300dca3f7d0ba2edf4b6e95ba2; dependency versions remain locked.

The six builds use native windows-2022/windows-11-arm, ubuntu-22.04/ubuntu-22.04-arm and
macos-15-intel/macos-15 runners for x64/arm64 respectively. Each archive contains a
versioned directory with its executable, complete resources and bilingual extraction/runtime
instructions. Windows uses LLMUsage.exe; Linux includes extracted AppDir with AppRun and
requires no FUSE mount; macOS includes the complete LLMUsage.app. Windows 11 uses system
WebView2, Linux retains the Ubuntu 22.04/glibc 2.35 build baseline and compatible desktop,
and macOS retains compatible system WebKit/Gatekeeper requirements. Data remains in the
existing per-user directory; moving the program requires disabling owned scheduled tasks first.

Packaging materializes tar before zstd -19 -T2 --long=27, runs zstd -t and verifies a fresh
extraction against all file hashes, symbolic links and Unix modes. Executable headers must
match the native matrix architecture. All six extracted executables pass actual headless
SQLite/collection checks; both Linux archives additionally pass native Xvfb/WebKitWebDriver
GUI/IPC collection and five-page navigation, with isolated source/data environments.
Linux reports verify 370 tree entries; macOS verifies four and Windows two.

Local Windows x64 used PowerShell 7, Node 24.21.0, npm 12.0.2, Rust 1.98.0, locked Tauri CLI
2.12.0 and zstd 1.5.7. Commands exited 0: node --test desktop/scripts/*.test.mjs (13 tests),
the extracted workflow checker (14 synthetic create/update/rejection cases), npm run lint:md
(451 files), npm run check:docs (199 repository pairs, 21 guide pairs, 32 Astro files without
diagnostics), npm run test:docs (31 tests), npm --prefix desktop run check, Cargo fmt and
git diff --check. The direct desktop Tauri CLI built the actual Windows 0.2.2 executable/NSIS;
version resources read back as 0.2.2. Its portable archive passed fresh-extraction/headless
checks and native-smoke.mjs passed 19 actual WebView2/IPC/UI checks with three startup samples.
The same 10,283,520-byte tar compressed to 4,843,925 bytes at zstd level 3 and 4,113,788 bytes
at level 19, a 15.1% reduction without resource trimming. Fixture tests and workflow simulations
are separate from native execution and real publication.

First main run [37757778777](https://github.com/owent/llm-usage/actions/runs/37757778777)
at 9d14217a5ce36c74dda90f61507e076225826c66 compiled both Linux targets but failed fresh
extraction mode comparison: the default tar extraction umask removed group-write bits.
The fix extracts with -p on Unix and adds 775/664 fixture coverage. At the tag's tested source,
[main run 37759068722](https://github.com/owent/llm-usage/actions/runs/37759068722) passed
all 11 required jobs (documents/frontend, three Rust and six builds); the tag-only publication
job was skipped. Linux GUI and native credential checks passed. The tag was created only
after this complete successful main run.

[Tag run 37761919575](https://github.com/owent/llm-usage/actions/runs/37761919575)
finished successfully after recovering the macOS x64 artifact-upload failure; all 12 logical
jobs have successful results at the same source. Draft [v0.2.2](https://github.com/owent/llm-usage/releases/tag/untagged-564360182cf0e15d7e8a)
has release ID 406718916, exact tag/name v0.2.2, that target commit and 13 assets:
six portable .tar.zst archives, the retained Windows x64 NSIS installer and six platform reports.
There is no deb, raw AppImage or gzip archive. The publication job hashes actual packages
before upload and compares all uploaded GitHub sizes/digests. Six reports were independently
downloaded, hashed and checked against the seven published packages' API sizes/SHA-256.

A controlled title/body marker was applied with explicit tag_name and read back. Rerunning
only the release job retained the same single draft ID, restored title/body/target metadata
and replaced all 13 asset IDs while preserving every file size and digest. The final release
attempt is 3; the independently downloaded reports were verified again after replacement.

| Package | Bytes | SHA-256 |
| --- | ---: | --- |
| LLMUsage-0.2.2-windows-x64-portable.tar.zst | 4,117,621 | 0eca2969168447cb2c1e755b2c89626d4f7a44734778241bbd788e7c01c385d1 |
| LLMUsage_0.2.2_x64-setup.exe | 4,046,000 | fdbcf85d1ed6ff73de55336669d5f427d8430fb2c72e847c94f8b9961c37b91a |
| LLMUsage-0.2.2-windows-arm64-portable.tar.zst | 3,951,219 | ad91aa491272f2c13ac0f4b90489c3929f736d152a4ab584d8f30f1fc7f590d3 |
| LLMUsage-0.2.2-linux-x64-portable.tar.zst | 72,899,343 | 67f2f64b5e054b6471d5daf907b7b1bc6d916985cf3e0b4233ac7d467c06b703 |
| LLMUsage-0.2.2-linux-arm64-portable.tar.zst | 71,499,124 | 9151080f8e2fb18207b8823564e3c870b1da777b065d1032d1554316ba1e4eab |
| LLMUsage-0.2.2-macos-x64-portable.tar.zst | 4,228,412 | 4d19e09a3f354a6b416b0345e0262c2d0690d96dbab1e274704914363d5ba1b9 |
| LLMUsage-0.2.2-macos-arm64-portable.tar.zst | 3,965,860 | de180b5e1dc24b76ca3b8451d5bf9de533e7e2f633ade0bd731b993f5f82f114 |

The first local root npm script rejected forwarded --bundles arguments under npm 12;
the direct desktop Tauri CLI succeeded. Old 0.2.1/current 0.2.2 NSIS outputs initially made
selection ambiguous; selection now requires the exact current-version filename and leaves
old outputs intact. A mistaken log path was corrected to repository-root build/ before
execution. The Windows sandbox launcher failed with CreateProcessAsUserW error 5; the approved
execution path ran the unchanged checks. The first tag attempt's macOS x64 archive and
headless checks passed, but GitHub CreateArtifact failed with DNS ENOTFOUND; the completed
failed job was rerun after the whole attempt ended, without changing the tested source.

Native Windows x64 GUI, Linux x64/arm64 CI GUI and six native headless results apply to
their actual environments. They do not establish Windows arm64/macOS GUI, a new installer
lifecycle, other Linux distributions or signing/notarization. The Release remains a draft.
Task logs, fixtures and raw reports stay under ignored root build/portable-release-022/;
the existing native smoke stores isolated evidence under root build/plan-completion/native/.
Later acceptance-record commits leave source, workflow and lockfiles identical to the tag.
