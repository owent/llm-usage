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

改版本时同步 desktop package、Tauri 配置及 Rust manifests，恢复锁文件和真实 LFS 资源。
原生要求见[平台要求](/zh-cn/reference/design/platform-ci/)及
[安装要求](/zh-cn/reference/design/installation-lifecycle/)。

## 发布边界

推送 tag 会触发应用 CI，在全部检查和各平台构建成功后发布 Draft Release。
草稿包含 Windows x64 NSIS 安装包、Linux x64 Debian/AppImage 包、macOS arm64
`.app.tar.gz`，以及每个平台的大小/SHA-256 报告。包报告必须匹配 tag 对应的源码 revision。
发行作业核对上传后的大小和摘要，通过固定提交版本的
[xresloader/upload-to-github-release](https://github.com/xresloader/upload-to-github-release)
使用 `overwrite: true` 上传。

重建 tag 或重新运行对应流水线会更新同一草稿的名称、正文和目标提交，并替换同名附件。
同一 tag 的发布串行执行；tag 已指向其他提交时，旧构建拒绝发布。Release 保持草稿状态，
检查后再公开发布。这些包不能建立签名、公证或安装验收结论。

构建与发行授权分开，设计计划不提供推送、部署、签名或外部凭据修改授权。
需要最终批准时，先完成可审阅产物、测试、身份检查和回滚方案。

当前文档发布已有用户明确授权，并使用专用流水线，将静态站发布到 `gh-pages` 并完成
Pages 部署。应用草稿发布已有独立的明确授权，写权限仅用于 tag 发行作业；PR 检查保持只读。
参阅[文档维护](/zh-cn/development/documentation/)。

记录受测完整 revision、实际任务及制品哈希。后续仅文档提交引用先前源码构建时须明确
代码/流水线范围，不能声称 CI 在另一个 revision 上运行过。
