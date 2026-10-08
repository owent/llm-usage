---
title: 安装与更新
description: 平台要求、安装包、数据位置及安全更新。
sidebar:
  order: 2
---

## 平台与安装包

Windows 11 x64 是首发桌面平台。Windows 使用 NSIS 安装包和系统 WebView2 运行时。
Linux 构建产出 deb、AppImage，macOS 构建产出 app。跨平台构建成功本身不能据此确认该平台的
安装、桌面集成或 GUI 行为。

仓库目前没有已发布的 GitHub Release 条目。请使用
[GitHub Actions 构建制品](https://github.com/owent/llm-usage/actions)，核对受测 revision
及包报告，或[自行构建应用](/zh-cn/development/setup/)。未签名的 pre-alpha 制品不等于已签名的正式发行。

平台验证结果见[安装记录](/zh-cn/reference/evidence/installation-lifecycle/)和
[最新验收](/zh-cn/reference/evidence/current-acceptance/)。Linux 容器 GUI 与 Windows
原生验收的范围分别记录，WSL 编译是另一类结果。

## 应用数据

存在 `APPDATA` 时，后端当前使用 `%APPDATA%\llm-usage-desktop\llm-usage.sqlite`。
否则，存在用户主目录时使用 `~/.local/share/llm-usage-desktop/llm-usage.sqlite`。
应用 `db_path()` 函数是路径行为的依据。

可以启动时指定独立应用数据目录：

```powershell
LLMUsage.exe --data-dir C:\UsageData
```

目录必须是绝对路径。这只改变应用存储，不改变 Agent 来源发现范围。如需主动限定采集范围，
请在设置中开启**仅扫描手工目录**并配置来源根。

## 升级、回滚和卸载

升级或回滚前保留一致数据库备份。SQLite 迁移及备份由后端实现，旧程序不一定支持新 schema。
不要删除数据库来让回滚看似成功。

Windows 卸载默认保留用量和设置，仅删除已经证明属于本次安装的启动项和系统任务。
升级保留已授权的后台行为并核对可执行文件路径。卸载不会恢复无关 IDE 配置，也不删除其他安装的数据。

开发者验证安装包时应使用专用的隔离生命周期脚本。前置条件和清理边界见
[安装要求](/zh-cn/reference/design/installation-lifecycle/)。
