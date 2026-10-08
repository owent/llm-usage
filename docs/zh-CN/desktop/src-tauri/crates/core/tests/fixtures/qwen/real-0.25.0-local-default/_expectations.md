# Qwen Code 0.25.0：默认自动记忆

<a id="qwen-code-0250-with-default-automatic-memory"></a>

2026-10-05 在 rootless Podman 中实际运行官方 npm 客户端，调用本地 llama.cpp 与
Qwen2.5-0.5B GGUF。JSONL 为真实 assistant 记录的字段提取：ID 匿名化、时间在同日
平移；移除正文、项目路径及其他非用量字段，token 数值保持原样。不是完整原始会话。
版本、镜像/模型校验和与独立统计见
[容器来源验收](../../../../../../../../docs/validation/desktop-usage/container-sources.md)。

- 主循环 1 次：prompt 10,226、output 2、cache read 0、total 10,228。
- CLI stats.bySource 另直报自动记忆提取 1 次、total 5,854；会话记录没有该逐次记录。
  CLI 总计 2 次 / 16,082 不作为此 fixture 的事件数或总量，不相减补造后台事件。
- schema_version 保留 0.25.0；原始本地模型名保留，canonical/provider 未知。
  未给出的缓存写入、reasoning 包含关系仍未知，不因 thoughts=0 补值。
- 二次扫描不新增、不更新，总计仍为 1 次 / 10,228。
