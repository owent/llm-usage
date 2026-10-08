# Application icons, static resources and Git LFS validation

<a id="应用图标静态资源与-git-lfs-验证"></a>

Date: 2026-09-24. Environment: Windows x64, PowerShell 7, Node.js 24.21.0, Git LFS 3.7.1;
Tauri CLI 2.11.5 and tauri-build 2.6.3, existing locks, no added dependencies. The working
tree already contained other M0 changes; this task added resources, their integration,
build and LFS configuration. Design, inventory and reproduction commands are in the
[resource guide](../../../desktop/assets/README.md).

<a id="实际结果"></a>

## Results

| Command/check | Working directory | Exit | Result |
| --- | --- | --- | --- |
| node desktop/scripts/assets.mjs --generate | Repository root | 0 | 67 generated files; 82 resources including sources/UI/illustrations checked |
| npm run assets:check | Repository root | 0 | Rebuilt files match; PNG RGBA8, ICO sizes, ICNS layers, Tauri references and LFS attributes pass |
| node desktop/scripts/assets.mjs --help | Repository root | 0 | Noninteractive options, dependencies and side effects shown |
| npm run check | Repository root | 0 | svelte-check: 0 errors, 0 warnings |
| npm run build:web | Repository root | 0 | Favicon, brand SVG, UI and illustrations included; review page excluded from release entry |
| cargo fmt --manifest-path desktop/src-tauri/Cargo.toml --check | Repository root | 0 | Build-script formatting passes |
| node node_modules/@tauri-apps/cli/tauri.js build --bundles nsis | desktop | 0 | Windows release and NSIS rebuilt after icon dependency fix |
| Headless Edge + CDP, temporary build/check-assets-browser.mjs | Repository root | 0 | 35 images load; no script errors; theme switching works; no horizontal overflow at 1440/390 px |
| LFS content check, temporary build/check-assets-lfs.mjs | Repository root | 0 | 94 indexed pointers match working-tree/local LFS objects by SHA-256/size; no missing tracked binaries |
| git lfs fsck --objects --pointers | Repository root | 0 | Latest HEAD objects and pointers pass |
| Prototype before/after comparison | Repository root | 0 | 12 original files byte-identical to f7c689c, including 9 pyc files, database, prebuilt JS and static JSON |
| YAML parsing | Repository root | 0 | All four job checkouts use lfs: true; frontend/three-platform builds check resources |
| npm run lint:md, git diff --check, git diff --cached --check | Repository root | 0 | Documentation and diff formatting pass |

LFS files total 9,121,435 logical bytes, including the existing prototype; this is not
remote transfer size. Migration updates the current index only and keeps historical Git
blobs. This task did not commit, push or rewrite history. At completion, parallel work
had created 3588345 (M0), including resources, LFS and build fixes. Resource checks passed
again on that HEAD; this record and references were added separately in the working tree.
New design sources, icons, UI and illustrations are SVG geometry, with no external fonts
or image downloads.

Final NSIS: desktop/src-tauri/target/release/bundle/nsis/llm-usage-m0_0.1.0_x64-setup.exe,
1,892,808 bytes, SHA-256:
`98698514D0626D30D0AAB7B572D7A47F951081B2F8A373D3DE765D2DC25779FD`.
Extracting the icon from an EXE copied to a new path visibly confirmed Usage U, avoiding
the old path's Shell cache. This package was not installed and no desktop process started.

<a id="发现并修复的问题"></a>

## Problems found and fixed

- Two Tauri CLI 2.11.5 ICNS outputs had identical layer hashes in different order. The
  generator now fixes layer order and retains legacy RGB/transparency pairs; generated
  bytes then matched.
- Initial release packaging passed, but extracting the EXE at a new path still showed
  the old blue icon. Installed tauri-build 2.6.3 did not declare ICO rerun-if-changed for
  Windows resource compilation. build.rs now tracks icons; rebuild/package/extraction passed.
- Initial preview exited with a watched-file EBUSY during configuration reload. Starting
  and closing the server in the validation process allowed a passing browser rerun;
  another task's Vite configuration was not changed.
- Screenshots showed the empty-state title wrapping beside an inline image. Block layout
  fixed it; the light preview now explicitly sets color-scheme.

<a id="验证结果与未执行项"></a>

<a id="证据与未执行项"></a>

## Saved results and unexecuted checks

Ignored local build/assets-validation/ contains light/dark/390 px screenshots, browser
and LFS check JSON, and the final EXE icon. Temporary browser and port-1428 server stopped.
Screenshots include only this task's resource review page, not other desktop windows.

GitHub CI, remote LFS upload/download, macOS/Linux desktop rendering and Windows taskbar
display after installation were not tested. Tray, navigation and empty-state integration
remain M6 work. This stage delivered static resources without enabling background jobs
or collection. Windows release/icon extraction does not replace these platform/install checks.
