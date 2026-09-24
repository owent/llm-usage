# M0：三平台 CI 矩阵建立

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
| 三平台作业实际运行 | 未执行 | 本轮不推送；首次运行结果需另登记 |
| fixture 测试作业 | 占位 | M0 尚无解析器测试；M1 起随核心库加入 `cargo test` 实际用例 |
| 桌面 WebDriver 集成 | 未建立 | M6 范围；macOS 嵌入式路径待那时评估 |
| release 资源/包体回归作业 | 未建立 | M7 范围 |
