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

Windows 需要系统 WebView2，Windows 11 通常自带；本包不包含固定版本离线运行时。
Linux 包含 AppDir 依赖库，无须 FUSE，使用 Ubuntu 22.04/glibc 2.35 基线，仍需要兼容的桌面和显示环境。
可选凭据集成使用系统 Secret Service/D-Bus。macOS 使用系统 WebKit，下载的应用仍受 Gatekeeper 管理。
其他 Linux 发行版和桌面环境需要独立验证。数据和凭据仍使用原有的用户目录/系统存储；
移动本目录不会迁移它们，移动前请停用应用创建的计划任务。

打包脚本先生成 tar，再使用 `zstd -19 -T2 --long=27` 压缩，保留全部资源。
核对压缩完整性、全新解压目录的文件摘要/链接/权限和二进制 CPU 类型，
并在全部六种原生 runner 上运行解压后真实程序的隔离无界面/SQLite 回归。
随后才记录用于发布的大小和 SHA-256。

构建与发行授权分开，设计计划不提供推送、部署、签名或外部凭据修改授权。
需要最终批准时，先完成可审阅产物、测试、身份检查和回滚方案。

当前文档发布已有用户明确授权，并使用专用流水线，将静态站发布到 `gh-pages` 并完成
Pages 部署。应用草稿发布已有独立的明确授权，写权限仅用于 tag 发行作业；PR 检查保持只读。
参阅[文档维护](/zh-cn/development/documentation/)。

记录受测完整 revision、实际任务及制品哈希。后续仅文档提交引用先前源码构建时须明确
代码/流水线范围，不能声称 CI 在另一个 revision 上运行过。
