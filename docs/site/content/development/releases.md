---
title: Packaging, CI and releases
description: Preserve revision records and separate builds from signed distribution.
sidebar:
  order: 5
---

The existing application CI has Markdown, frontend, Rust and platform release-build jobs.
Its platform matrix keeps Windows, Linux and macOS, with `fail-fast: false`. A package's
bundle report records revision, size and SHA-256; inspect the actual artifact rather than
assuming a green job produced the package you intended.

## Build locally

```powershell
npm run pack:desktop
```

This checks assets and frontend types before the Tauri release build. It does not sign,
notarize, publish a Release or establish installation/native-provider acceptance.
Package outputs are under `desktop/src-tauri/target/release/bundle/` according to platform.

Keep root/desktop npm manifests and locks, Tauri configuration, Rust manifests and Cargo.lock
consistent when changing a version. The portable script rejects mismatches and tags other than
`v<version>`. Restore locks and actual LFS assets. Read the
[platform requirements](/reference/design/platform-ci/) and
[installation requirements](/reference/design/installation-lifecycle/) for native requirements.

## Publication boundaries

Pushing a tag triggers the application CI and publishes a draft Release after every check
and platform build succeeds. The draft contains six `.tar.zst` portable archives for Windows,
Linux and macOS in both x64 and arm64, the existing Windows x64 NSIS installer, and one
size/SHA-256 report for each OS/architecture. Debian packages and raw AppImages are not uploaded.
Package reports must match the tagged source revision. The release job verifies uploaded
sizes and digests, using
[xresloader/upload-to-github-release](https://github.com/xresloader/upload-to-github-release)
at a fixed commit with `overwrite: true`.

Recreating a tag or rerunning its workflow updates the same draft's name, body and target
commit, replacing same-name assets. Publication is serialized per tag, and an older build
is rejected when the tag has moved. The Release remains a draft; review it before making
it public. These packages do not establish signing, notarization or installation acceptance.

## Portable packages

Choose `LLMUsage-<version>-<windows|linux|macos>-<x64|arm64>-portable.tar.zst` for your system.
Extract the complete directory using a zstd-capable tool (current 7-Zip on Windows,
or `tar --zstd -xf <archive>` where supported). Run `LLMUsage.exe`, `AppRun`, or `LLMUsage.app`
from the extracted Windows, Linux, or macOS directory respectively. Keep bundled resources
together. Each archive includes English/Chinese runtime instructions.

New archives also include `llmusage-package.json`, identifying the version, platform,
architecture, native executable, launch entry and managed top-level files. The updater
verifies this identity before selecting the same portable format. Keep the marker with
the package; an old archive without it needs a manual replacement to adopt this updater.

Windows requires system WebView2, normally present in Windows 11; these packages do not embed
a fixed offline runtime. Linux includes AppDir libraries without requiring FUSE, uses an
Ubuntu 22.04/glibc 2.35 baseline, and still needs a compatible desktop/display. Optional
credential integration uses system Secret Service/D-Bus. macOS uses system WebKit and downloaded
apps remain subject to Gatekeeper. Other Linux distributions and desktop environments need
separate validation. Data and credentials retain their existing per-user locations; moving
the directory does not move them. Disable application-owned scheduled tasks before moving it.

The packager materializes tar then uses `zstd -19 -T2 --long=27`, preserving all resources.
It validates compression, a fresh extraction's digests/links/modes and binary CPU type, and
runs the actual extracted application's isolated headless/SQLite regression on all six native
runners. Only then are archive sizes and SHA-256 recorded for publication.

The application checks the public GitHub latest stable release and verifies the matching
asset's API size and SHA-256. Preserve exact portable and NSIS names, publish all matching
assets before making a release public, and retain the API digests. The updater trusts HTTPS
and repository release ownership; these digests are not publisher signatures. It ignores
drafts, prereleases and same/older versions. Development builds and unidentified executables
can check versions but cannot select, download or install a package. See the
[update design](/reference/design/application-updates/) and
[validation record](/reference/evidence/application-updates/). Run `npm run test:update:windows`
after building to check actual portable IPC/helper replacement using a synthetic update;
this does not establish a public release upgrade or NSIS lifecycle acceptance.

GitHub metadata request failures try the documentation site's `/updates/latest.json` once.
The snapshot preserves exact package URLs, sizes and SHA-256 digests; downloads still use
GitHub. Clients reject snapshots older than seven days or more than five minutes in the future.
Documentation CI refreshes it during main builds and daily at 04:17 UTC. Release publication,
editing, unpublication or deletion dispatches a main-branch documentation build, preserving
the Pages environment's branch restriction. PR builds use the committed snapshot offline.
The generator requires all seven uploaded packages and never replaces the original timestamp
when reusing a valid snapshot. Refresh locally with
`node docs/site/scripts/update-feed.mjs`; the result stays under ignored root `build/`.
Run `npm run test:update:check` after building for an isolated Windows IPC version check,
or run `node desktop/tests/native-update-check.mjs --exe desktop/src-tauri/target/debug/LLMUsage.exe --development`
for an embedded debug build. See the [check repair record](/reference/evidence/update-check-repair/).

Authorization to build and to publish is separate. A design plan is not authorization to push,
deploy, sign or change external credentials. Complete a reviewable artifact, tests,
identity checks and rollback plan before a required final approval.

The current documentation publication has explicit user authorization and a dedicated
workflow. It publishes the static site to `gh-pages` and completes Pages deployment;
application draft publication has its own explicit authorization and tag-only write permissions.
PR checks remain read-only. See [documentation maintenance](/development/documentation/).

Record the tested full revision, actual jobs and artifact hashes. A later documentation-only
commit can cite an earlier source build only when its code/workflow scope is clearly stated;
it must not claim that CI ran on a different revision.
