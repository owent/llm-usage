# M0：三平台 CI 矩阵建立

后续补齐 Linux Rust 作业依赖、macOS 应用归档和制品报告测试，见
[M0/M1 审查](m0-m1-review.md)；GitHub 实际运行仍待登记。

## 元信息

| 项目 | 内容 |
| --- | --- |
| 日期 | 2026-09-24 |
| 执行环境 | 本机编写；GitHub Actions 尚未运行（本轮不推送触发） |
| 代码 revision | 工作树未提交改动 |
| 依据合同 | platform-ci.md「GitHub CI 合同」 |

## 实际建立的内容

`.github/workflows/ci.yml`：触发为 PR、main push、手动；`permissions: contents: read`。

| 作业 | runner | 内容 |
| --- | --- | --- |
| docs | ubuntu-22.04 | 根 `npm ci`、`npm run lint:md` |
| frontend | ubuntu-22.04 | desktop `npm ci`、`npm run check`、`npm run build` |
| rust | ubuntu-22.04 | `cargo fmt --check`、`cargo clippy --locked -D warnings`、`cargo test --locked` |
| build | windows-2022 / ubuntu-22.04 / macos-15，fail-fast=false | 前端构建 + `npx tauri build`，产出 NSIS/deb+AppImage/.app，记录大小与 SHA-256 并上传 artifact（保留 14 天） |

- Actions 固定完整 commit SHA：checkout `d23441a4…`（v6）、setup-node `24997072…`（v6）、
  upload-artifact `b7c566a7…`（v6）、dtolnay/rust-toolchain `6bed0761…`（stable 分支当前指向）、
  swatinem/rust-cache `6323deb1…`（v2）。SHA 经 GitHub API 解引用到 commit。
- Node 固定 24、Rust 固定 1.98.0；缓存键含 OS、架构、工具链版本与锁文件。
- Linux 作业安装 WebKitGTK 4.1 等 Tauri 系统依赖；命令与 desktop/ 实际锁文件一致。
- 无发布/签名/自动更新步骤；artifact 与 GitHub Release 分开。

## 未执行项

| 项 | 状态 | 原因 |
| --- | --- | --- |
| 三平台作业实际运行 | **已执行并登记（2026-09-30，见下节）** | — |
| fixture 测试作业 | 占位 | M0 尚无解析器测试；M1 起随核心库加入 `cargo test` 实际用例 |
| 桌面 WebDriver 集成 | 未建立 | M6 范围；macOS 嵌入式路径待那时评估 |
| release 资源/包体回归作业 | 未建立 | M7 范围 |

## 三平台首次登记运行（2026-09-30）

| 运行 | 触发/HEAD | 结论 | 证据 |
| --- | --- | --- | --- |
| [36444880100](https://github.com/owent/llm-usage/actions/runs/36444880100) | push `当前进度暂时完成`（7831f89，2026-09-28） | **success**（6/6 作业：windows-2022 / ubuntu-22.04 / macos-15 release 构建 + Markdown 文档检查 + Rust fmt/clippy/test + 前端类型检查与构建；5m18s） | `gh run view` 查询（2026-09-30） |
| [36707037687](https://github.com/owent/llm-usage/actions/runs/36707037687) | push `F2 费用估算引擎…`（118c442，2026-09-30） | **failure**（Rust 作业：kilo busy_writer 在 Linux 上暴露暂存备份 Busy 重试计入页数的既有竞速——20.5s 页上限先于 30s 超时触发误报 space cap；Windows 本地超时出口先触发故全绿。修复见 36709419197） | `gh run view --log-failed` + WSL Linux 复现 |
| [36709419197](https://github.com/owent/llm-usage/actions/runs/36709419197) | push `修复暂存备份 Busy 重试计入页数的平台相关竞速`（cb28454） | **success**（6/6；Windows/WSL 双平台复测一致） | `gh run list/view` |
| [36709463931](https://github.com/owent/llm-usage/actions/runs/36709463931) | push `移除误提交的调试用嵌入 worktree`（00b9643） | **success**（6/6） | `gh run list/view` |

历史注记：2026-09-26/28 两次失败运行（36256404241、36378875396）早于
36444880100，属修复过程中的中间态，不以失败运行冒充基线；
36707037687 的失败暴露的是 438261d 引入的跨平台测试竞速（暂存备份限额
注释与代码不一致，2s→30s 后两出口竞速），随 cb28454 修复并全绿。
