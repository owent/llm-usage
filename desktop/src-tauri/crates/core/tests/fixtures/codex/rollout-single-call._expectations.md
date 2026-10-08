# rollout-single-call._expectations.md

Source: `<HOME>/.codex/sessions/2026/09/24/rollout-<ts>-<UUID>.jsonl`, 183 KB,
17 lines, 2026-09-24T08:10Z. Extracted on 2026-09-24; the active session could append later.

<a id="结构期望"></a>

## Structural expectations

- All 17 lines parse; parseErrors=0.
- Counts: session_meta=1, turn_context=1, world_state=1, response_item/message=4,
  response_item/reasoning=1, response_item/compaction=1, token_usage_record=1;
  event_msg contains two thread_settings_applied and one each of task_started,
  user_message, agent_message, token_count and task_complete.
- session_meta.cli_version="0.155.0-alpha.16.3". Original originator=codex_vscode,
  anonymized to anon-N in the sample.
- turn_context.model=codex-auto-review: an auto-review sub-agent session, without
  a user-message-associated interactive model.
- Top-level timestamp uses ISO 8601 UTC with milliseconds, such as 2026-09-24T08:10:24.567Z.
  Some payload fields, including started_at/completed_at/create_time, use epoch seconds.

<a id="usage-数值期望白名单原字段"></a>

<a id="usage-数值期望提取的原字段"></a>

## Usage expectations for allowlisted native fields

Exactly one model call appears in three usage records, with identical values:

| Record | Path | input_tokens | cached_input_tokens | cache_write_input_tokens | output_tokens | reasoning_output_tokens | total_tokens |
| --- | --- | --- | --- | --- | --- | --- | --- |
| token_usage_record | payload.usage | 25558 | 4864 | 0 | 131 | 65 | 25689 |
| event_msg/token_count | payload.info.last_token_usage | 25558 | 4864 | 0 | 131 | 65 | 25689 |
| event_msg/token_count | payload.info.total_token_usage | 25558 | 4864 | 0 | 131 | 65 | 25689 |

Every relationship holds in this sample:

- total_tokens=input_tokens+output_tokens: 25689=25558+131.
- cached_input_tokens⊆input_tokens: 4864≤25558.
- reasoning_output_tokens⊆output_tokens: 65≤131.
- last==total in this one-call session; the cumulative snapshot equals the individual call.

There are 16 anonymized IDs, anon-1…anon-16.
