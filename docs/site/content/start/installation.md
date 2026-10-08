---
title: Installation and updates
description: Platform requirements, packages, data locations and safe updates.
sidebar:
  order: 2
---

## Platform and packages

Windows 11 x64 is the first desktop target. The Windows package is an NSIS installer
and uses the system WebView2 runtime. Linux builds produce deb and AppImage packages;
macOS builds produce an app bundle. A successful cross-platform build does not by itself
verify installation, desktop integration or GUI behavior on that platform.

The repository currently has no published GitHub Release entries. Use the packages in
the [GitHub Actions build artifacts](https://github.com/owent/llm-usage/actions), checking
the tested revision and bundle report, or [build the application](/development/setup/).
Do not interpret an unsigned pre-alpha artifact as a signed production release.

For the platform-specific results, read the [installation record](/reference/evidence/installation-lifecycle/)
and [current acceptance](/reference/evidence/current-acceptance/). Linux container GUI
acceptance and Windows native acceptance have separate scopes; WSL compilation is a
different result.

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

For developers testing packages, use the dedicated isolated installation lifecycle scripts.
Their prerequisites and cleanup boundaries are documented in the
[installation requirements](/reference/design/installation-lifecycle/).
