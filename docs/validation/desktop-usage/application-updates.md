# Software update validation

Date: 2026-10-08. Version: 0.2.2 with uncommitted updater changes on base
`36d32858f11bbd63b4fdff4f553fd8d04429e064`. This is local
implementation evidence; no commit, tag, release or deployment was created. Requirements:
[software updates](../../design/desktop-usage/application-updates.md).

## Source inspection and decisions

Read the locked Tauri/HTTP dependencies, GUI settings/IPC, SQLite settings and process
guards, portable scripts, platform CI, pinned NSIS template and installation lifecycle.
Read the official Tauri updater, GitHub release/asset API and Windows file-replacement
references linked in the design. A live read of the public v0.2.2 release verified seven
matching package assets with sizes and SHA-256 digests.

The standard Tauri updater cannot consume the existing Windows portable tar.zst format.
The implementation therefore shares strict GitHub release selection and streaming
verification across packages, then applies NSIS or portable updates with a separate helper.
The trust boundary is HTTPS plus repository release ownership, without an independent
publisher signature. Automatic downloads default off; installation always requires a click.

## Environment and commands

Windows 11 x64, Node 24.21.0, Rust 1.98.0, Tauri CLI 2.12.0, system Edge/WebView2 and locked
dependencies. Commands run at the repository root. Logs and independent synthetic
databases are under ignored `build/application-updates/`; browser screenshots/results are
under `build/browser-smoke/`. No real Agent records are needed by these tests.

| Command/check | Result and tested scope |
| --- | --- |
| `cargo test --manifest-path desktop/src-tauri/Cargo.toml --locked --bin LLMUsage update -- --nocapture` | Final exit 0; 12 updater tests. Defaults/old settings, persisted deadlines/clock extremes, strict package/architecture/stable version selection, missing digests, hostile redirects, bounded/truncated streams, cancellation, archive safety, staged metadata/CPU, unmanaged collisions, interrupted swaps, missing originals/duplicate backups, idempotent rollback, changed-target refusal, target lock and saved failures |
| `npm run test:scripts` | Exit 0; 13 checks, including real zstd/tar extraction of markers for six platform/architecture layouts |
| `npm run test:ui` | Exit 0; 22 checks, including update catalog completeness in ten languages |
| `npm run test:browser` | Exit 0; 34 recorded checks and no browser errors. Isolated Edge server with simulated IPC covers settings, manual/automatic downloads, cross-page progress, cancellation including pending preparation, dismissal, package-specific buttons, explicit installation, current/saved errors and narrow layout |
| `npm run verify` | Exit 0; Rust 1,044 passed/8 platform cases ignored, frontend 22 and scripts 13, clean types/fmt/clippy, asset check and frontend build. Subsequent helper-path refinement passed focused 12 tests, clippy and fmt again |
| `npm run build:desktop` | Final exit 0; release executable and Windows x64 NSIS built |
| `npm run test:update:windows` | Final exit 0; three native scenario groups: public check/settings, actual IPC/helper replacement, and missing-executable recovery/relaunch |
| `npm run test:headless` | Exit 0; 11 isolated executable/SQLite checks; final portable packaging repeats them from the real extraction |
| `node desktop/tests/native-smoke.mjs --runs 1` | Exit 0; 19 existing real WebView2/IPC/SQLite/task/window checks and three required launches on the preceding build. The later helper path change affects updater recovery, verified separately on the final executable |
| `node desktop/scripts/portable.mjs desktop/src-tauri/target/release build/application-updates/packages windows x64` | Exit 0; actual portable archive, compression/extraction/native CPU checks, extracted headless checks and copied NSIS installer |
| `npm run check:docs`, `npm run test:docs`, `npm run build:docs`, `git diff --check` | Exit 0; 201 repository pairs/21 guide pairs and reviewed source comments, clean Astro checks, 31 tests, 1,371 pages/2,872 published files with valid local links. Records are synchronized in both languages with reviewed hashes |

Local packages remain under `build/application-updates/packages/`, labeled revision `local`,
not a published release. Portable: 4,199,627 bytes, SHA-256
`40487736aa3d9dc9f5c8a81f6e4ead25073b0592365b8d2eff0efcc2c6ed78b3`.
NSIS: 4,125,060 bytes, SHA-256
`d1ab8cbe6c8e8c31fa4cecb4620abab29c92fab1c4e90496c23938b510c3bf09`.

## Terminology review

The inventory contained 518 messages per locale before this wording revision. Reviewed all
33 update messages and 100 common navigation/settings/collection controls in each of the
ten locales, plus the remaining Simplified Chinese catalog and affected English/Chinese
documentation. Revised 166 existing messages and added a provider-defaults heading in
each locale so the section title and Add action have separate names. Identifiers, package
selection, scheduling and update installation behavior are unchanged.

Official references establish examples of established interface usage, not a universal
ban on other words. Mozilla's current German help article identifies itself as an unchecked
machine translation, so the terminology comparison uses its actual localization source.
The nine non-English about-dialog catalogs were fetched at Firefox localization commit
`d8def708b77353ad76db8f14d62a7b594a3b13b1`; full snapshots and review extracts are under
`build/update-terminology/`. Wording decisions follow the actual LLM Usage controls:

| Locale | Source and resulting wording |
| --- | --- |
| Simplified Chinese | [Apple software updates](https://support.apple.com/zh-cn/108382) and [appearance](https://support.apple.com/zh-cn/guide/mac-help/mchl52e1c2d2/mac) use the requested software-update term and light/dark appearance terms. UI now uses `软件更新`, `浅色` and `深色` |
| Traditional Chinese | [Apple Taiwan](https://support.apple.com/zh-tw/108382) uses `軟體更新`. Installer help uses `安裝程式`, and the action closes a notification |
| English | [Apple](https://support.apple.com/en-us/108382) and [Firefox help](https://support.mozilla.org/en-US/kb/update-firefox-latest-release) support Software updates, Check for updates and restart wording. Version status names LLM Usage explicitly |
| Japanese | [Firefox strings](https://github.com/mozilla-l10n/firefox-l10n/blob/d8def708b77353ad76db8f14d62a7b594a3b13b1/ja/browser/browser/aboutDialog.ftl) use software-update/check/restart vocabulary. The section is `ソフトウェアの更新`; collection uses `収集` consistently |
| Korean | [Firefox strings](https://github.com/mozilla-l10n/firefox-l10n/blob/d8def708b77353ad76db8f14d62a7b594a3b13b1/ko/browser/browser/aboutDialog.ftl) use `업데이트 확인` and identify the software in current-version status. [Apple's Korean App Store guidance](https://developer.apple.com/kr/support/app-store/) also uses `앱 업데이트`; retain that title and clarify launch/confirmation wording |
| Spanish | [Firefox strings](https://github.com/mozilla-l10n/firefox-l10n/blob/d8def708b77353ad76db8f14d62a7b594a3b13b1/es-ES/browser/browser/aboutDialog.ftl) use `Buscar actualizaciones`. Portable/installed labels name the version, completed-download text has an explicit subject, and help follows the existing informal address |
| French | [Firefox strings](https://github.com/mozilla-l10n/firefox-l10n/blob/d8def708b77353ad76db8f14d62a7b594a3b13b1/fr/browser/browser/aboutDialog.ftl) use `Rechercher des mises à jour` and software as the subject of current-version status. Clarify version labels and replace the literal save hint with complete sentences |
| German | [Firefox strings](https://github.com/mozilla-l10n/firefox-l10n/blob/d8def708b77353ad76db8f14d62a7b594a3b13b1/de/browser/browser/aboutDialog.ftl) use `Nach Updates suchen` and a software subject. Use Updates, explicit version labels and the separated verb `Neu starten` |
| Brazilian Portuguese | [Firefox strings](https://github.com/mozilla-l10n/firefox-l10n/blob/d8def708b77353ad76db8f14d62a7b594a3b13b1/pt-BR/browser/browser/aboutDialog.ftl) use `Verificar se há atualizações`. Status names LLM Usage; downloaded/installed labels name the version, and privacy help names usage data |
| Russian | [Firefox strings](https://github.com/mozilla-l10n/firefox-l10n/blob/d8def708b77353ad76db8f14d62a7b594a3b13b1/ru/browser/browser/aboutDialog.ftl) use `Проверить наличие обновлений`. Completed-download status names the version; scheduled daily wording uses a grammatical time phrase |

Source inspection found three additional semantic problems. `UpdateNotice.svelte` only
hides the current status key; there is no reminder timer, so all ten dismiss labels now
describe closing a notification. `commands.rs` writes the current user's Run registry value;
[Microsoft](https://learn.microsoft.com/en-us/windows/win32/setupapi/run-and-runonce-registry-keys)
defines that trigger as user logon, so Chinese/English labels now name sign-in. The scanner
collects enabled due sources rather than only today's records, so all ten interval labels
name automatic collection. Provider defaults select pricing channels and 5/60-minute
prompt-cache pricing assumptions; [Anthropic's cache documentation](https://platform.claude.com/docs/zh-CN/build-with-claude/prompt-caching)
confirms those durations. The lifetime control now has a translated accessible name; it
does not configure an Agent's actual cache. Other repairs name saved settings, global
settings, the start of the week and permanent retention explicitly.

Visual review also found a language-switch defect: the save confirmation was translated
before `onsaved` updated the locale and kept the previous language in every next screenshot.
Save/default-restored feedback now retains its message key and translates at render time.
The browser loop verifies the newly selected language's save confirmation for all ten locales
before capturing each update page.

For this wording revision, `npm run test:ui` passed 22 tests and `npm run check` reported
zero errors/warnings. `npm run build:web` passed. `npm run lint:md` checked 455 files without
issues; `npm run check:docs` passed 201 repository pairs/21 guide pairs and clean Astro
diagnostics; `npm run test:docs` passed 31 tests. `npm run test:browser` passed its 34 recorded groups without browser
errors, including all ten update/settings locale switches at a 760-pixel viewport, unchanged
update actions/progress/cancellation and separate installer/portable buttons. Screenshots
are `build/browser-smoke/updates-<locale>-narrow.png`. These are browser/mock-IPC checks;
they do not establish native-speaker review of every translation.
The native update results and package hashes below/above belong to the preceding feature
build; this wording revision does not rerun NSIS, portable replacement or other-platform GUI
acceptance. The writing Skill, related package help and reviewed bilingual documents are synchronized.

## Native Windows portable result

`npm run test:update:windows` copies the built application into an independent package
root with spaces and Chinese characters. It isolates source environments, TEMP/TMP and
SQLite data, disables collection and sets manual update checks. Real WebView2/IPC verifies
portable identity, the unauthenticated public latest-release check, settings persistence
and the saved last-check time after restart.

The test compiles a native replacement fixture and injects a completed tar.zst download
with matching package identity, size and SHA-256. A real `install_update` call verifies and
stages it; the original GUI exits, the copied helper waits for exit, replaces managed
files at the same path, retains the original backup and launches the new executable.
The fixture writes a restart receipt. Independent checks verify the new executable and
marker, original backup hash, an unrelated root file, SQLite sentinel and saved settings.
The journal ends in `applied`.

A second recovery scenario changes only the owned test journal to `applying`, removes
the synthetic replacement executable and starts the retained helper directly. It restores
the missing original, records `rolled_back`, launches the real application, preserves the
personal file and exposes `update_interrupted_and_restored` through real IPC. The final
report is `build/application-updates/native/1791466696837-62876/report.json`.

This verifies actual update IPC/process/file behavior using a synthetic native executable.
It does not verify a newer published application, that application's GUI/database migration,
or NSIS update execution. The production updater has no IPC override for endpoints or paths;
the test writes only its own local cache before startup.

## First failures and corrections

- Normal sandbox execution returned `CreateProcessAsUserW failed: 5`; automatically reviewed
  command execution restored access. This was infrastructure, not an application test failure.
- Browser testing first encountered an occupied Vite port/shared dependency cache. The harness
  now chooses a free port and independent cache/config under root build. Existing user servers
  and the user's debug application remain running.
- Browser checks found the Updates tab missing the shared Save button; the tab now saves its
  settings. Progress assertions were aligned with the one-second status poll, and notice
  dismissal now distinguishes package names as well as versions/phases.
- A final saved-error change initially failed Rust's mutable/immutable borrow check. The
  initialization now clones package identity before updating status. Clippy's redundant
  closure and a patch-context compile error were also corrected; the final 12 tests pass.
- Markdown checking found a duplicate Chinese title/section; the section now has a distinct title.
- Installation preparation originally disabled Cancel while its IPC call was pending. Cancel
  now has its own pending state and remains available before file installation.
- The documentation sidebar test counted the earlier 219-page source inventory. It now
  requires both update routes and the current 221-page inventory; all 31 tests pass.
- PowerShell's npm wrapper rejected `--runs` with `EUNKNOWNCONFIG`; the real native script
  was executed directly with Node and passed.
- Missing-executable recovery first timed out on a dialog: a directly launched helper used
  a normal Windows path while the journal required the canonical path. Helper input now
  canonicalizes the regular journal before validation; an independent final run passes.
- Review moved check-time persistence outside the update status lock and onto the worker,
  keeping status/cancellation responsive while the database writer is occupied. Automatic
  downloads reserve the available state under one lock. File replacement preserves backups,
  and rollback handles a missing original or an unchanged original beside its backup.

After the final test, a native process inspection found only the preexisting user debug
application; owned update/helper and native-smoke processes had exited. Tests did not stop
the user's running application or alter an existing installation or real source files.

## Acceptance limits

The public latest version equals the running version, so the live request verifies
`up_to_date`, not a production newer-version download. A future public release must retain
the exact names, complete assets and API digests, and needs a separate real upgrade check.
Previously released portable archives without the new marker need one manual replacement
before using this updater.

The NSIS lifecycle preflight found an existing user debug application. Its harness requires
no running application, so this round preserves that process and does not rerun installation,
uninstallation or task changes. Earlier lifecycle results remain historical; the new installer
selection and post-install registry/version checks do not establish real NSIS updater acceptance.

Linux/macOS native update GUI execution, platform-specific permissions and relaunch remain
separate acceptance. Six-layout packaging tests and retained CI definitions do not establish
execution of this uncommitted source on remote runners. No signing credentials were added.
