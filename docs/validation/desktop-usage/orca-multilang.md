# Debian Orca navigation acceptance in ten languages

<a id="debian-orca-十语言导航验收"></a>

Date: 2026-10-06. Ten-language testing first passed with the
[Cline-stage artifact](cline-container-sample.md); the current
[OpenClaw-stage artifact](openclaw-container-sample.md) was rebuilt and passed a full rerun.
Requirements: [languages](../../design/desktop-usage/i18n.md) and
[installation lifecycle](../../design/desktop-usage/installation-lifecycle.md).

Environment: WSL Debian 13.7, rootless Podman 5.4.2; image ID
4575f4ba7437ce291ce8e1633292f232ad5899d1551c06ed20aced61f3fc212d.
Orca 48.1, pyatspi 2.46.1, Speech Dispatcher/espeak-ng 0.12.0, separate D-Bus and ALSA
null. Networking disabled, no host mounts, default seccomp. Ordinary GUI user UID=1000,
CapEff=0; only the FUSE container explicitly adds SYS_ADMIN.

The complete command used `npm run test:install:linux -- --screen-reader --appimage-mode fuse`
with the established old/new deb, current AppImage and fixed acceptance-image arguments;
exit 0. Actual install/upgrade/rollback/uninstall/reinstall, GTK/WebKit IPC and read-only
FUSE mount/release remain nine groups, 47 checks. The current successful WSL root is
build/install-lifecycle/linux/1791289583346; previous 1791287694583 retains the Cline-package
scope. Saved summaries and raw Orca output are in root build/plan-final-push/
orca-multilang-proof.json, orca-result.json, orca-debug.log and linux-final-proof.json.

All ten languages (zh-CN, zh-TW, en, ja, ko, es, fr, de, pt-BR, ru) were saved through
product settings, then checked in persisted values and HTML lang. Native Tab reached
the five navigation buttons per language: 50 names. Each check used only AT-SPI focus
logs and Orca speech output after that keystroke, followed by native Enter and the actual
page title. The original English brand-name check was retained. No product-name injection
or reuse of another language's speech output; English was restored afterward.

First run 1791287290747 incorrectly treated navigation index 3 as settings and could
not find the language form; actual App.svelte order corrected it to 4. Second run
1791287460071 produced real focus/speech for Chinese `总览`, but the assertion expected
the English verbose-log format. Using actual `FOCUS MANAGER: Locus of focus is` allowed
the full rerun to pass. Both first-failure logs remain; they are not product accessibility failures.

This verifies ten-language navigation names reaching focus and the speech pipeline.
It does not verify pronunciation, physical audibility, complete operation of other page
controls, Windows Narrator/NVDA, or a complete host desktop.
