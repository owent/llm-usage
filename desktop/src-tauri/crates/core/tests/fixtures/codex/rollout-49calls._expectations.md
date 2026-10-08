# rollout-49calls._expectations.md

Source: `<HOME>/.codex/sessions/2026/09/24/rollout-2026-09-24T16-12-01-<UUID>.jsonl`,
approximately 1.2 MB and 545 lines. Extracted on 2026-09-24.

<a id="结构期望"></a>

## Structural expectations

- All 545 lines parse. cli_version=0.155.0-alpha.16.3;
  originator=codex_vscode, anonymized in the test sample.
- Record counts: response_item/message=101, event_msg/thread_settings_applied=50,
  event_msg/task_started=49, turn_context=49, event_msg/user_message=49,
  event_msg/agent_message=49, token_usage_record=49, event_msg/token_count=49,
  event_msg/task_complete=49, response_item/reasoning=48, session_meta=1,
  response_item/compaction=1, world_state=1.
- 49 model calls, equal to the token_usage_record count; all models are codex-auto-review.

<a id="usage-数值期望"></a>

## Usage expectations

Sum the per-call token_usage_record.payload.usage or token_count last_token_usage:

| Field | Sum |
| --- | --- |
| input_tokens | 4,184,537 |
| cached_input_tokens | 4,022,016 |
| cache_write_input_tokens | 0 |
| output_tokens | 4,051 |
| reasoning_output_tokens | 725 |
| total_tokens | 4,188,588 |

- All 98 per-call records satisfy total=input+output.
- The final token_count total_token_usage snapshot matches every summed per-call field.
- Cache-read/input ratio: 4,022,016 / 4,184,537 ≈ 96.1%, reflecting the auto-review loop.

total_token_usage is cumulative. Summing all 49 snapshots, yielding 81,558,231 input,
is incorrect. Use the final value or sum per-call last_token_usage/token_usage_record.
