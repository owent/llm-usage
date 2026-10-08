# M0: WSL 2 Linux build and WSLg smoke checks

<a id="m0wsl-2-linux-构建与-wslg-冒烟"></a>

<a id="元信息"></a>

## Run information

| Item | Value |
| --- | --- |
| Date | 2026-09-24 |
| Environment | WSL 2 Debian 13 (trixie) x86_64; kernel 6.18.33.2-WSL2; 869 GiB free; WSLg available, WAYLAND_DISPLAY=wayland-0 |
| Tools | Node v24.21.0 official tarball installed in user directory, matching Windows; preinstalled rustup Rust 1.98.1 vs Windows 1.98.0; npm 11.19.0 vs Windows 12.0.2, difference recorded |
| Code revision | Copy of Windows working tree at ~/llm-usage-m0/; tar excludes node_modules/dist/target/gen; 576 KiB |
| Design reference | platform-ci.md, local WSL execution sequence |

<a id="命令与结果"></a>

## Commands and results

| # | Command (WSL ~/llm-usage-m0) | Exit | Time/result |
| --- | --- | --- | --- |
| 1 | Environment discovery: uname/os-release/df/WSLg | 0 | Above |
| 2 | Install Node tarball at ~/.local/node-v24.21.0 | 0 | 19 s |
| 3 | apt download 674 Tauri dependency packages; dpkg-deb -x into ~/.local/m0-sysroot, 2.2 GiB | 0 | 183 s; **sudo password locked; no root used**, user-space sysroot instead |
| 4 | npm ci (desktop/) | 0 | 52 packages, 3 s |
| 5 | npm run check | 0 | svelte-check: 0 errors/warnings |
| 6 | npm run build | 0 | vite 8.3.0; JS 517.92 KiB |
| 7 | cargo generate-lockfile | 0 | Cargo.lock SHA-256 matches Windows |
| 8 | cargo build --release | 0 | 93 s; stripped ELF 4,622,184 bytes |
| 9 | npx tauri build without bundle arguments | 0 | 39 s; **binary only, no package**: nsis-only configuration did not apply to Linux; fixed below |
| 10 | npx tauri build --bundles deb,appimage | 1 | deb succeeded; AppImage failed as described below |
| 11 | Manual AppImage linuxdeploy with GTK plugin removed | 0 | extract-and-run survived 8 s |

Sysroot versions: webkit2gtk-4.1 2.52.6, gtk+-3.0 3.24.49, libsoup-3.0 3.6.5,
ayatana-appindicator3 0.5.94, librsvg 2.60.0 and glib 2.84.4.

<a id="产物sha256-见-buildm0-wsl-linuxreportmd"></a>

## Outputs (SHA-256 in build/m0-wsl-linux/REPORT.md)

| Output | Size | Notes |
| --- | --- | --- |
| llm-usage-m0 ELF | 4,622,184 bytes | Release executable |
| llm-usage-m0_0.1.0_amd64.deb | 2,105,848 bytes | Depends structure verified; not installed without root |
| llm-usage-m0-x86_64.AppImage | 102,189,560 bytes | Experimental: GTK runtime hook removed; embedded libraries patched with local paths |
| Windows comparison | EXE 4,244,480 bytes; NSIS 1,751,286 bytes | [Windows baseline](m0-windows-baseline.md) |

<a id="wslg-冒烟"></a>

## WSLg smoke checks

The sysroot WebKit helper contained a system /usr/lib/... path absent on this machine.
An equal-length rodata path replacement used /tmp/wkgtk41 symlinks, with originals backed
up as .m0-orig. Three checks survived 12/53/90 s: visible window, Rust IPC returned
sqlite 3.53.2 · rows=2, and ECharts rendered. Screenshot: build/m0-wsl-linux/smoke-wslg.png.
This was a local experiment without root and does not establish clean-machine behavior.

<a id="失败与未执行项"></a>

## Failures and unexecuted checks

| Item | Status | Cause/next step |
| --- | --- | --- |
| Single-command AppImage packaging | Failure causes identified | trixie removed libfuse2, causing runtime dlopen failure; after bypassing extraction, GTK plugin assumed system GTK/GI. Recheck clean paths with root; not counted as passing |
| deb installation | Not tested | No root |
| Cross-platform outputs from plain npx tauri build | Fixed | tauri.conf.json targets now [nsis,deb,appimage,app]; CI explicitly chooses --bundles by OS. Windows NSIS rebuild passed, 1,751,735 bytes, exit 0 |
| Native Linux desktop acceptance | Not tested | Existing requirements distinguish WSL/WSLg from native acceptance |

<a id="验证产物"></a>

<a id="证据文件"></a>

## Validation files

Ignored build/m0-wsl-linux/ contains REPORT.md, smoke-wslg.png and 21 execution scripts.
WSL retains ~/llm-usage-m0/, ~/.local/m0-sysroot, ~/.local/node-v24.21.0 and ~/.cache/tauri/
for review. wsl --shutdown was not run; all task processes stopped.
