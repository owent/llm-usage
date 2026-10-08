# Copilot CLI assistant_usage_events 真实脱敏样本预期值

<a id="copilot-cli-assistant_usage_events-native-sample-expectations"></a>

来源：本机 `~/.copilot/session-store.db`（schema_version=8）只读提取，
36 行（2026-08-03T14:25:56Z–14:36:22Z，单会话单模型 claude-opus-4.8，
request_multiplier 恒 27.0）。脱敏：session_id/agent_id → syn-N（agent_id 实际
全 NULL）；只提取允许保留的列，不含 token_details_json/api_endpoint 等正文列。

人工核算（全 36 行求和）：

| 字段 | 值 |
| --- | --- |
| input_tokens | 4,649,981 |
| output_tokens | 34,157 |
| cache_read_tokens | 4,416,791 |
| cache_write_tokens | 233,118 |
| reasoning_tokens | 17,678 |
| input_uncached（派生 = input−read−write） | 72 |
| total_tokens（派生 = input+output） | 4,684,138 |

语义核验：36/36 行满足 input >= cache_read + cache_write
（input = 未缓存 + 读 + 写，M0 m0-agent-fixtures.md 结论在真实数据复核成立）。
request_multiplier=27.0 是 premium 付费倍率，不进 token 统计（不入账）。
