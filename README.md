# llm-usage

[![status](https://img.shields.io/badge/status-M0_%E8%BF%9B%E8%A1%8C%E4%B8%AD_%C2%B7_Pre--Alpha-orange)](Plan.md)
[![platform](https://img.shields.io/badge/platform-Windows_11_x64_%E9%A6%96%E5%8F%91-0078D6)](docs/design/desktop-usage/platform-ci.md)
[![CI targets](https://img.shields.io/badge/CI-Windows_%C2%B7_macOS_%C2%B7_Linux-informational)](docs/design/desktop-usage/platform-ci.md)
[![数据范围](https://img.shields.io/badge/%E7%BB%9F%E8%AE%A1%E8%8C%83%E5%9B%B4-%E4%BB%85%E6%9C%AC%E6%9C%BA_Agent_%E6%95%B0%E6%8D%AE-blue)](docs/design/desktop-usage/data-contract.md)
[![license](https://img.shields.io/badge/license-%E6%9C%AA%E6%8C%87%E5%AE%9A-lightgrey)](https://github.com/owent/llm-usage)

[![Tauri](https://img.shields.io/badge/Tauri-2.11-FFC131?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-1.98.x-DEA584?logo=rust)](https://www.rust-lang.org)
[![Svelte](https://img.shields.io/badge/Svelte-5.57-FF3E00?logo=svelte)](https://svelte.dev)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.9-3178C6?logo=typescript)](https://www.typescriptlang.org)
[![Node.js](https://img.shields.io/badge/node.js-%E2%89%A522_%C2%B7_24-339933?logo=nodedotjs)](https://nodejs.org)
[![Vite](https://img.shields.io/badge/Vite-8-646CFF?logo=vite)](https://vite.dev)
[![SQLite](https://img.shields.io/badge/SQLite-rusqlite_0.40-003B57?logo=sqlite)](https://www.sqlite.org)
[![ECharts](https://img.shields.io/badge/ECharts-6.1-AA344D?logo=apacheecharts)](https://echarts.apache.org)

[![verify](https://img.shields.io/badge/npm_run_verify-%E9%80%9A%E8%BF%87_2026--09--24-brightgreen)](docs/validation/desktop-usage/)
[![Git LFS](https://img.shields.io/badge/Git_LFS-%E9%9D%99%E6%80%81%E8%B5%84%E6%BA%90-blue?logo=git)](desktop/assets/README.md)
[![repo size](https://img.shields.io/github/repo-size/owent/llm-usage)](https://github.com/owent/llm-usage)
[![last commit](https://img.shields.io/github/last-commit/owent/llm-usage)](https://github.com/owent/llm-usage/commits)
[![issues](https://img.shields.io/github/issues/owent/llm-usage)](https://github.com/owent/llm-usage/issues)
[![languages](https://img.shields.io/github/languages/count/owent/llm-usage)](https://github.com/owent/llm-usage)

本项目计划提供本地 AI Agent 用量桌面客户端，统计各模型的 token、请求和缓存使用。
实施前的设计准备已完成；M0 进行中，桌面客户端尚未实现。

已确认 Windows 11 x64 首发，GitHub CI 保留 macOS/Linux，本地可尝试 WSL 构建。
所有 Agent 均保留本地支持计划，缺证 IDE 后移 F1；仅统计本机来源，支持计划中的界面定时提取配置。
用户已允许实施时提取本机真实 Agent 数据验证。

- [执行计划](Plan.md)：未完成任务与验收条件。
- [开工准备](docs/design/desktop-usage/implementation-readiness.md)：已确认范围、真实数据验证流程与检查结论。
- [详细设计](docs/design/desktop-usage/README.md)：架构、统计合同、Agent 接入、配置与测试。
- [调研依据](docs/design/desktop-usage/research.md)：官方资料、固定源码和原型静态核对。
- [定时提取](docs/design/desktop-usage/scheduling.md)与 [平台/CI](docs/design/desktop-usage/platform-ci.md)：后台任务和跨平台验收合同。
- [验证记录](docs/validation/desktop-usage/)：M0 起的实际执行证据，随证据产生逐条登记。
- [应用图标与静态资源](desktop/assets/README.md)：Usage U 设计、预览、重新生成与 Git LFS 约定。
- 已有 [previous-draft](previous-draft/README.md) 作为参考，未进行运行验收。

- 共享规则：[AGENTS.md](AGENTS.md)。
- 维护流程与按需资源：[Skills](.agents/skills/README.md)。

## 常用命令

Node.js 22+（当前锁定 24）与 Rust（当前锁定 1.98.x）。在仓库根直接执行：

```powershell
git lfs install --local  # 为当前克隆启用 LFS（首次）
git lfs pull            # 下载图标及其他静态/二进制资源
npm ci                  # 恢复文档工具依赖（首次）
npm --prefix desktop ci # 恢复桌面工具依赖（首次）
npm run assets:check    # 检查资源格式、派生文件与 LFS 属性
npm run lint:md         # Markdown 检查
npm run dev:web         # 仅前端 Vite 热更新服务（http://127.0.0.1:1421，无后端）
npm run dev:desktop     # 开发模式拉起 GUI（debug 构建 + 热重载，不打包）
npm run check           # 前端类型检查（svelte-check）
npm run build:web       # 前端产物构建
npm run test:rust       # Rust 测试
npm run clippy          # Rust 静态检查（-D warnings）
npm run fmt:check       # Rust 格式检查
npm run build:desktop   # 桌面 release 构建（产出 NSIS/deb/AppImage/.app 按平台）
npm run verify          # 以上检查与测试的一条龙（不含 build:desktop）
```

`npm run verify` 与 `npm run dev:desktop` 已于 2026-09-24 实测退出码 0、GUI 窗口正常出现；
日常功能验证用 dev:desktop 即可，不必打包。业务命令与锁定版本以
`desktop/package.json`、`desktop/src-tauri/Cargo.toml` 及各自锁文件为准。
文档检查不代替业务验收；M0 实测证据见 [验证记录](docs/validation/desktop-usage/)。

图片（含 SVG）、字体、媒体及二进制文件使用 Git LFS；首次构建前须下载实际资源。
重新生成图标：`npm run assets:generate`。资源预览页：
`npm run dev:web` 后打开 `http://127.0.0.1:1421/asset-preview.html`。
