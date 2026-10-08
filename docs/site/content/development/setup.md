---
title: Development setup
description: Install the verified toolchain and run the desktop application or documentation.
sidebar:
  order: 1
---

## Prerequisites and dependency ownership

Use Node.js 22 or newer for repository tooling; the Astro site specifically requires
Node 22.12 or newer. CI selects Node 24 and Rust 1.98.0. The Rust manifest declares an
MSRV of 1.89; use the tested toolchain when reproducing current acceptance.
Documentation comment checks also need Python 3 and PowerShell 7: they parse Python and
PowerShell source without executing it. Install them through your usual development
environment if absent; `check:docs` does not install tools automatically.

Root `package.json` and `package-lock.json` own documentation/tooling dependencies and
the unified command entry. `desktop/package.json`, its lockfile and the Cargo manifests/
lockfile own the application. Restore the locks rather than inferring exact versions from
caret ranges. Git LFS stores images, fonts and binary assets; a pointer is not a usable asset.

```powershell
git lfs install --local
git lfs pull
npm ci
npm --prefix desktop ci
npm run assets:check
```

Linux Tauri builds require the packages listed in `.github/workflows/ci.yml`, including
WebKitGTK 4.1, GTK, OpenSSL, appindicator and build tools. macOS requires the corresponding
native build environment. Platform compilation, a WSL build and a native GUI test remain
separate claims.

## Run the application

```powershell
npm run dev:desktop
```

This starts the development GUI with a debug backend and Vite hot reload; it does not
package a release. `npm run dev:web` starts only the frontend at `127.0.0.1:1421`, without
the Tauri backend. Use the browser mock harness for browser-only interaction checks.

```powershell
npm run verify
npm run build:desktop
npm run test:headless
```

`verify` checks Markdown, assets, scripts, UI logic, Svelte types, Rust formatting/clippy/
tests and the frontend build. Headless/native scripts require a built executable and use
isolated sources; `--data-dir` alone is insufficient to isolate source discovery.

## Run the documentation

```powershell
npm run dev:docs
npm run check:docs
npm run build:docs
npm run test:docs
npm run test:docs:browser
```

Documentation output and generated reference content live in root `build/documentation-site/`.
Read the [documentation maintenance guide](/development/documentation/) before changing
language pairs or publication inputs. Source comments use English; their Chinese reference
is maintained with the corresponding code.
