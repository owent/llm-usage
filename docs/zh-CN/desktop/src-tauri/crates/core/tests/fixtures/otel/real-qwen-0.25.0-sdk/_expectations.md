# Qwen Code 0.25.0 SDK file 真实脱敏样本

<a id="qwen-code-0250-sdk-file-anonymized-native-sample"></a>

官方 CLI、固定本地 llama.cpp / Qwen2.5 模型的无网络 Podman 会话，来源与版本见
[provenance.json](provenance.json)；原始 SHA-256 与 [容器来源记录](../../../../../../../../docs/validation/desktop-usage/container-sources.md)
一致。保留 SDK `_rawAttributes` 数组、`_spanContext`、时间、kind 与允许保留的属性；
会话及 trace/span ID 等值替换，不保留 prompt、响应正文、URL、账户或主机信息。
metrics 只保留非调用判别结构。此文件是 25 个连续多行 JSON 对象，不是 JSONL。

| 来源 | 次数 | 输入（含缓存） | 输出 | 缓存读 | thoughts | 总量 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| interaction 主循环 | 1 | 10,226 | 2 | 0 | 0 | 10,228 |
| standalone managed-auto-memory-extractor | 1 | 5,639 | 556 | 3 | 0 | 6,195 |
| CLI 全部请求 | 2 | 15,865 | 558 | 3 | 0 | 16,423 |

[native.jsonl](native.jsonl) 包含同一真实会话的首用户记录形状（message.parts 内容删除）
及唯一 assistant 用量记录，只有主循环 10,228 token；首记录用于核验手工根路由。
[cli-stats.json](cli-stats.json) 为 CLI 最终统计中的允许字段。log API response
与 llm_request span 是相同请求的两种观测，只采用 span；HTTP/interaction/metrics 不相加。
后台 span 没有 parentSpanContext；共用 session.id 不能据此核验子会话父子关系。

未返回的缓存写/provider 保持未知。样本 thoughts=0，不能据此核验非零 reasoning 的包含关系
或其他供应商/版本。在原生文件与导出文件之间按已核验主机/用户/会话/本地日择一；不按时间或 token
相等猜调用 ID，导出开启前的覆盖受限。边界、混合版本、错误与回滚控制是合成变体，
不能核验相应真实 SDK 场景。
