---
title: 架构与数据流
description: 模块边界、单写者、IPC 和持久化依据。
sidebar:
  order: 2
---

桌面程序使用 Tauri 2、Rust 后端、SQLite 及 Svelte/TypeScript 前端。
ECharts 按需加载并使用 SVG 渲染。Node 仅用于构建，历史 Python 原型不作为运行时 sidecar。

## 模块映射

| 位置 | 职责 |
| --- | --- |
| `desktop/src-tauri/crates/core/src/adapters/` | 产品独立发现、格式/版本探测、有界解析和字段映射。 |
| `desktop/src-tauri/crates/core/` | 共享领域规则、存储、迁移、去重、查询、归档和价格。 |
| `desktop/src-tauri/src/` | Tauri IPC、应用状态、调度、进程归属、系统任务、遥测和凭据集成。 |
| `desktop/src/` | 五页 Svelte UI、类型 API、本地语言目录、图表及交互逻辑。 |
| `desktop/tests/` | 浏览器、真实程序、原生、规模、安装及凭据测试。 |
| `docs/design/desktop-usage/` | 应用主要设计说明及已核验来源调研。 |
| `docs/validation/desktop-usage/` | 命令、环境、首次失败和验证范围。 |

## 持久化采集

发现确定产品、物理文件归属和本机归属。适配器有界读取，产生标准化事件或独立累计快照。
SQLite 单写者在同一事务提交观测、修订、诊断、指纹、generation、处理位置和符合条件的汇总。

两个来源实例可并行读取。失败或中断不阻断其他有效来源，不推进未确认游标/期限。
重启恢复和重复读取不重复入库。解析修正比较完整旧摘要、保留真实冲突，并按字段规则重新检查未变旧文件。

## 前端与 IPC

UI 接收带质量计数、范围、状态和数据修订的类型 DTO。大 token 通过十进制字符串跨 IPC，
保留超过 JavaScript 安全整数范围的数值。图表可缩放展示，表格/导出保留精确量。

前端不读取任意文件、执行 SQL、访问 Agent 凭据或启动 shell。后端筛选和分页有界。
可选 HTTP 接收共享已校验采集边界，保持本机监听和认证。

修改前请阅读[架构说明](/zh-cn/reference/design/architecture/)、
[数据规则](/zh-cn/reference/design/data-contract/)和
[查询加速说明](/zh-cn/reference/design/query-acceleration/)。
