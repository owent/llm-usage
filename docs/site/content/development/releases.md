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

Keep version fields in the desktop package, Tauri configuration and Rust manifests
consistent when changing a version. Restore locks and actual LFS assets. Read the
[platform requirements](/reference/design/platform-ci/) and
[installation requirements](/reference/design/installation-lifecycle/) for native requirements.

## Publication boundaries

Pushing a tag triggers the application CI and publishes a draft Release after every check
and platform build succeeds. The draft contains the Windows x64 NSIS installer, Linux x64
Debian/AppImage packages, macOS arm64 `.app.tar.gz` and a size/SHA-256 report per platform.
Package reports must match the tagged source revision. The release job verifies uploaded
sizes and digests, using
[xresloader/upload-to-github-release](https://github.com/xresloader/upload-to-github-release)
at a fixed commit with `overwrite: true`.

Recreating a tag or rerunning its workflow updates the same draft's name, body and target
commit, replacing same-name assets. Publication is serialized per tag, and an older build
is rejected when the tag has moved. The Release remains a draft; review it before making
it public. These packages do not establish signing, notarization or installation acceptance.

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
