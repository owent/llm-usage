# Claude Code 2.1.197 真实用量提取字段

<a id="claude-code-21197-真实用量白名单"></a>

<a id="claude-code-21197-native-usage-sample"></a>

2026-10-07，Debian/rootless Podman、智谱 Coding Plan Anthropic 兼容端点。
两次请求的 HTTP/SSE usage、CLI 结果与原生 JSONL 逐项核对，正文剔除、稳定 ID
哈希化；provenance 保留原始文件 SHA-256。两个 JSONL 各 6 行、两条 assistant
共享 message.id，requestId 缺失，按消息 ID 去重，不按内容块计调用。

| 模型 | 去重调用 | 非缓存输入 | 输出 |
| --- | --- | --- | --- |
| glm-5.3-flash | 1 | 1,339 | 33 |
| glm-5.3 | 1 | 1,338 | 23 |

缓存创建未由供应商报告，客户端仍写零；原生零桶没有有效性标记，缓存读/写、完整
输入/总量与 reasoning 未知。provider/费用不由协议或模型名推断，CLI 费用仅估值。
事件逐条 version=2.1.197、known_version；其他版本兼容，不能据此确认历史。

完整记录见 [容器验收](../../../../../../../../docs/validation/desktop-usage/claude-container-sample.md)。
