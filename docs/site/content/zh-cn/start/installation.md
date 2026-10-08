---
title: 安装与更新
description: 平台要求、安装包、数据位置及安全更新。
sidebar:
  order: 2
---

## 平台与安装包

Windows 11 x64 是首发桌面平台。发行包提供 Windows、Linux、macOS 各自 x64、arm64 的
`.tar.zst` 便携归档及 Windows x64 NSIS 安装包。Windows 使用系统 WebView2，
Linux 归档包含解压后的 AppDir，macOS 归档包含应用包；不包含 Debian 包。
跨平台构建成功本身不能据此确认该平台的
安装、桌面集成或 GUI 行为。

平台验证结果见[安装记录](/zh-cn/reference/evidence/installation-lifecycle/)和
[最新验收](/zh-cn/reference/evidence/current-acceptance/)。Linux 容器 GUI 与 Windows
原生验收的范围分别记录，WSL 编译是另一类结果。

## 下载

推送 tag 会准备 [GitHub Release 草稿](https://github.com/owent/llm-usage/releases)，
维护者公开发布前，草稿仅对仓库协作者可见。
也可在[应用构建流水线](https://github.com/owent/llm-usage/actions/workflows/ci.yml)下载：
打开成功的运行，核对提交及软件包报告，下载对应平台的制品。下载流水线制品需要登录 GitHub。
也可以[自行构建应用](/zh-cn/development/setup/)。未签名的 pre-alpha 制品不等于已签名的正式发行。

便携包请按系统和 CPU 架构选择，使用新版 7-Zip 或支持 zstd 的 tar 完整解压
`LLMUsage-<version>-<os>-<arch>-portable.tar.zst`。Windows 打开 `LLMUsage.exe`，
Linux 运行 `./AppRun`，macOS 打开 `LLMUsage.app`。
Linux 无须 FUSE，但需要兼容的桌面/显示环境及 glibc 2.35 或更高版本（Ubuntu 22.04 构建基线）。
Windows 11 通常自带 WebView2，便携包不内嵌该运行时；macOS 下载的应用仍需按 Gatekeeper 流程批准。
系统条件详见包内中英文 `README.txt`。

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

便携包继续使用相同的用户数据和系统凭据，移动程序目录不会迁移数据。
移动或删除目录前请停止程序、停用自有计划任务。更新时解压到新目录、保留数据库备份，
需要时以新的路径重新启用任务。

开发者验证安装包时应使用专用的隔离生命周期脚本。前置条件和清理边界见
[安装要求](/zh-cn/reference/design/installation-lifecycle/)。
