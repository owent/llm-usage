# 桌面客户端验证记录

<a id="desktop-validation-records"></a>

[最新验收](current-acceptance.md)汇总受测版本、平台结果及来源记录；
[Plan.md](../../../Plan.md)统一维护未完成任务与条件，
[V01–V31](../../design/desktop-usage/validation.md)保留验收标准。
本目录按阶段保存当时的执行记录，旧记录中的待办不代表当前状态。

| 记录类别 | 入口 |
| --- | --- |
| 本批三平台 CI 与下载制品 | [CI 作业、受测提交、包摘要及首次失败](ci-plan-validation.md) |
| Windows/Linux 安装与实际 GUI | [NSIS/deb/AppImage/FUSE 生命周期](installation-lifecycle.md)、[Orca 十语言](orca-multilang.md) |
| 系统凭据、原生交互及规模 | [跨平台凭据](platform-auth-continuation.md)、[原生/取消/增量](plan-finalization.md)、[查询与资源](plan-execution.md) |
| 真实来源与字段修正 | [最新来源索引](current-acceptance.md#来源与功能记录)、[Qwen/OpenCode](container-sources.md)、[M8 十源](m8-container-samples.md)、[MiMo/Zoo/DSH](m3-container-samples.md) |
| 设计审查与资源 | [初期审查](review-2026-09-27.md)、[M0/M1 审查](m0-m1-review.md)、[静态资源](static-assets.md) |

记录规则见 [交付要求](../../design/desktop-usage/execution.md)，模板见 [TEMPLATE.md](TEMPLATE.md)：

- 完成实际检查并记录结果后才写通过；保留 cwd、命令、版本、环境、退出码、数量、首次失败、恢复及未执行项。
- 静态、合成、真实来源、浏览器模拟 IPC、原生桌面、WSL/容器与 CI 分项报告，不能相互替代。
- 原始私人数据、核对库、提取脚本和日志只放根 build/；公开记录及测试数据须脱敏，不复制正文、凭据或私人路径。
- 已完成细节归专项记录；活动计划保留稳定 ID、当前状态、剩余条件和记录链接，不重复累计测试数字或逐轮历史。
