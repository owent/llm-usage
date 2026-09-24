# M0：Windows 基线、版本锁定与最小 release 试验

## 元信息

| 项目 | 内容 |
| --- | --- |
| 日期 | 2026-09-24 |
| 执行环境 | Windows 11 Pro 26200 x64；Ryzen 7 9700X（8C/16T）；64 GiB；Node v24.21.0 / npm 12.0.2；Rust 1.98.0；WebView2 153.0.4234.48（注册表核实） |
| 代码 revision | 工作树含 M0 未提交改动（desktop/ 新建，根 package.json 恢复） |
| 依据合同 | execution.md M0；architecture.md 资源目标；platform-ci.md 平台矩阵 |

## 命令与结果

cwd 为仓库根或标注的子目录；关键步骤（完整清单见 `build/m0-release-trial/summary.md`）：

| # | 命令（cwd） | 退出码 | 结果摘要 |
| --- | --- | --- | --- |
| 1 | `npm install --package-lock-only`（根） | 0 | 恢复根锁文件；`npm ci --dry-run` 通过 |
| 2 | `npm run lint:md`（根） | 0 | 26 文件 0 问题；previous-draft 列入 ignores（冻结原型，既有 6 问题不改） |
| 3 | `npm install`（desktop/） | 0 | 52 包，0 漏洞，生成 package-lock.json |
| 4 | `npm run check`（desktop/，svelte-check） | 0 | 0 errors 0 warnings |
| 5 | `npm run build`（desktop/，vite） | 0 | 2.10 s |
| 6 | `cargo generate-lockfile`（desktop/src-tauri） | 0 | 439 crate，生成 Cargo.lock |
| 7 | `npx tauri build`（desktop/） | 0 | 120 s；NSIS 安装包产出 |
| 8 | 纯 TS 对照构建（build/m0-pure-ts-compare/） | 0 | vite 0.871 s |
| 9 | NSIS 静默安装到 `build/m0-install-test/` + `du` | 0 | 实测安装目录 |
| 10 | 进程内存测量（measure-procs.ps1） | 0 | 7 进程清单见 `build/m0-release-trial/process-list.txt` |

## 锁定版本

| 依赖 | 锁定版本 | 实际解析 | 许可证 | 来源 |
| --- | --- | --- | --- | --- |
| @tauri-apps/cli | 2.11.5 | 2.11.5 | Apache-2.0 OR MIT | desktop/package-lock.json |
| @tauri-apps/api | 2.11.1 | 2.11.1 | Apache-2.0 OR MIT | 同上 |
| vite | 8.3.0 | 8.3.0 | MIT | 同上 |
| svelte | 5.57.1 | 5.57.1 | MIT | 同上 |
| @sveltejs/vite-plugin-svelte | 7.3.1 | 7.3.1 | MIT | 同上 |
| typescript | 5.9.3 | 5.9.3 | Apache-2.0 | 同上；svelte-check peer 为 ^5/6，TS 7.0.2 不支持故固定 5.9.3 |
| svelte-check | 4.7.6 | 4.7.6 | MIT | 同上 |
| echarts | 6.1.0 | 6.1.0 | Apache-2.0 | 同上 |
| tauri | =2.11.6 | 2.11.6 | Apache-2.0 OR MIT | desktop/src-tauri/Cargo.lock |
| tauri-build | =2.6.3 | 2.6.3 | Apache-2.0 OR MIT | 同上 |
| rusqlite（bundled） | =0.40.2 | 0.40.2 / libsqlite3-sys 0.38.2 | MIT（SQLite 公有领域） | 同上；运行时 `sqlite_version()`=3.53.2 实证 ≥3.51.3 合同 |
| serde / serde_json | =1.0.229 / =1.0.151 | 同左 | MIT OR Apache-2.0 | 同上 |

传递依赖许可证明细：`build/m0-release-trial/npm-licenses-desktop.tsv`、`cargo-licenses.tsv`。
npm 侧含 MPL-2.0 两件（lightningcss 系，vite 8 传递依赖）；Cargo 侧无纯 GPL。

## 测量与测试

| 指标 | 拟定目标 | 实测 | 方法 | 结论 |
| --- | --- | --- | --- | --- |
| NSIS 安装包 | ≤ 20 MiB | 1.670 MiB（1,751,286 B） | `target/release/bundle/nsis/…_x64-setup.exe`，不含 WebView2 | 达标 |
| 安装目录 | ≤ 60 MiB | 4.124 MiB（4,323,622 B） | NSIS 静默实装 + du | 达标 |
| 前端 JS+CSS gzip | ≤ 1 MiB | 170.5 KiB（未压缩 506.0 KiB） | Node zlib gzip-9 逐文件求和 | 达标 |
| 空闲内存（全进程） | ≤ 180 MiB private bytes | **206.5 MB**（主进程 4.8 + 6 个 WebView2 子进程 201.7；working set 合计 381.7 MB） | 安装后空闲 15 s，进程树遍历 | **未达标**，超出 26.5 MB；M7 前需定位 WebView2 子进程占用 |
| bundled SQLite | ≥ 3.51.3 | 3.53.2 | 应用窗口实证 `SELECT sqlite_version()` | 达标 |
| IPC/后端命令 | 最小验证 | rows=2；文件读取 190 B | sqlite_probe / read_sample_file，截图存档 | 通过 |

Svelte 对照：纯 TS 版同一柱图 gzip 160.4 KiB，Svelte 增量 gzip +10.3 KiB（+6.3%），
未构成换框架理由；维护成本对照留 M6 实际页面规模评估。

## 失败与未执行项

| 项 | 状态 | 原因 | 后续条件 |
| --- | --- | --- | --- |
| 空闲内存目标 | 未达标 | WebView2 六子进程 201.7 MB | M6/M7 定位与复测，不以隐藏子进程美化 |
| cargo-tauri 独立安装 | 未执行 | 使用 npm @tauri-apps/cli 已满足 | 需要全局 CLI 时再评估 |
| 截图隐私事故 | 已处置 | 首次 CopyFromScreen 拍到遮挡窗口，文件已删 | 截图统一用 PrintWindow 只含本应用窗口 |
| GitHub CI 实际运行 | 未执行 | 本轮不推送触发 | 推送后按运行结果登记 |

## 证据文件

- `desktop/`：最小应用源码与锁文件（package-lock.json、Cargo.lock）。
- `build/m0-release-trial/`（gitignored）：summary.md、process-list.txt、app-window.png、许可证 TSV、测量脚本。
- `build/m0-install-test/`：静默安装目标目录。
- `build/m0-pure-ts-compare/`：纯 TS 对照工程与产物。
