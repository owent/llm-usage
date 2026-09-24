# rollout-49calls._expectations.md

来源：`<HOME>/.codex/sessions/2026/09/24/rollout-2026-09-24T16-12-01-<UUID>.jsonl`（约 1.2 MB，545 行）。
提取时间：2026-09-24。

## 结构期望

- 545 行全部可解析。cli_version=0.155.0-alpha.16.3，originator=codex_vscode（fixture 中匿名）。
- 记录类型计数：response_item/message ×101、event_msg/thread_settings_applied ×50、
  event_msg/task_started ×49、turn_context ×49、event_msg/user_message ×49、
  event_msg/agent_message ×49、token_usage_record ×49、event_msg/token_count ×49、
  event_msg/task_complete ×49、response_item/reasoning ×48、session_meta ×1、
  response_item/compaction ×1、world_state ×1。
- 49 次模型调用（token_usage_record 计数即调用数）；模型均为 codex-auto-review。

## usage 数值期望

逐次记录（token_usage_record.payload.usage / token_count 的 last_token_usage）合计：

| 字段 | 合计 |
| --- | --- |
| input_tokens | 4,184,537 |
| cached_input_tokens | 4,022,016 |
| cache_write_input_tokens | 0 |
| output_tokens | 4,051 |
| reasoning_output_tokens | 725 |
| total_tokens | 4,188,588 |

- 全部 98 条逐次记录满足 total = input + output（98/98）。
- 最后一条 token_count 的 total_token_usage（累计快照）= 上表逐次合计，逐字段相等。
- 缓存读取占输入比 = 4,022,016 / 4,184,537 ≈ 96.1%（auto-review 循环高命中）。

注意：total_token_usage 是累计快照，49 条快照求和（81,558,231 input）是错误算法，
只能取最终值或对 last_token_usage/token_usage_record 逐条求和。
