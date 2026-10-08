# Installation lifecycle acceptance

<a id="安装生命周期验收"></a>

On 2026-10-05, the user authorized local Windows and WSL/Debian Podman installation
acceptance, without requiring macOS desktop or specified hardware. Follow
[platform requirements](platform-ci.md) and [scheduling rules](scheduling.md).
Commands and actual results belong in validation records.

<a id="环境与安装包"></a>

## Environments and packages

- Windows uses actual current-user NSIS packages. First inspect existing installations,
  processes, product registry keys and shortcuts read-only. With no existing installation
  in this batch, use isolated installation/data/synthetic-source directories under root
  build/. Compare existing old and current packages; record SHA-256, installed version
  and actual executable path.
- Linux uses task-specific rootless Podman storage, a fixed official Debian image digest,
  isolated users/directories/display/D-Bus. Install real deb packages and verify GUI/IPC
  through GTK/WebKit WebDriver. Record upgrades/downgrades, uninstall/reinstall and
  AppImage extraction/FUSE separately; retain data and confirm repeat scans add nothing.
  Include CJK fonts; check representative Chinese glyph coverage before GUI testing and
  inspect actual screenshots. DOM text does not establish readability; missing glyphs
  cannot pass complete-display acceptance.
- Installing software/containers creates no usage by itself. Distinguish nonempty product
  files, real model-returned tokens, official anonymized samples and synthetic usage from
  simulated interfaces. Neither images nor samples may contain account keys.

<a id="行为要求"></a>

## Required behavior

After installation, check registration, shortcuts, actual executable, IPC and isolated
SQLite. Upgrades retain user data, source configuration and consented system tasks, then
read back task executable paths. Check database compatibility before rollback. Record
refused rollback or consistent-backup restoration honestly; never delete the database to
make a test pass. Uninstall retains usage/configuration by default and removes only this
installation's owned tasks/startup items. Matching name prefixes cannot justify deleting
another installation, data directory or mismatched definition.

Windows cleanup must run before GUI initialization, database migration or collection.
Verify tasks against the current executable and name/arguments/identity; recheck failures
and prevent uninstall from deleting the executable. Temporary uninstall during upgrades
keeps tasks. Uninstall does not automatically restore IDE configuration or delete other
data. Task presence, exit 0 or an unqueryable post-delete task cannot replace full-definition
checks. Current ownership requirements: owned hashed name, description, single Exec,
absolute data-dir argument, current-user SID, interactive logon and least privilege.
Resolve COM account names to SID; capture complete XML and recheck before deletion.
Open only existing databases referenced by tasks, using the existing single-writer lock
to disable background intent/pending requests. Create/migrate no database; retain other
settings, usage and schema. Verify task absence after deletion. Reinstallation does not
automatically restore background consent.

The NSIS template is pinned to official tauri-cli-v2.12.0 and retains its MIT license.
The only body change replaces the default Run deletion without path verification with
this installation's ownership-checking cleanup command. Tool upgrades require reviewing
template differences. PREUNINSTALL calls --uninstall-cleanup only for standalone
uninstall and aborts on failure before executable deletion.

Login/logout and process exit are distinct; never log out host Windows/WSL users
automatically. Stop only this task's processes/containers. Do not stop WSL globally,
delete personal images or overwrite existing installations. New scripts require a side-effect-free --help.

<a id="可复用入口"></a>

## Reusable commands

Run from the repository root after building and preparing actual old/new packages.
--help installs nothing and creates no containers.

```powershell
npm run test:install:windows -- --previous-installer <previous-NSIS-path> --previous-version 0.2.0
```

Windows defaults to the current release/bundle/nsis package; --installer can override it.
Require no existing LLMUsage installation, process or startup item. Snapshot/preserve
existing shortcuts. NSIS controller timeout=180 seconds. Tests create current-user
tasks/startup items and remove only exact owned entries. Isolated installation, data
and results are under build/install-lifecycle/windows/. System tasks must persist
manual_roots_only=true and synthetic manual roots; parent process environment is insufficient.

```sh
npm run test:install:linux -- --previous-deb <previous-deb-path> --deb <current-deb-path> \
  --appimage <current-AppImage-path> --appimage-mode extract
```

Linux requires Node 22+ and rootless Podman and builds a fixed Debian GUI image by
default. Building may use networking; runtime containers have no networking/host mounts,
and files enter through cp. --image/--skip-build-image can reuse a task image with a
recorded digest. --storage-dir accepts only a task-specific directory under root build/
and empty registry authentication. Build timeout=30 minutes, control command=5 minutes,
GUI request≤40 seconds. Collect results on success/failure, then stop/remove this run's
container; leave images/cache in its dedicated storage. Output: build/install-lifecycle/linux/.

--gui-scale 1|2 sets GTK window scale and matching screen dimensions on isolated X11.
Verify WebKit devicePixelRatio and actual screenshots, checking horizontal layout on
each page. Record this separately from CSS zoom, host display settings and screen readers;
leave the production default environment unchanged. --appimage-mode fuse explicitly maps
/dev/fuse and adds SYS_ADMIN only to this rootless container, retaining default seccomp.
GUI runs as an ordinary user with effective capabilities=0. The added capability lets
container fusermount mount the image; no privileged mode or disabled WebKit sandbox.
Verify actual FUSE type, read-only mount, owning user and executable inside AppImage;
verify release on exit. A default container lacking this capability does not establish
missing WSL FUSE: check device, kernel, user namespace and container permission separately.
sudo may install missing components, without automatically changing host global configuration.

Current packages and results: [installation acceptance](../../validation/desktop-usage/installation-lifecycle.md).

--screen-reader adds a separate native Orca GUI run after reinstalling the current
package. Image builds install Orca, Speech Dispatcher/espeak-ng, pyatspi and xdotool;
reused images require version readback. Isolated D-Bus/temporary user settings enable
accessibility only inside the container. Use native Tab/Enter, await actual speech and
verify five-page switching in ten languages. Save languages through actual settings;
each keystroke reads only subsequent new AT-SPI focus/speech logs. Save debug logs and
results; see [ten-language acceptance](../../validation/desktop-usage/orca-multilang.md).
The test launcher only enables line buffering for the installed Orca debug file; body
changes require rechecking. Event/speech behavior stays unchanged. ALSA null avoids
audio-hardware dependence, without verifying pronunciation, physical audibility or other
page controls. On exit, stop/wait only owned reader/speech processes with time limits;
the container remains offline.
