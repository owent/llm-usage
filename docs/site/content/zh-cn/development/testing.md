---
title: 测试与验收依据
description: 每项结论使用真正覆盖该行为的验证入口。
sidebar:
  order: 4
---

在仓库根按锁文件准备依赖后执行。准确命令定义见 `package.json`，下表说明范围，不替代命令。

| 命令 | 检查范围 |
| --- | --- |
| `npm run verify` | Markdown、资源、脚本/UI 逻辑、Svelte 类型、Rust fmt/clippy/test 和前端生产构建。 |
| `npm run test:browser` | 模拟 IPC 的五页浏览器交互，Windows 使用已安装 Edge。 |
| `npm run build:desktop` | 平台 Tauri release 构建和安装包。 |
| `npm run test:headless` | 真实可执行文件/SQLite，隔离合成来源，需要已构建程序。 |
| `npm run test:desktop` | Windows WebView2 和真实 IPC，需要构建与可用 CDP。 |
| `npm run test:import` | Windows 首次规模 GUI 导入及全进程资源测量。 |
| `npm run test:receiver` | Windows 真实 IPC/HTTP/系统凭据及自有令牌回收。 |
| `npm run test:install:windows -- --help` | 无副作用说明真实 NSIS 生命周期前提。 |
| `npm run test:install:linux -- --help` | 无副作用说明隔离 Podman 包/GTK/WebKit/FUSE/Orca 前提。 |
| `npm run test:credentials:linux` | 独立 D-Bus/keyring 的原生 Linux Secret Service 往返。 |
| `npm run check:docs` / `build:docs` / `test:docs:browser` | 文档翻译/类型、生产产物/链接及浏览器行为。 |

## 回归要求

采集修改覆盖有效输入/输出、旧摘要、已消费游标、重复读取、冲突、回滚、归属及归档/保留
边界。未知保持未知。没有有效性依据的默认零不能变成已知零。来源失败不能悄然阻止其他
有效事件入库。

每次使用独立测试数据库，同时隔离来源环境和应用存储。临时脚本、日志、探测、提取记录
和测试库位于根 `build/<任务名>/`，不复用可能重开旧库的 PID 命名目录。

## 如实报告

记录 cwd、命令、环境/工具链、精确 revision 或工作树状态、退出码、耗时、首次失败、
重试结果及范围。静态、合成、真实样本、模拟浏览器、原生、安装、CI 和生产验证结果分开。
文档构建通过不能验证应用或供应商行为。

新记录使用[验证模板](/zh-cn/reference/evidence/template/)。
[Plan.md](https://github.com/owent/llm-usage/blob/main/Plan.md)维护活动工作，
[最新验收索引](/zh-cn/reference/evidence/current-acceptance/)链接专项记录。
最后执行 `git diff --check`，同时检查未跟踪文件及 tracked diff。
