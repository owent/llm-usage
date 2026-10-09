---
title: 打包、CI 与发行
description: 保留 revision 依据，区分构建和签名分发。
sidebar:
  order: 5
---

既有应用 CI 包含 Markdown、前端、Rust 和平台 release 构建任务。平台矩阵保留
Windows、Linux、macOS，使用 `fail-fast: false`。包报告记录 revision、大小和 SHA-256，
请检查真实制品，不假定绿色任务产出了目标安装包。

<a id="本机构建"></a>

## 自行构建

```powershell
npm run pack:desktop
```

先检查资源和前端类型，再执行 Tauri release 构建；不签名、公证、发布 Release，也不能据此确认
安装或真实供应商行为。包产物按平台位于 `desktop/src-tauri/target/release/bundle/`。

改版本时同步根目录/desktop npm manifests 和锁文件、Tauri 配置、Rust manifests 及 Cargo.lock。
便携打包脚本会拒绝版本不一致或不等于 `v<version>` 的 tag；恢复锁文件和真实 LFS 资源。
原生要求见[平台要求](/zh-cn/reference/design/platform-ci/)及
[安装要求](/zh-cn/reference/design/installation-lifecycle/)。

## 发布边界

推送 tag 会触发应用 CI，在全部检查和各平台构建成功后发布 Draft Release。
草稿包含 Windows、Linux 和 macOS 各自 x64、arm64 的六种 `.tar.zst` 便携归档，
现有 Windows x64 NSIS 安装包，以及每个系统/架构的大小/SHA-256 报告。
不上传 Debian 包或原始 AppImage。包报告必须匹配 tag 对应的源码 revision。
发行作业核对上传后的大小和摘要，通过固定提交版本的
[xresloader/upload-to-github-release](https://github.com/xresloader/upload-to-github-release)
使用 `overwrite: true` 上传。

重建 tag 或重新运行对应流水线会更新同一草稿的名称、正文和目标提交，并替换同名附件。
同一 tag 的发布串行执行；tag 已指向其他提交时，旧构建拒绝发布。Release 保持草稿状态，
检查后再公开发布。这些包不能建立签名、公证或安装验收结论。

## 便携包

选择对应系统的 `LLMUsage-<version>-<windows|linux|macos>-<x64|arm64>-portable.tar.zst`。
使用支持 zstd 的工具完整解压目录，例如 Windows 上的新版 7-Zip，
或支持时使用 `tar --zstd -xf <archive>`。
在解压后的 Windows、Linux、macOS 目录分别运行 `LLMUsage.exe`、`AppRun`、`LLMUsage.app`，
保留全部资源。每个归档包含完整中英文运行说明。

新归档还包含 `llmusage-package.json`，标识版本、平台、架构、原生可执行文件、启动
入口和受管理的顶层文件。更新器核对该身份后选择同格式的便携包；请将清单与包放在
一起。没有清单的旧归档需要手动替换一次，才能采用本更新器。

Windows 需要系统 WebView2，Windows 11 通常自带；本包不包含固定版本离线运行时。
Linux 包含 AppDir 依赖库，无须 FUSE，使用 Ubuntu 22.04/glibc 2.35 基线，仍需要兼容的桌面和显示环境。
可选凭据集成使用系统 Secret Service/D-Bus。macOS 使用系统 WebKit，下载的应用仍受 Gatekeeper 管理。
其他 Linux 发行版和桌面环境需要独立验证。数据和凭据仍使用原有的用户目录/系统存储；
移动本目录不会迁移它们，移动前请停用应用创建的计划任务。

打包脚本先生成 tar，再使用 `zstd -19 -T2 --long=27` 压缩，保留全部资源。
核对压缩完整性、全新解压目录的文件摘要/链接/权限和二进制 CPU 类型，
并在全部六种原生 runner 上运行解压后真实程序的隔离无界面/SQLite 回归。
随后才记录用于发布的大小和 SHA-256。

应用检查 GitHub 公开的最新稳定版，核对对应资产的 API 大小和 SHA-256。保留完整的
便携包及 NSIS 文件名，公开发行前上传全部对应资产，并保留 API 摘要。更新器信任
HTTPS 和仓库发行管理权限；摘要不是发布者签名。草稿、预发行、相同或较旧版本
不会提供更新。开发构建和身份未识别的程序可以检查版本，但不能选择、下载或安装
更新包。见[更新设计](/zh-cn/reference/design/application-updates/)及
[验证记录](/zh-cn/reference/evidence/application-updates/)。构建后运行
`npm run test:update:windows`，使用合成更新检查真实便携 IPC/辅助进程替换；该检查
不能建立公开发行升级或 NSIS 生命周期验收。

GitHub 元数据请求失败后，会从文档站点 `/updates/latest.json` 再尝试一次。
快照保留完整的更新包 URL、大小和 SHA-256 摘要；下载文件仍来自 GitHub。
客户端拒绝超过七天或生成时间晚于当前时间五分钟以上的快照。文档 CI 在 main 构建时
以及每天 UTC 04:17（北京时间 12:17）刷新。Release 发布、编辑、撤回或删除时，
会触发 main 分支的文档构建，保留 Pages 环境的分支限制。PR 构建使用已提交的快照，
无须联网刷新。生成器要求七个更新包均已上传；复用有效快照时不修改原始生成时间。
本地可运行 `node docs/site/scripts/update-feed.mjs` 刷新，结果仅保存在忽略的根目录
`build/` 下。构建后运行 `npm run test:update:check`，检查隔离 Windows 程序的真实
IPC 版本查询；带有内嵌前端的调试构建使用
`node desktop/tests/native-update-check.mjs --exe desktop/src-tauri/target/debug/LLMUsage.exe --development`。
详见[检查更新修复记录](/zh-cn/reference/evidence/update-check-repair/)。

构建与发行授权分开，设计计划不提供推送、部署、签名或外部凭据修改授权。
需要最终批准时，先完成可审阅产物、测试、身份检查和回滚方案。

当前文档发布已有用户明确授权，并使用专用流水线，将静态站发布到 `gh-pages` 并完成
Pages 部署。应用草稿发布已有独立的明确授权，写权限仅用于 tag 发行作业；PR 检查保持只读。
参阅[文档维护](/zh-cn/development/documentation/)。

记录受测完整 revision、实际任务及制品哈希。后续仅文档提交引用先前源码构建时须明确
代码/流水线范围，不能声称 CI 在另一个 revision 上运行过。
