---
title: 开发环境
description: 准备已核验工具链，运行桌面程序或文档站。
sidebar:
  order: 1
---

## 前提与依赖归属

仓库工具使用 Node.js 22+，Astro 文档站要求至少 22.12。CI 选择 Node 24 和 Rust 1.99.0。
根目录的 `rust-toolchain.toml` 在本地选择同一 Rust 版本。桌面 manifest 声明 MSRV 1.90，
这是 Tauri 2.12.2 的要求；复现当前验收时使用受测工具链。
文档注释检查还需要 Python 3 和 PowerShell 7，用于解析 Python 与 PowerShell 源码，
不执行被检查文件。缺少时通过惯用开发环境准备；`check:docs` 不自动安装工具。

根 `package.json`、`package-lock.json` 管理文档/工具依赖和统一命令入口。
`desktop/package.json`、其锁文件及 Cargo manifests/lockfile 管理应用。
按锁文件恢复，不从 caret 范围推断精确版本。图片、字体和二进制资源使用 Git LFS，
指针不是可用资源。

```powershell
git lfs install --local
git lfs pull
npm ci
npm --prefix desktop ci
npm run assets:check
```

也可以在根目录执行 `pnpm install --frozen-lockfile`。工作区包含根目录和 `desktop`，
通过 `packageManager` 固定 pnpm 12.10.1；使用 pnpm 时，根 postinstall 跳过嵌套的 npm 安装。
现有命令也支持 `pnpm run`。两个 npm 锁文件与共享的 pnpm 锁文件须同步维护。
工作区允许 esbuild 和 sharp 的构建脚本，并为刚发布的依赖记录精确版本例外；
保留 peer 检查和限定范围的批准。

TypeScript 保留在 6.0.3，因为 Astro 和 Svelte 检查器需要编译器 API，接受 TypeScript 5 或 6。
TypeScript 7.0.2 尚无此 API，详见
[TypeScript 7 发布说明](https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/)。

Linux Tauri 构建需要 `.github/workflows/ci.yml` 中的包，包括 WebKitGTK 4.1、GTK、
OpenSSL、appindicator 及构建工具。macOS 需要相应原生环境。平台编译、WSL 构建和
原生 GUI 测试是独立结果。

## 运行应用

```powershell
npm run dev:desktop
```

启动 debug 后端与 Vite 热重载开发 GUI，不打包发行。`npm run dev:web` 仅在
`127.0.0.1:1421` 启动前端，没有 Tauri 后端。浏览器交互检查使用模拟 IPC 的浏览器测试入口。

```powershell
npm run verify
npm run build:desktop
npm run test:headless
```

`verify` 检查 Markdown、资源、脚本、UI 逻辑、Svelte 类型、Rust fmt/clippy/test 和前端构建。
无界面/原生脚本要求已构建程序并隔离来源；仅 `--data-dir` 不隔离发现范围。

## 运行文档

```powershell
npm run dev:docs
npm run check:docs
npm run build:docs
npm run test:docs
npm run test:docs:browser
```

文档产物和参考生成内容位于根 `build/documentation-site/`。修改语言配对或发布输入前，
阅读[文档维护](/zh-cn/development/documentation/)。源码注释使用英文，中文对照随对应代码维护。
