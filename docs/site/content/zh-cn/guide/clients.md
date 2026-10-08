---
title: 客户端支持
description: 支持的客户端、本地记录格式及版本限制。
sidebar:
  order: 3
---

[接入矩阵](/zh-cn/reference/design/adapters/)列出完整支持范围，包括产品版本、本地文件或数据库、
可用字段及支持状态。请核对具体客户端和版本；使用相同内核或模型名并不保证记录格式兼容。

## 常见类别

| 客户端类别 | 采集方式及适用范围 |
| --- | --- |
| Codex | 原生 rollout JSONL 和已核验用量记录；有界回填保留游标并轮转文件访问。 |
| Claude Code | 原生消息和逐记录版本，多内容块按消息身份去重。 |
| OpenCode 及衍生产品 | 产品独立记录和 schema；共享内核不能据此确认其他产品的映射。 |
| Gemini、Qwen、Pi、Oh My Pi | 独立适配器和逐版本规则，不盲目将缺项或默认零视为已报告用量。 |
| Cline、Roo | 独立核验的 VS Code 记录格式；Cline SDK metrics 可能覆盖多个请求。 |
| Zed、Junie | 参阅已测供应商配置和原生文件的测试记录，托管服务及未测供应商方式另行核验。 |
| 本地 OTel 导出 | 仅允许已核验发送端、记录格式和本地归属；接收器不是通用 Collector。 |
| 其他已注册客户端 | 查看完整矩阵，包含文档级实施、探针和排除路线。 |

## GitHub Copilot 客户端

- **CLI：** 旧 `assistant_usage_events` 与新版 chronicle 能力不同，不重建已经缺失的逐次 token。
- **VS Code：** `copilot_chat` 读取原生 `chatSessions/*.jsonl`；已核验本地 OTel file
  可替代符合条件的原生贡献，避免重复计算。
- **Visual Studio：** `vs_copilot` 读取 `%TEMP%`/`%TMP%` 下已经核验的
  `VSGitHubCopilotLogs/traces` 遥测文件，临时文件不承诺完整历史。
- **账户额度：** 客户端保存的 `copilot-user-cache.json` 提供独立额度快照，premium 请求和 credit 不折算为 token。

Visual Studio 发现不筛选 Community、Professional 或 Enterprise，也不猜安装路径。
只读核查工具使用微软 `vswhere` 查询所有产品、版本及预览实例。已核验的 VS 18 exporter
不同于受查 VS 2022 和旧扩展制品。没有该 exporter 的版本不能靠补猜年份/SKU 目录解决。

```powershell
pwsh -NoProfile -File desktop/scripts/inspect-vs-copilot.ps1
```

解释空结果前请阅读[Visual Studio 跨版本分析](/zh-cn/reference/evidence/m9-vs-copilot-discovery/)
和[Copilot 审查](/zh-cn/reference/evidence/m9-copilot-review/)。

<a id="验证范围"></a>

## 版本与格式限制

已有解析器仍可能缺少特定版本或格式的原生用量样本，矩阵将这些情况与已通过原生记录核对的格式区分。
“未核验”不等于“不支持”；仅有已安装客户端或空会话还不能确认 token 采集。
详细结果及剩余限制见[验收记录](/zh-cn/reference/evidence/current-acceptance/)。
