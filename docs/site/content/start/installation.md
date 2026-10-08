---
title: Installation and updates
description: Platform requirements, packages, data locations and safe updates.
sidebar:
  order: 2
---

## Platform and packages

Windows 11 x64 is the first desktop target. Releases provide `.tar.zst` portable archives
for Windows, Linux and macOS in x64 and arm64, plus the Windows x64 NSIS installer.
Windows uses system WebView2; Linux archives contain an extracted AppDir and macOS archives
contain an app bundle. Debian packages are not included. A successful cross-platform build does not by itself
verify installation, desktop integration or GUI behavior on that platform.

For the platform-specific results, read the [installation record](/reference/evidence/installation-lifecycle/)
and [current acceptance](/reference/evidence/current-acceptance/). Linux container GUI
acceptance and Windows native acceptance have separate scopes; WSL compilation is a
different result.

## Downloads

Tag pushes prepare [GitHub Release drafts](https://github.com/owent/llm-usage/releases).
Drafts are visible to repository collaborators until a maintainer publishes them.
You can also download packages from the [application build workflow](https://github.com/owent/llm-usage/actions/workflows/ci.yml):
open a successful run, check its revision and bundle report, and download the artifact for
your platform. GitHub requires sign-in to download workflow artifacts.
You can also [build the application](/development/setup/).
Unsigned pre-alpha artifacts do not establish a signed production release.

For portable packages, choose the matching OS and CPU architecture, then completely extract
`LLMUsage-<version>-<os>-<arch>-portable.tar.zst` using current 7-Zip or a zstd-capable tar.
Open `LLMUsage.exe` on Windows, run `./AppRun` on Linux, or open `LLMUsage.app` on macOS.
Linux does not require FUSE, but needs a compatible desktop/display and glibc 2.35 or later
(Ubuntu 22.04 build baseline). Windows 11 normally includes WebView2; it is not embedded in
the portable package. macOS downloaded apps remain subject to Gatekeeper approval.
Read the included English/Chinese `README.txt` for system conditions.

## Application data

The backend currently uses `%APPDATA%\llm-usage-desktop\llm-usage.sqlite` when `APPDATA`
is available. Otherwise it uses `~/.local/share/llm-usage-desktop/llm-usage.sqlite` when a
home directory is available. The source of truth is the application's `db_path()` routine.

To select a separate application data directory, launch:

```powershell
LLMUsage.exe --data-dir C:\UsageData
```

The directory must be absolute. This changes application storage, not agent discovery.
For a deliberately restricted collection scope, enable **Only scan manually added
directories** and configure those roots in Settings.

## Upgrade, rollback and removal

Keep a consistent database backup before an upgrade or rollback. SQLite migrations and
their backups are implemented by the backend; an older executable may not support a newer
schema. Do not delete your database to make a rollback appear successful.

Windows uninstall preserves usage and settings by default and removes only startup/task
definitions proven to belong to that installation. Upgrading preserves previously approved
background behavior and verifies the executable path. Uninstall does not restore unrelated
IDE configuration or remove another installation's data.

Portable packages retain the same per-user data and system credentials; moving their directory
does not migrate data. Stop the app and disable its owned scheduled tasks before moving/removing
the directory. Extract an update to a new directory, retain your database backup and re-enable
tasks with the new path when needed.

For developers testing packages, use the dedicated isolated installation lifecycle scripts.
Their prerequisites and cleanup boundaries are documented in the
[installation requirements](/reference/design/installation-lifecycle/).
