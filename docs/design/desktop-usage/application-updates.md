# Software updates

## Settings and notifications

Update settings are separate from collection, price refresh and system tasks. Automatic
checks default to daily; users can choose every application launch, daily, weekly or
manual only. Daily and weekly mean elapsed 24 hours and seven days since the last
successful check, persisted across launches. Failed automatic requests have a one-hour
retry delay. GUI startup and a minute timer check deadlines; headless collection never
checks or installs updates. Manual checks bypass deadlines. Checks and downloads share
one operation slot.

Automatic downloads default off. A discovered release prompts Download; with automatic
downloads enabled the application downloads and verifies the matching package, then
prompts Install update or Restart and update. Installation always needs an explicit
click. A global notice and Settings > Software updates show version, package type, downloaded
bytes, total bytes, progress, cancellation and errors. Switching pages preserves progress.
Successful checks display their time. Settings changes take effect after Save.

The settings section is Software updates in English, `软件更新` in Simplified Chinese and
`軟體更新` in Traditional Chinese. Other locales follow their own software interface usage.
Dismiss notification hides the current notice until its phase, version, package or error
changes. Update controls remain available in Settings; dismissal does not schedule a reminder.

## Release and package identity

The source is the public HTTPS GitHub Releases API for `owent/llm-usage`. Use the latest
published stable release, strict semantic version comparison and exact asset names.
Never downgrade, cross architectures or switch installation types. A Windows NSIS
installation selects `LLMUsage_<version>_x64-setup.exe`; a portable installation selects
`LLMUsage-<version>-<platform>-<arch>-portable.tar.zst`.

New portable archives contain `llmusage-package.json` with schema, version, platform,
architecture, executable and managed top-level entries. Its native executable must resolve
to the running executable. Windows installed identity is checked against the NSIS registry
InstallLocation, MainBinaryName and DisplayVersion, using the actual executable path. Unidentified raw
binaries and unsupported installed package formats cannot automatically select an asset.
Development builds therefore require a packaged copy to exercise installation.

Requests send an application User-Agent and public release identifiers only, without usage,
local paths, credentials or account identifiers. Production endpoints and redirects are
restricted to GitHub HTTPS release infrastructure. Read bounded metadata and stream bounded
downloads to disk. Verify exact size and the GitHub API asset SHA-256 before extraction,
again before applying, and when reusing a completed download after restart. Missing digests,
duplicate assets, mismatched URLs, versions or sizes fail closed. A `.part` file is never
installable; interrupted downloads restart from byte zero.

This design trusts HTTPS and the repository's GitHub release ownership. SHA-256 detects
corruption and asset changes; it is not an independent publisher signature. Existing CI
already verifies GitHub's uploaded asset digests. Tauri's standard updater additionally
requires publisher keys but does not support the current Windows portable tar.zst packages;
adopting that protocol would require another package format and a managed signing key.

## Applying updates

NSIS updates use the existing `/UPDATE /P` mode and the original installation directory;
the installer handles privileges and must register the requested version after exiting.
It preserves data and owned task consent.
Portable updates stage on the target filesystem, retain a backup and replace managed
package entries at their existing paths. Unknown files at the package root are preserved;
a newly introduced managed entry must not overwrite an existing unrelated file. Application
data and system credentials remain in their existing per-user locations.

Portable extraction rejects traversal, absolute paths, duplicate members, hard links,
special files and escaping symbolic links, and bounds expanded bytes and entry counts.
Internal Unix symlinks are created after regular members; final resolution rejects dangling,
cyclic or escaping links. Package metadata and native
architecture are verified before a restart can be offered.

An auxiliary instance enters update mode before database initialization, receiver or GUI
startup. On Windows it runs a copy of the current executable outside managed entries;
on Unix it can keep the running inode while replacing package files. It acquires a target
lock and signals readiness before the parent exits, waits for the parent with a deadline,
then applies the prepared plan. A journal records original entry presence before any
rename. Replacement failures restore the original entries. Interrupted journals are
recovered under the same target lock. The last backup and result remain available; a new
process launch alone does not prove that the updated GUI or database started successfully.
Database migrations keep their existing consistent-backup rules. File rollback does not
silently restore or discard a user's database.

Existing regular files use Windows `ReplaceFileW` with a same-volume backup, or a Unix
backup link and rename. Directory changes still require journal recovery between renames.
If an interrupted Windows replacement leaves the main executable missing, open the retained
`.llmusage-update/helper.exe` to restore the original files and launch the application.

Installation is refused while collection or database maintenance runs. Restart retains
the current data directory; headless/background integration keeps the same executable
path. Disk-space, permission, antivirus locking, helper launch, timeout, hash, extraction,
installer and relaunch failures are shown explicitly, including saved helper failures after
relaunch. Cancellation is available during check/download/preparation, before any files are
installed; a blocked network read returns within its bounded request timeout.

## Verification

Regress defaults and old settings, persisted deadlines and clock rollback, manual checks,
concurrent operations, stable version ordering, both architectures, exact installer/portable
selection, unknown identity, malformed metadata, redirects, missing digests, truncation,
corruption, cancellation, cache reuse, malicious archives, collisions, replacement rollback
and interrupted journals. Browser checks cover settings, global progress and explicit
installation. Native Windows tests use isolated packaged copies and independent synthetic
databases; record helper file replacement and actual NSIS lifecycle separately. Linux/macOS
CI verifies portable layout and platform code; local Windows results do not establish their
desktop update acceptance. A public newer-version upgrade requires a subsequently published
release and is recorded separately from injected release metadata.

## Source references

Retrieved on 2026-10-08:

- [Tauri updater](https://v2.tauri.app/plugin/updater/): supported artifacts, mandatory
  signatures, separate download/install and Windows installer modes.
- [GitHub releases](https://docs.github.com/en/rest/releases/releases?apiVersion=2026-03-10):
  latest stable release and unauthenticated public access.
- [GitHub release assets](https://docs.github.com/en/rest/releases/assets?apiVersion=2026-03-10):
  asset names, uploaded state, size, digest and download URL.
- [Windows file replacement](https://learn.microsoft.com/en-us/windows/win32/fileio/moving-and-replacing-files):
  replacing files and keeping originals.
- [ReplaceFileW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-replacefilew):
  same-volume replacement/backup, preserved attributes and partial-failure states.
- Repository `desktop/scripts/portable.mjs`, NSIS template pinned to tauri-cli-v2.12.0,
  and `.github/workflows/ci.yml`; live v0.2.2 release metadata verified all seven package
  sizes and SHA-256 digests.
