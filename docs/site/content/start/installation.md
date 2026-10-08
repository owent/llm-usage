---
title: Installation and updates
description: Platform requirements, packages, data locations and safe updates.
sidebar:
  order: 2
---

## Platform and packages

Releases provide `.tar.zst` portable archives
for Windows, Linux and macOS in x64 and arm64, plus the Windows x64 NSIS installer.
Windows uses system WebView2; Linux archives contain an extracted AppDir and macOS archives
contain an app bundle.

## Downloads

Download the package for your OS and CPU architecture from the
[latest Release](https://github.com/owent/llm-usage/releases/latest).
Windows x64 users can also choose the `*-setup.exe` installer.

For portable packages, choose the matching OS and CPU architecture, then completely extract
`LLMUsage-<version>-<os>-<arch>-portable.tar.zst` using current 7-Zip or a zstd-capable tar.
Open `LLMUsage.exe` on Windows, run `./AppRun` on Linux, or open `LLMUsage.app` on macOS.
Linux does not require FUSE, but needs a compatible desktop/display and glibc 2.35 or later
(Ubuntu 22.04 build baseline). Windows 11 normally includes WebView2; it is not embedded in
the portable package. macOS downloaded apps remain subject to Gatekeeper approval.
Read the included English/Chinese `README.txt` for system conditions.

## Application data

Application data is stored in `%APPDATA%\llm-usage-desktop\llm-usage.sqlite` when `APPDATA`
is available. Otherwise it uses `~/.local/share/llm-usage-desktop/llm-usage.sqlite` when a
home directory is available.

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
schema. Keep a backup compatible with the version you plan to run.

Windows uninstall preserves usage and settings by default and removes startup entries and
scheduled tasks created for that installation. Upgrading preserves enabled background
settings and updates the executable path. Uninstall does not restore unrelated
IDE configuration or remove another installation's data.

Portable packages retain the same per-user data and system credentials; moving their directory
does not migrate data. Stop the app and disable its owned scheduled tasks before moving/removing
the directory. Extract an update to a new directory, retain your database backup and re-enable
tasks with the new path when needed.
