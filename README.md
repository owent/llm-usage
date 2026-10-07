# llm-usage

[![status](https://img.shields.io/badge/status-Pre--Alpha-orange)](Plan.md)
[![platform](https://img.shields.io/badge/platform-Windows_11_x64_%E9%A6%96%E5%8F%91-0078D6)](docs/design/desktop-usage/platform-ci.md)
[![CI targets](https://img.shields.io/badge/CI-Windows_%C2%B7_macOS_%C2%B7_Linux-informational)](docs/design/desktop-usage/platform-ci.md)
[![数据范围](https://img.shields.io/badge/%E7%BB%9F%E8%AE%A1%E8%8C%83%E5%9B%B4-%E4%BB%85%E6%9C%AC%E6%9C%BA_Agent_%E6%95%B0%E6%8D%AE-blue)](docs/design/desktop-usage/data-contract.md)
[![license](https://img.shields.io/badge/license-%E6%9C%AA%E6%8C%87%E5%AE%9A-lightgrey)](https://github.com/owent/llm-usage)

[![Tauri](https://img.shields.io/badge/Tauri-2.12-FFC131?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-1.98.x-DEA584?logo=rust)](https://www.rust-lang.org)
[![Svelte](https://img.shields.io/badge/Svelte-5.57-FF3E00?logo=svelte)](https://svelte.dev)
[![TypeScript](https://img.shields.io/badge/TypeScript-6.0-3178C6?logo=typescript)](https://www.typescriptlang.org)
[![Node.js](https://img.shields.io/badge/node.js-%E2%89%A522_%C2%B7_24-339933?logo=nodedotjs)](https://nodejs.org)
[![Vite](https://img.shields.io/badge/Vite-8-646CFF?logo=vite)](https://vite.dev)
[![SQLite](https://img.shields.io/badge/SQLite-rusqlite_0.40-003B57?logo=sqlite)](https://www.sqlite.org)
[![ECharts](https://img.shields.io/badge/ECharts-6.1-AA344D?logo=apacheecharts)](https://echarts.apache.org)

[![verify](https://img.shields.io/badge/npm_run_verify-%E9%80%9A%E8%BF%87_2026--10--07-brightgreen)](docs/validation/desktop-usage/current-acceptance.md)
[![Git LFS](https://img.shields.io/badge/Git_LFS-%E9%9D%99%E6%80%81%E8%B5%84%E6%BA%90-blue?logo=git)](desktop/assets/README.md)
[![repo size](https://img.shields.io/github/repo-size/owent/llm-usage)](https://github.com/owent/llm-usage)
[![last commit](https://img.shields.io/github/last-commit/owent/llm-usage)](https://github.com/owent/llm-usage/commits)
[![issues](https://img.shields.io/github/issues/owent/llm-usage)](https://github.com/owent/llm-usage/issues)
[![languages](https://img.shields.io/github/languages/count/owent/llm-usage)](https://github.com/owent/llm-usage)

本项目提供本地 AI Agent 用量桌面客户端，统计各模型的 token、请求和缓存使用。
目前处于预发布阶段，已实现总览、趋势、详情、数据源和设置；支持范围及本轮结果见执行计划。
支持完整标准化明细导入/Merge，以及默认关闭的日/月 token 或单币种估算预算提醒。

已确认 Windows 11 x64 首发，GitHub CI 保留 macOS/Linux，本地可尝试 WSL 构建。
来源能力与环境受限条件保留在接入矩阵，缺安装/载体的 F1 IDE 已移出本轮；仅统计本机来源，支持在界面配置定时提取。
用户已允许实施时提取本机真实 Agent 数据验证。

- [执行计划](Plan.md)：当前结果、执行范围与移出条件。
- [开工准备](docs/design/desktop-usage/implementation-readiness.md)：已确认范围、真实数据验证流程与检查结论。
- [详细设计](docs/design/desktop-usage/README.md)：架构、统计合同、Agent 接入、配置与测试。
- [调研依据](docs/design/desktop-usage/research.md)：官方资料、固定源码和原型静态核对。
- [定时提取](docs/design/desktop-usage/scheduling.md)与 [平台/CI](docs/design/desktop-usage/platform-ci.md)：后台任务和跨平台验收合同。
- [验证记录](docs/validation/desktop-usage/)：M0 起的实际执行记录，随检查完成逐条登记。
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
npm run test:ui         # 前端纯逻辑回归
npm run test:browser    # 五页浏览器回归（模拟 IPC；Windows 使用已安装的 Edge）
npm run clippy          # Rust 静态检查（-D warnings）
npm run fmt:check       # Rust 格式检查
npm run build:desktop   # 桌面 release 构建（产出 NSIS/deb/AppImage/.app 按平台）
npm run test:headless   # 真实可执行文件与 SQLite，隔离合成来源；先构建
npm run test:desktop    # Windows 原生 WebView2/IPC，要求 CDP 可用；先构建
npm run verify          # 文档、类型、脚本/前端单元测试、Rust 检查与测试、前端构建
```

`test:browser` 单独执行，会启动并关闭临时 Vite 服务；截图写入 `build/browser-smoke/`。
非 Windows 环境先在 `desktop` 中运行 `npx playwright install chromium`。
浏览器检查模拟 IPC，不能替代原生桌面、系统任务和安装验收；当前验证结果与未完成项见
[最新验收](docs/validation/desktop-usage/current-acceptance.md)。
原生与无界面脚本的输入为隔离合成来源，输出写入根 `build/plan-completion/`。
`test:desktop` 默认测 release；调试构建可用 `-- --dev --exe <debug 可执行文件>`。
日常功能验证用 dev:desktop 即可，不必打包。业务命令与锁定版本以
`desktop/package.json`、`desktop/src-tauri/Cargo.toml` 及各自锁文件为准。
文档检查不代替业务验收；M0 实测结果见 [验证记录](docs/validation/desktop-usage/)。

Windows 后台提取默认关闭，设置页可启用当前用户的分钟任务，并展示期望与实际状态。
`LLMUsage.exe --headless` 只按已保存意图及到期规则采集；`--scan-once` 手动扫描全部
启用来源。可用 `--data-dir <绝对目录>` 独立保存数据库和导出；它不改变来源发现范围。
需限定采集范围时，在设置中启用“仅扫描手工目录”；保存的规则同时约束 GUI 和后台任务。

图片（含 SVG）、字体、媒体及二进制文件使用 Git LFS；首次构建前须下载实际资源。
重新生成图标：`npm run assets:generate`。资源预览页：
`npm run dev:web` 后打开 `http://127.0.0.1:1421/asset-preview.html`。
