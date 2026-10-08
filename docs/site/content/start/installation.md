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

In **Settings > Software updates**, choose every launch, daily, weekly or manual checks.
Daily/weekly checks use elapsed time since the last successful check and run while the app
is open. Automatic downloads default off; manual checks are always available. Save changes
to apply them. The global notice and settings page show download progress, errors and cancellation.
After a verified download, click **Install update** for NSIS or **Restart and update** for portable.

Updates keep the OS, architecture and package type. Windows installed copies receive an NSIS
installer; portable copies receive the matching portable archive. Portable updates retain the
existing executable path and a backup under `.llmusage-update/backup`, preserving unrelated
files at the package root. Keep personal files outside package-managed subdirectories.
New portable archives contain `llmusage-package.json`; keep the complete package together.
Raw executables without a verified package identity cannot choose an automatic update.
If collection/maintenance is running, finish it and retry installation. Replacement failures restore the
original files; an interrupted portable update is recovered on the next GUI launch.
If the main Windows executable is missing after interruption, open the retained
`.llmusage-update/helper.exe` to recover and restart.

Checks contact the public GitHub release service without sending local usage. Downloads
require an exact size and SHA-256 match. This verifies the HTTPS GitHub release asset;
it does not provide an independent publisher signature. Native update results and platform
limits are recorded in [update validation](/reference/evidence/application-updates/).

Keep a consistent database backup before an upgrade or rollback. SQLite migrations and
their backups are implemented by the backend; an older executable may not support a newer
schema. Keep a backup compatible with the version you plan to run.

Windows uninstall preserves usage and settings by default and removes startup entries and
scheduled tasks created for that installation. Upgrading preserves enabled background
settings and updates the executable path. Uninstall does not restore unrelated
IDE configuration or remove another installation's data.

Portable packages retain the same per-user data and system credentials; moving their directory
does not migrate data. Stop the app and disable its owned scheduled tasks before moving/removing
the directory. For a manual replacement, extract the new complete package to a new directory,
retain your database backup and re-enable tasks with the new path when needed. The application's
portable update keeps the existing path.
