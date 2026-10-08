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

Authorization to build and to publish is separate. A design plan is not authorization to push,
deploy, sign or change external credentials. Complete a reviewable artifact, tests,
identity checks and rollback plan before a required final approval.

The current documentation publication has explicit user authorization and a dedicated
workflow. It publishes the static site to `gh-pages` and completes Pages deployment;
application release jobs do not inherit that publication authorization or its write permissions.
PR checks remain read-only. See [documentation maintenance](/development/documentation/).

Record the tested full revision, actual jobs and artifact hashes. A later documentation-only
commit can cite an earlier source build only when its code/workflow scope is clearly stated;
it must not claim that CI ran on a different revision.
