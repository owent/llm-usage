# M0：WSL 2 Linux 构建与 WSLg 冒烟

## 元信息

| 项目 | 内容 |
| --- | --- |
| 日期 | 2026-09-24 |
| 执行环境 | WSL 2 Debian 13 (trixie) x86_64，内核 6.18.33.2-WSL2，869 GiB 可用；WSLg 可用（WAYLAND_DISPLAY=wayland-0） |
| 工具链 | Node v24.21.0（官方 tarball 用户目录安装，与 Windows 一致）；Rust 1.98.1（rustup 预装；Windows 为 1.98.0）；npm 11.19.0（Windows 12.0.2，差异登记） |
| 代码 revision | 与 Windows 工作树同源副本（`~/llm-usage-m0/`，tar 排除 node_modules/dist/target/gen，576 KiB） |
| 依据合同 | platform-ci.md「本地 WSL 执行顺序」 |

## 命令与结果

| # | 命令（cwd：WSL ~/llm-usage-m0） | 退出码 | 耗时/结果 |
| --- | --- | --- | --- |
| 1 | 环境探测（uname/os-release/df/WSLg） | 0 | 见元信息 |
| 2 | Node tarball 安装到 `~/.local/node-v24.21.0` | 0 | 19 s |
| 3 | Tauri 依赖：`apt download` 674 包 + `dpkg-deb -x` 入 `~/.local/m0-sysroot`（2.2 GiB） | 0 | 183 s；**sudo 密码锁定，全程无 root**，改用用户态 sysroot |
| 4 | `npm ci`（desktop/） | 0 | 52 包，3 s |
| 5 | `npm run check` | 0 | svelte-check 0 错 0 警 |
| 6 | `npm run build` | 0 | vite 8.3.0，js 517.92 KiB |
| 7 | `cargo generate-lockfile` | 0 | Cargo.lock sha256 与 Windows 一致 |
| 8 | `cargo build --release` | 0 | 93 s，ELF stripped 4,622,184 B |
| 9 | `npx tauri build`（裸命令） | 0 | 39 s，**只产二进制无安装包**：conf targets 仅 nsis 对 Linux 不适用（已修复，见下） |
| 10 | `npx tauri build --bundles deb,appimage` | 1 | deb 成功；AppImage 失败（见失败链） |
| 11 | AppImage 手动 linuxdeploy（移出 gtk 插件） | 0 | extract-and-run 冒烟 8 s 存活 |

sysroot 版本：webkit2gtk-4.1 2.52.6、gtk+-3.0 3.24.49、libsoup-3.0 3.6.5、
ayatana-appindicator3 0.5.94、librsvg 2.60.0、glib 2.84.4。

## 产物（sha256 见 `build/m0-wsl-linux/REPORT.md`）

| 产物 | 大小 | 说明 |
| --- | --- | --- |
| `llm-usage-m0` ELF | 4,622,184 B | release 可执行文件 |
| `llm-usage-m0_0.1.0_amd64.deb` | 2,105,848 B | Depends 结构核验正常；无 root 未做安装测试 |
| `llm-usage-m0-x86_64.AppImage` | 102,189,560 B | 试验产物：移除了 gtk 运行时钩子，内嵌库带本机路径补丁 |
| Windows 对照 | exe 4,244,480 B；NSIS 1,751,286 B | 见 [m0-windows-baseline.md](m0-windows-baseline.md) |

## WSLg 冒烟

sysroot 内 WebKit helper 内置路径指向系统 `/usr/lib/...`（本机未装），做了等长 rodata 路径改写
（`/tmp/wkgtk41` 符号链接，原文件 `.m0-orig` 备份）。之后三次冒烟（12 s/53 s/90 s）均存活、
窗口可见、Rust IPC 返回 `sqlite 3.53.2 · rows=2`、ECharts 渲染；截图 `build/m0-wsl-linux/smoke-wslg.png`。
该补丁仅为本机无 root 环境的试验处置，不代表干净机器行为。

## 失败与未执行项

| 项 | 状态 | 原因/后续 |
| --- | --- | --- |
| AppImage 单命令打包 | 失败链已定位 | trixie 移除 libfuse2 → runtime dlopen 失败；绕过 extract 后 gtk 插件假定系统 GTK/GI。需有 root 环境复核干净路径；不掩盖为通过 |
| deb 安装测试 | 未执行 | 无 root |
| 裸 `npx tauri build` 跨平台产物 | 已修复 | `tauri.conf.json` targets 改为 `["nsis","deb","appimage","app"]`，CI 另按 OS 显式 `--bundles`；Windows NSIS 重建通过（1,751,735 B，退出码 0） |
| 原生 Linux 桌面验收 | 未执行 | WSL/WSLg 不替代原生验收（合同既有约束） |

## 证据文件

`build/m0-wsl-linux/`（gitignored）：REPORT.md、smoke-wslg.png、21 个执行脚本。
WSL 内保留供复核：`~/llm-usage-m0/`、`~/.local/m0-sysroot`、`~/.local/node-v24.21.0`、`~/.cache/tauri/`；
未 `wsl --shutdown`，本任务进程已全部终止。
