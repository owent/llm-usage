---
title: 安装与更新
description: 平台要求、安装包、数据位置及安全更新。
sidebar:
  order: 2
---

## 平台与安装包

发行包提供 Windows、Linux、macOS 各自 x64、arm64 的
`.tar.zst` 便携归档及 Windows x64 NSIS 安装包。Windows 使用系统 WebView2，
Linux 归档包含解压后的 AppDir，macOS 归档包含应用包。

## 下载

从[最新 Release](https://github.com/owent/llm-usage/releases/latest) 下载适合系统和 CPU 架构的软件包。
Windows x64 用户也可选择 `*-setup.exe` 安装器。

便携包请按系统和 CPU 架构选择，使用新版 7-Zip 或支持 zstd 的 tar 完整解压
`LLMUsage-<version>-<os>-<arch>-portable.tar.zst`。Windows 打开 `LLMUsage.exe`，
Linux 运行 `./AppRun`，macOS 打开 `LLMUsage.app`。
Linux 无须 FUSE，但需要兼容的桌面/显示环境及 glibc 2.35 或更高版本（Ubuntu 22.04 构建基线）。
Windows 11 通常自带 WebView2，便携包不内嵌该运行时；macOS 下载的应用仍需按 Gatekeeper 流程批准。
系统条件详见包内中英文 `README.txt`。

## 应用数据

存在 `APPDATA` 时，应用数据保存在 `%APPDATA%\llm-usage-desktop\llm-usage.sqlite`。
否则，存在用户主目录时使用 `~/.local/share/llm-usage-desktop/llm-usage.sqlite`。

可以启动时指定独立应用数据目录：

```powershell
LLMUsage.exe --data-dir C:\UsageData
```

目录必须是绝对路径。这只改变应用存储，不改变 Agent 来源发现范围。如需主动限定采集范围，
请在设置中开启**仅扫描手工目录**并配置来源根。

## 升级、回滚和卸载

在**设置 > 软件更新**中选择每次启动、每天、每周或仅手动检查。每天和每周按距离
上次成功检查的时间计算，仅在应用运行时检查。自动下载默认关闭，始终支持手动检查；
保存设置后生效。全局通知条和设置页显示下载进度、错误及取消操作。下载验证通过后，
安装版点击**安装更新**，便携版点击**重启并更新**。

更新保持系统、架构和包类型。Windows 安装版下载 NSIS 安装包，便携版下载对应便携
归档。便携更新保留原可执行文件路径，在 `.llmusage-update/backup` 保留备份，并保留
包根目录中的无关文件。请将个人文件放在包管理的子目录之外。新便携归档包含
`llmusage-package.json`，请保留完整包。没有可验证包身份的独立二进制不能选择自动
更新。采集或维护运行时暂不能安装更新；替换失败恢复原文件，中断的便携更新在下次
GUI 启动时恢复。
如果中断后 Windows 主程序缺失，打开保留的 `.llmusage-update/helper.exe` 恢复并重启。

检查访问公开 GitHub 发行服务，不发送本地用量。下载要求大小和 SHA-256 精确匹配；
这核对 HTTPS GitHub 发行资产，不提供独立发布者签名。原生更新结果和平台限制见
[更新验证](/zh-cn/reference/evidence/application-updates/)。

升级或回滚前保留一致数据库备份。SQLite 迁移及备份由后端实现，旧程序不一定支持新 schema。
请保留与计划运行版本兼容的备份。

Windows 卸载默认保留用量和设置，删除本次安装创建的启动项和计划任务。
升级保留已开启的后台设置并更新可执行文件路径。卸载不会恢复无关 IDE 配置，也不删除其他安装的数据。

便携包继续使用相同的用户数据和系统凭据，移动程序目录不会迁移数据。
移动或删除目录前请停止程序、停用自有计划任务。手动替换时将新完整包解压到新目录，
保留数据库备份，需要时以新的路径重新启用任务。应用内便携更新保留既有路径。
