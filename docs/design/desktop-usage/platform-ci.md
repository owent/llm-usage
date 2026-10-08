# Platforms, GitHub CI and WSL validation

<a id="平台github-ci-与-wsl-验证"></a>

Status: on 2026-10-06, the authorized separate test branch was committed and pushed.
Two rounds of three-platform Rust/release and shared checks each passed all eight jobs;
two explicit native macOS credential tests passed. Downloads of three archives/four packages
were checked against their digests and source revision; see [this batch's CI](../../validation/desktop-usage/ci-plan-validation.md).
Windows 11 x64 is the user-confirmed first desktop target, with macOS/Linux GitHub CI retained.
This file preserves acceptance requirements; validation records own actual results.

The user authorized this plan: fix runner/toolchain/Linux baselines in M0 and probe WSL/WSLg
availability in M0. The current release scope includes portable x64 and arm64 packages for
Windows, Linux and macOS. These are
implementation checks rather than prerequisites requiring another startup approval.
The 2026-10-05 scope adjustment removed macOS desktop and specific hardware requirements,
retaining macOS builds and Rust CI. Actual Linux GUI/installation lifecycle may use isolated
Podman inside WSL/Debian. Record distribution, image digest, user, display and sandbox conditions;
do not expand the result to host login or complete desktop support. Native Windows install,
upgrade, rollback and uninstall are authorized; see [installation lifecycle](installation-lifecycle.md).

<a id="平台与制品矩阵"></a>

## Platform and artifact matrix

| Platform | First-release role | Initial CI target | Delivery and validation results |
| --- | --- | --- | --- |
| Windows x64 | First desktop target on Windows 11 | windows-2022; x86_64-pc-windows-msvc | Portable exe archive and existing NSIS installer; native IPC/install/task acceptance remains separate |
| Windows arm64 | Required portable distribution | windows-11-arm; aarch64-pc-windows-msvc | Native release exe archive and extracted executable headless tests |
| Linux x64 | Required portable distribution | ubuntu-22.04; x86_64-unknown-linux-gnu | Extracted AppDir archive, native headless tests; no Debian package in the Release |
| Linux arm64 | Required portable distribution | ubuntu-22.04-arm; aarch64-unknown-linux-gnu | Native AppImage build, extracted AppDir archive and native headless tests |
| macOS x64 | Required portable distribution | macos-15-intel; x86_64-apple-darwin | App bundle archive and extracted executable headless tests |
| macOS arm64 | Required portable distribution | macos-15; aarch64-apple-darwin | App bundle archive, native headless tests and existing Rust/credential checks |
| Local WSL 2 Linux x64 | Development and authorized container acceptance | Actual distribution/toolchain recorded | Builds, tests and packaging recorded separately; isolated Podman GTK/WebKit GUI and package lifecycle checked within this round's scope |

Use versioned runner labels and explicit Rust targets; verify runner and binary architectures.
Floating latest labels cannot silently change architecture. All six combinations are required
for publication; one missing archive prevents the release job from updating a draft.
A GitHub Windows runner build does not establish success in a Windows 11 user's environment.
Archive availability and native headless tests do not establish complete desktop support.
Keep failures visible; do not hide them indefinitely with continue-on-error.

Sources: [GitHub runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners),
[Tauri multiplatform pipeline](https://v2.tauri.app/distribute/pipelines/github/) and
[platform prerequisites](https://v2.tauri.app/start/prerequisites/). Establish the minimum Linux
runtime against actual linked dependencies in M0; the CI image does not establish support
for every Linux environment.

<a id="github-ci-合同"></a>

<a id="github-ci-要求"></a>

<a id="github-application-ci-contract"></a>

## GitHub application CI requirements

Three-platform jobs exist; ongoing M7 acceptance needs actual runs. Triggers are PRs,
main-branch pushes, tag pushes and manual dispatch. The user authorized automatic draft
Release publication, deletion/recreation of v0.2.1 to validate it, and version 0.2.2 portable
publication through v0.2.2. Other remote writes
still need authorization.

1. Shared checks: Markdown/local links, frontend types/unit tests and Rust fmt/clippy;
   determine commands from actual lockfiles.
2. OS matrix: run domain/adapter test data, real temporary SQLite, scheduler, path and lock
   tests on each native runner, then build the same source's frontend/Rust release.
   External Agents need no installation; do not read a runner's personal directories.
3. Artifacts: six portable archives and the Windows x64 installer with source revision,
   OS/architecture, dependency versions, size and digests. Separate package/test failures;
   cargo check cannot replace a complete build.
4. Desktop integration: actual WebView2 CDP on Windows; GTK/WebKit WebDriver with Tauri's
   automation environment switch on Linux. Release packages do not add a persistent listener.
   Native macOS desktop is outside this round.
5. Separate release resource jobs measure package size/performance. Hosted-runner timing
   is a regression signal rather than a substitute for a fixed machine's P95.
6. Build dependency downloads may use the network; statistical runtime/acceptance cannot
   contact usage/billing services. Test data is synthetic/redacted; no personal databases
   or original conversations are uploaded. Offline cases cover telemetry switches.

Checks include cargo fmt --all --check and cargo clippy --workspace --all-targets --locked
-- -D warnings across the core crate and test targets. Frontend jobs run TypeScript logic
tests and Playwright browser regressions with synthetic IPC, covering themes, timezones,
quick filters, user isolation and refresh. This is separate from item 4's native acceptance.
All six builds run the extracted executable's test:headless with isolated synthetic sources
and source-category assertions. Rust fmt/Clippy/test run on all three native runners, compiling
their platform storage implementations.

Linux also runs test:credentials:linux with isolated D-Bus and a disposable encrypted keyring;
credentials reach test children only through stdin. macOS Rust jobs explicitly exercise
system credential cross-process round trips and real loopback HTTP revocation, creating and
precisely cleaning random owned items without synchronization or authentication UI.
Explicit native Windows credential tests run locally; ignored tests do not establish system
storage acceptance. Local test:desktop checks actual IPC through WebView2 CDP without adding
a release listener. See [current acceptance](../../validation/desktop-usage/current-acceptance.md)
for local results and remote CI gaps.

Use fail-fast=false to retain all matrix results and timeouts per job. Retry only diagnosed
temporary infrastructure failures. Cache keys include OS, architecture, Rust/Node versions
and lockfile digests; separate targets and never cache real Agent data.
Application Actions use fixed commit SHAs. PR permissions remain read-only. Only the tag-push release job receives
contents: write through GitHub's short-lived token, after all shared checks and platform
builds succeed. Privileged pull_request_target cannot execute PR code.
CI artifacts and GitHub Releases are separate. Tag pushes create or update one draft by
tag_name, including its name, body and target commit. Publication is serialized per tag;
the job checks the current tag commit before updating a draft, so an older build cannot
overwrite a recreated tag. Draft lookup covers all release pages. The
[upload Action](https://github.com/xresloader/upload-to-github-release) receives the exact
release ID and overwrite: true to replace same-name assets. Repeated runs replace the
metadata and assets of the same draft. Platform reports have distinct asset names;
package revisions, sizes and SHA-256 digests are checked before publication and uploaded
assets are checked afterward. This workflow does not establish signing/notarization or
desktop installation acceptance. Upload only the six `.tar.zst` archives, the existing Windows
x64 NSIS installer and distinct size/digest reports. Debian packages and raw AppImages are not uploaded.

## Portable archive requirements

Names include version, OS and architecture: `LLMUsage-<version>-<windows|linux|macos>-<x64|arm64>-portable.tar.zst`.
Each archive has one equally named directory, containing the Windows exe, Linux AppDir with
`AppRun`, or macOS `.app`, plus English/Chinese usage instructions.
Portable means extraction without installing the application. Existing per-user data and
system credential storage remain in use; moving the archive does not migrate those stores.
Retain complete bundled resources, executable modes and symlinks. Do not trim locales.

Windows uses the system WebView2 Runtime; Windows 11 normally supplies it. These archives do
not embed a fixed offline browser runtime. macOS uses system WebKit and is not notarized by
this pipeline. Linux builds on Ubuntu 22.04 for both architectures and archives the extracted
AppImage dependencies, avoiding FUSE and a second compressed container. It still requires a
compatible glibc (2.35 or later), desktop display and system facilities such as D-Bus/Secret
Service for optional credential integration; other distributions require separate validation.
The archive instructions identify these requirements without claiming a universal runtime.

The packaging script writes a materialized tar, compresses with `zstd -19 -T2 --long=27`,
checks `zstd -t`, decompresses and extracts into a fresh verification directory, then compares
all file digests, symlink targets and Unix modes. It verifies the extracted PE/ELF/Mach-O CPU
type against the expected architecture and runs real headless/SQLite tests from that extraction.
Both Linux architectures additionally launch the extracted AppRun through native WebKitWebDriver
under Xvfb and isolated D-Bus, checking the window, five pages, real IPC and synthetic SQLite results.
Only validated archives receive reports and enter publication. A failed build/compression,
wrong architecture, corrupt archive or incomplete platform report blocks publication.
Repeated packaging replaces the same archive after validation; repeated publication retains
the draft identity and replaces same-name assets. Roll back by fixing the source and rerunning
the authorized tag workflow; never reuse an older build after its tag has moved.

Sources: [Tauri ARM AppImages](https://v2.tauri.app/distribute/appimage/#appimages-for-arm-based-devices),
[AppImage extraction](https://docs.appimage.org/user-guide/run-appimages.html#extract-the-contents-of-an-appimage)
and [Tauri WebView2 requirements](https://v2.tauri.app/distribute/windows-installer/#webview2-installation-options).

Desktop source: [Tauri WebDriver](https://v2.tauri.app/develop/tests/webdriver/).
This round's Linux tests use native WebKitWebDriver capabilities directly to launch actual
packaged applications, matching tauri-driver's WebKit parameter mapping. Separate native macOS
build/credential tests from desktop acceptance. Basic builds do not require paid platform services.

<a id="文档-ci-与-pages"></a>

## Documentation CI and Pages

The explicitly authorized documentation publication has a separate workflow under
.github/workflows/docs.yml and [documentation requirements](../documentation-site.md).
It uses verified pinned Action revisions, Node 24, paired-content/comment checks, Astro checks,
unit tests, production output validation and browser regressions before uploading a static
artifact. Documentation changes trigger it automatically; source changes also check comment
references. Pull requests have read-only permissions and cannot publish.

Successful main-branch builds publish the compiled root to gh-pages without force-pushing,
verify the source/output digests and explicitly request/observe the configured Pages build.
Creating Pages initially requires an administrator; ordinary publication uses contents/pages
permissions. Branch publication, observed Pages build, custom-domain DNS and HTTPS are separate
results. The configured domain is llm-usage.atframe.work; its DNS CNAME must point to owent.github.io.
An authored workflow or local production build does not establish remote CI/deployment success.

<a id="本地-wsl-执行顺序"></a>

## Local WSL execution order

Attempt only under implementation authorization and record every step.

1. Probe installed distributions, WSL version, CPU architecture, free space and WSLg.
   Record missing prerequisites. Permission to attempt builds does not authorize installing,
   upgrading or restarting the entire machine/distribution.
2. Create this task's separate working copy on a Linux filesystem, recording the Windows
   source revision and uncommitted-diff digest. Cloning only HEAD must not omit pending changes.
   Do not copy Windows node_modules/target or personal Agent data.
3. Prepare the locked Linux Rust/Node toolchain and GTK/WebKitGTK 4.1, SSL, tray and packaging
   dependencies. Keep Windows' default toolchain intact and do not reuse cross-system build
   caches. Verify exact installation commands at execution time.
4. Run logic/test-data checks, release builds and Linux packaging sequentially, recording commands,
   exit codes, artifacts and dependencies. Separate AppImage FUSE/packaging failures.
   Producing an executable does not establish package acceptance.
5. Within authorized scope, use isolated Podman, an ordinary user, Xvfb, window manager and
   D-Bus; install actual packages and verify GTK/WebKit windows, IPC, pages and isolated data.
   test:install:linux is the reusable entry. Without actual display/GUI results, retain only
   compilation/test conclusions; installing GUI dependencies does not establish GUI success.
6. Stop/clean only task-owned processes/directories. Avoid global wsl --shutdown that affects
   other work. Temporary application databases use a local Linux filesystem and cannot share
   an active database with Windows.

WSLg source: [Microsoft Linux GUI guidance](https://learn.microsoft.com/en-us/windows/wsl/tutorials/gui-apps).
It requires WSL 2 and does not provide a complete Linux desktop. This round's container
deb/AppImage and actual GUI acceptance follows the user's scope; host tray, notifications,
login/logout and startup remain separate. WSL builds and collection of real WSL Agent usage
are recorded independently. Read-only local-data validation is authorized under the readiness
field restrictions; build tests do not implicitly start collection.

See V26/V27. Documentation checks cannot replace unexecuted CI, WSL or desktop tests.
