# M0: Windows baseline, locked versions and minimal release trial

<a id="m0windows-基线版本锁定与最小-release-试验"></a>

<a id="元信息"></a>

## Run information

| Item | Value |
| --- | --- |
| Date | 2026-09-24 |
| Environment | Windows 11 Pro 26200 x64; Ryzen 7 9700X, 8C/16T; 64 GiB; Node v24.21.0/npm 12.0.2; Rust 1.98.0; registry-verified WebView2 153.0.4234.48 |
| Code revision | Uncommitted M0 working tree: new desktop/ and restored root package.json |
| Design references | execution.md M0; architecture.md resource targets; platform-ci.md platform matrix |

<a id="命令与结果"></a>

## Commands and results

Root unless stated; complete steps in build/m0-release-trial/summary.md.

| # | Command (directory) | Exit | Result |
| --- | --- | --- | --- |
| 1 | npm install --package-lock-only (root) | 0 | Root lock restored; npm ci --dry-run passes |
| 2 | npm run lint:md (root) | 0 | 26 clean files; previous-draft ignored as frozen prototype, 6 existing issues retained |
| 3 | npm install (desktop/) | 0 | 52 packages, 0 vulnerabilities; package-lock.json generated |
| 4 | npm run check (desktop/, svelte-check) | 0 | 0 errors/warnings |
| 5 | npm run build (desktop/, vite) | 0 | 2.10 s |
| 6 | cargo generate-lockfile (desktop/src-tauri) | 0 | 439 crates; Cargo.lock generated |
| 7 | npx tauri build (desktop/) | 0 | 120 s; NSIS generated |
| 8 | Pure TypeScript comparison, build/m0-pure-ts-compare/ | 0 | vite 0.871 s |
| 9 | Silent NSIS install in build/m0-install-test/ and du | 0 | Installed-directory measurement |
| 10 | measure-procs.ps1 process memory | 0 | Seven-process inventory in build/m0-release-trial/process-list.txt |

<a id="锁定版本"></a>

## Locked versions

| Dependency | Locked | Resolved | License | Reference |
| --- | --- | --- | --- | --- |
| @tauri-apps/cli | 2.11.5 | 2.11.5 | Apache-2.0 OR MIT | desktop/package-lock.json |
| @tauri-apps/api | 2.11.1 | 2.11.1 | Apache-2.0 OR MIT | Same |
| vite | 8.3.0 | 8.3.0 | MIT | Same |
| svelte | 5.57.1 | 5.57.1 | MIT | Same |
| @sveltejs/vite-plugin-svelte | 7.3.1 | 7.3.1 | MIT | Same |
| typescript | 5.9.3 | 5.9.3 | Apache-2.0 | Same; svelte-check peer ^5/6 excludes TS 7.0.2, so 5.9.3 fixed |
| svelte-check | 4.7.6 | 4.7.6 | MIT | Same |
| echarts | 6.1.0 | 6.1.0 | Apache-2.0 | Same |
| tauri | =2.11.6 | 2.11.6 | Apache-2.0 OR MIT | desktop/src-tauri/Cargo.lock |
| tauri-build | =2.6.3 | 2.6.3 | Apache-2.0 OR MIT | Same |
| rusqlite, bundled | =0.40.2 | 0.40.2/libsqlite3-sys 0.38.2 | MIT; SQLite public domain | Same; runtime sqlite_version()=3.53.2 meets ≥3.51.3 requirement |
| serde/serde_json | =1.0.229/=1.0.151 | Same | MIT OR Apache-2.0 | Same |

Transitive licenses: build/m0-release-trial/npm-licenses-desktop.tsv and cargo-licenses.tsv.
npm contains two MPL-2.0 lightningcss-family transitive Vite 8 packages; Cargo no GPL-only ones.

<a id="测量与测试"></a>

## Measurements and tests

| Metric | Target | Observed | Method | Result |
| --- | --- | --- | --- | --- |
| NSIS | ≤20 MiB | 1.670 MiB, 1,751,286 bytes | target/release/bundle/nsis/…_x64-setup.exe, excluding WebView2 | Met |
| Installed directory | ≤60 MiB | 4.124 MiB, 4,323,622 bytes | Actual silent install and du | Met |
| Frontend JS+CSS gzip | ≤1 MiB | 170.5 KiB, uncompressed 506.0 KiB | Node zlib gzip-9 sum by file | Met |
| Idle all-process memory | ≤180 MiB private bytes | **206.5 MB**: main 4.8 plus six WebView2 children 201.7; working set 381.7 MB | Process-tree traversal after 15 s idle | **Not met**, 26.5 MB over; investigate children before M7 |
| Bundled SQLite | ≥3.51.3 | 3.53.2 | Application-window SELECT sqlite_version() | Met |
| IPC/backend | Minimal check | rows=2; 190-byte file read | sqlite_probe/read_sample_file; screenshot retained | Passed |

Pure TypeScript same-chart gzip: 160.4 KiB; Svelte adds 10.3 KiB, 6.3%, insufficient reason
to switch frameworks. Compare maintenance costs at actual M6 page scale.

<a id="失败与未执行项"></a>

## Failures and unexecuted checks

| Item | Status | Cause | Next condition |
| --- | --- | --- | --- |
| Idle memory target | Not met | Six WebView2 children, 201.7 MB | Investigate/retest M6/M7; include child processes |
| Separate cargo-tauri install | Not performed | npm CLI suffices | Assess only if global CLI needed |
| Screenshot privacy incident | Resolved | First CopyFromScreen captured an overlapping window; file deleted | Use PrintWindow for this app's window only |
| GitHub CI | Not run | No trigger push in this stage | Record actual run after push |

<a id="验证产物"></a>

<a id="证据文件"></a>

## Validation files

- desktop/: minimal app and package-lock.json/Cargo.lock.
- Ignored build/m0-release-trial/: summary.md, process-list.txt, app-window.png, license TSVs,
  measurement scripts.
- build/m0-install-test/: silent-install target.
- build/m0-pure-ts-compare/: comparison project/output.
