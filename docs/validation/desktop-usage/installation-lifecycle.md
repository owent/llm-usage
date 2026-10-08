# Windows and Debian container installation acceptance

<a id="windows-与-debian-容器安装验收"></a>

Date: 2026-10-05; version 0.2.1; [installation lifecycle requirements](../../design/desktop-usage/installation-lifecycle.md).
Windows cwd was repository root; Linux cwd was this task's independent WSL Debian copy.
The user removed macOS desktop/specific-hardware requirements and authorized native Windows/
Linux Podman installation acceptance. This record preserves the 2026-10-05 artifacts/results.
For 2026-10-06 source corrections/actual Orca checks, see [source upgrades](source-policy-upgrades.md).
Later M8 packages' 12 Windows checks, nine Linux deb/FUSE/GTK/Orca groups/47 checks and exact
digests are in [M8 samples](m8-container-samples.md). Never mix different stages' package hashes.
The same-current-package ten-language Orca/full-lifecycle rerun is in [Orca results](orca-multilang.md).

<a id="环境与制品"></a>

## Environment and artifacts

Windows 11 Pro x64 10.0.26300, Node 24.21.0, Rust 1.98.1, Tauri CLI 2.12.0,
PowerShell 7.6.6, WebView2 154.0.4258.53. Old NSIS was the actual existing 0.2.0 package,
whose source revision is unconfirmed; its version alone cannot identify a commit.
Current package uses 0c5d63c plus worktree changes. Installations under
build/install-lifecycle/windows/ used Unicode/space paths with isolated sources/data.

Linux: WSL 2 Debian 13.7 x86_64, kernel 6.18.40.1, rootless Podman 5.4.2.
Ordinary acceptance user UID 1000, Xvfb/Openbox, independent D-Bus, WebKitWebDriver,
actual GTK/WebKit frontend/IPC. Default seccomp, no privileged containers/host-directory
mounts/disabled WebKit sandbox. Extraction runs added no capability. FUSE runs added
only SYS_ADMIN; GUI processes still had zero effective capabilities. Containers used
--network=none; only package-control operations used mapped container root.
Official Debian base:
docker.io/library/debian@sha256:7792b1f7702a86946cd518db72b6a407302c3e9bc1635634368b878189e8221c.
Final GUI image ID: 8fdeddb5aadaf9e6afcb68434ae4269b669ba23d776830b52adbef2684758154,
built by adding fonts-noto-cjk 1:20240730+repack1-1 to image
311d072b80b15c3a57a9afd8135deed530931aec6e4afac81361f35f6f19793e.
Old Linux packages used original lockfiles from 2e8ceadecbba0b93e10777c51b4c0d01b4b61dd3.
New packages include cross-platform credentials, early CLI dispatch and current Qwen/
OpenCode capability notes. Windows uninstall code is excluded from Linux binaries.
Earlier root build/install-lifecycle/final-windows-metadata.json and
final-linux-metadata.json retain their separate package digests.

| Actual artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| Windows 0.2.0 NSIS | 3,901,720 | 4572792f5a7010fc5dfbdd02fc0e3155a5c64ee38a974328a9a3df3400ffbab5 |
| Windows 0.2.1 NSIS | 3,925,533 | c058671a65c8f084fbcb5af22a5180e5a55b2da31fcc0e66df9dcd8900d5f893 |
| Windows 0.2.1 exe | 9,935,872 | 39aa599ae3330fe3320d6a2d46a1b804c1da0efefc18ff7387e5b41cdc6a5af1 |
| Linux 0.2.0 deb | 4,885,222 | 90a48c2f3d94c5e4ed86e0540a83995b9a9b6439f38018baf2fa685edf32b679 |
| Linux 0.2.1 deb | 5,441,170 | ba4a4b9b0692eab659fc34c380cc8dc1f85588aeaf6dc624d7c4d496aa835b09 |
| Linux 0.2.1 AppImage | 111,122,936 | 7a6e070c9b86d1ba7092c7e5b9bffeac617f0665381d031b71e331eb95a413cb |

<a id="windows-结果与修复"></a>

## Windows results and repairs

Actual standalone uninstall removed the application/startup entry but left its owned
minute task. Add cleanup before GUI/database initialization, verify task ownership using
the complete definition, hold the existing database writer lock and disable saved intent.
Failures stop NSIS from deleting the executable. Actual COM UserId returns an account
name rather than the XML SID; resolve the account natively before comparing SIDs.
Upstream NSIS Run deletion lacks path verification and may delete another value.
Remove only that statement and preserve the upstream license; fixed
[tauri-cli-v2.12.0 template](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.0/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi).

```powershell
npm run build:desktop
npm run test:install:windows -- --previous-installer desktop/src-tauri/target/release/bundle/nsis/LLMUsage_0.2.0_x64-setup.exe --previous-version 0.2.0
```

Both commands exited 0. After packaging final capability notes and the pre-deletion
definition recheck, all 12 checks passed: old installation/two real shortcuts, old
application import, upgrade, native IPC task/startup registration, rollback, another
upgrade, uninstall cleanup/data retention, reinstall with retained data/background off,
CLI rejection of an active writer, actual NSIS aborting on failure while preserving the
application for retry, and another uninstall preserving unrelated startup values/tasks
with similar prefixes. Data stayed at one synthetic event/15 tokens, schema 11.
Final inspection found no product registration/process/shortcut/startup/owned or decoy tasks.

Another complete 12-check run with current capability notes exited 0:
build/install-lifecycle/windows/1791209633008/result.json.
Final packages: build/install-lifecycle/artifacts/fuse-continuation/windows/;
fuse-continuation-windows-metadata.json. An earlier package operation immediately after
process exit returned 2 with an unconfirmed cause. The controller added a short post-exit
wait and the full round trip passed. That wait is not a verified product-defect repair.

<a id="linux-结果与边界"></a>

## Linux results and limits

```sh
npm run test:install:linux -- \
  --previous-deb build/install-lifecycle/LLMUsage_0.2.0_amd64.deb \
  --deb build/install-lifecycle/LLMUsage_0.2.1_amd64.deb \
  --appimage build/install-lifecycle/LLMUsage_0.2.1_amd64.AppImage \
  --image localhost/llm-usage-lifecycle-cjk-verified:20261005 --skip-build-image
```

Exit 0, eight groups: five actual deb GUI runs (old/upgrade/rollback/upgrade again/reinstall)
and one extracted AppImage GUI. Each checked frontend, isolated discovery, native IPC/
SQLite, five pages and platform state: 30 checks; remove/purge data retention added two,
for 32 assertions. Both versions preserved one synthetic event/15 tokens without failed
migration/duplicates/loss. The actual executable was absent after uninstall. Linux correctly
reported Windows tasks unsupported. Screenshots/results:
build/install-lifecycle/linux/1791202022165/. Owned containers were removed; dedicated
storage had no running containers.

Initial Chinese screenshots showed boxes because the test image contained Latin fonts only.
Passing DOM/IPC did not establish readable text. Add official
[Debian CJK fonts](https://packages.debian.org/trixie/fonts-noto-cjk) to the reusable Containerfile.
Download from a mirror in the [official list](https://www.debian.org/mirror/list), then check the
[official download page](https://packages.debian.org/trixie/all/fonts-noto-cjk/download)
for 56,674,044 bytes and SHA256
f5dc28a754e17327d99f0a612134d92c8dd6187314ae967cb77f25df60860139 before offline installation.
Font coverage correctly rejected the old Latin-only image. All eight groups passed after
adding fonts, with manually reviewed readable Chinese deb/AppImage screenshots.
Precisely cancel slow/retrying CDN downloads and substitute the verified package.
This test-environment repair did not change application packages; sizes/hashes remain above.

Default rootless /dev/fuse mapping failed with
fusermount: mount failed: Operation not permitted.
Same-image/package comparisons confirmed kernel FUSE and device mode 0666.
Device mapping alone failed; adding SYS_ADMIN let UID 1000 --appimage-mount return an
actual read-only FUSE mount while retaining default seccomp. Check capability scope against
[Podman 5.4.2](https://docs.podman.io/en/v5.4.2/markdown/podman-run.1.html#cap-add-capability).
No privileged/rootful Podman/global-host changes; sudo existed but installation did not need it.

Reusable --appimage-mode fuse runs the actual GUI usr/bin/LLMUsage under the verified mount.
Mount type fuse.LLMUsage.AppImage, ro/nosuid/nodev, user_id/group_id=1000.
All four application UIDs were 1000, CapEff=0, Seccomp=2. Read mountinfo after closing
the GUI to verify owned mount release. Initial 34-check results remain in
build/install-lifecycle/linux/1791206172914/. Final scaling/package reruns are below.
The [extraction alternative](https://docs.appimage.org/user-guide/troubleshooting/fuse.html)
retains its earlier separate 32 passes.

Additional [GTK 3 X11 scaling](https://docs.gtk.org/gtk3/x11.html) checks:
--gui-scale 1|2 sets GDK_SCALE=1/2 and GDK_DPI_SCALE=1.
Independent Xvfb DPI=96, displays 1440×1000/2880×2000; actual WebKit devicePixelRatio 1/2.
CSS zoom=1 in both; logical window 1036×780, scale-two screenshots 2072×1560.
Each run had eight groups/40 checks (38 GUI/scaling/mount/release plus two retention),
all exit 0; no document-wide horizontal overflow on five pages, with manual Chinese
screenshot review. Roots:
build/install-lifecycle/linux/1791209706834/ (1) and 1791209741062/ (2).
Windows summaries/screenshots: fuse-continuation-scale-{1,2}-result.json and corresponding
appimage-fuse/upgrade-again.png. Package hashes and zero dedicated-container leftovers:
fuse-continuation-linux-metadata.json. Both new deb/AppImage above underwent both complete
lifecycles; image hash stayed unchanged. Actual container GTK scaling does not verify
host display settings, physical monitors or screen readers.

Debian container package/GUI results do not cover other distributions, the complete host
desktop, login/logout, notifications/tray or native boot startup. macOS desktop/specific
hardware are outside this round's requested scope.

<a id="最终检查"></a>

## Final checks

Windows npm run verify exited 0: Rust 908 (core 810/app 98, eight ignored), frontend
21/scripts four; no type errors/warnings. Markdown 189 files, assets, fmt, Clippy and
frontend build passed. Final Windows test:headless 11 passed; two uninstall-ownership/
future-schema/writer-lock units and final Clippy passed. Final Linux Clippy, Qwen three/
OpenCode eight format tests, deb/AppImage builds and both native source rereads passed,
preserving usage/current capability notes. Both platforms' install-script --help,
JS/Python/PowerShell AST and shell syntax checks passed.

The final change only isolated added format-test databases: independent root-build
directories/nanosecond tags avoid PID reuse. Windows/Debian OpenCode eight tests plus
fmt/Clippy passed again; production code/tested packages stayed unchanged.
Command logs: `build/install-lifecycle/final-*` and `fuse-continuation-*`.
Source/package check results are recorded separately. No commit/push/publication or remote CI trigger.
