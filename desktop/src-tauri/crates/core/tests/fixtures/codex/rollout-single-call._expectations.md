# rollout-single-call._expectations.md

来源：`<HOME>/.codex/sessions/2026/09/24/rollout-<ts>-<UUID>.jsonl`（183 KB，17 行，2026-09-24T08:10Z）。
提取时间：2026-09-24；文件此后可能继续追加（会话活跃）。

## 结构期望

- 17 行全部可解析（parseErrors=0）。
- 记录类型计数：session_meta ×1、turn_context ×1、world_state ×1、
  response_item/message ×4、response_item/reasoning ×1、response_item/compaction ×1、
  event_msg/{thread_settings_applied ×2, task_started, user_message, agent_message, token_count, task_complete}、
  token_usage_record ×1。
- session_meta：`cli_version="0.155.0-alpha.16.3"`，originator 原值为 `codex_vscode`（fixture 中已匿名为 anon-N）。
- turn_context.model = `codex-auto-review`（auto-review 子代理会话，无用户消息对应的真实交互模型）。
- 顶层 `timestamp` 为 ISO 8601 毫秒 UTC（`2026-09-24T08:10:24.567Z`）；
  部分 payload 内含 epoch 秒数值字段（如 started_at/completed_at/create_time）。

## usage 数值期望（白名单原字段）

恰有 1 次模型调用，usage 以三种记录出现，数值一致：

| 记录 | 路径 | input_tokens | cached_input_tokens | cache_write_input_tokens | output_tokens | reasoning_output_tokens | total_tokens |
| --- | --- | --- | --- | --- | --- | --- | --- |
| token_usage_record | payload.usage | 25558 | 4864 | 0 | 131 | 65 | 25689 |
| event_msg/token_count | payload.info.last_token_usage | 25558 | 4864 | 0 | 131 | 65 | 25689 |
| event_msg/token_count | payload.info.total_token_usage | 25558 | 4864 | 0 | 131 | 65 | 25689 |

包含关系（本样本全部成立）：

- total_tokens = input_tokens + output_tokens（25689 = 25558 + 131）
- cached_input_tokens ⊆ input_tokens（4864 ≤ 25558）
- reasoning_output_tokens ⊆ output_tokens（65 ≤ 131）
- 单次调用会话中 last == total（累计快照恰等于单次值）

匿名 ID：16 个（anon-1…anon-16）。
