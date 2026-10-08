# rollout-multimodel-full._expectations.md

Source: `<HOME>/.codex/sessions/2026/09/23/rollout-2026-09-23T23-27-28-<UUID>.jsonl`,
approximately 6.7 MB and 1457 lines. Extracted on 2026-09-24. The complete session
includes multiple models, compaction and an aborted turn.

<a id="结构期望"></a>

## Structural expectations

- All 1457 lines parse; cli_version=0.155.0-alpha.16.3, multi_agent_version=v2.
- Top-level counts: response_item=709 (reasoning 229, custom_tool_call 214,
  custom_tool_call_output 214, message 52); event_msg=511 (item_completed 268,
  token_count 222, thread_settings_applied 9, task_started 6, task_complete 5,
  turn_aborted 1); token_usage_record=221, turn_context=8, world_state=5,
  compacted=2, session_meta=1.
- turn_context models change from gpt-6-astra at lines 8/397/782/817 to gpt-6-sol
  at lines 1019/1136/1301/1445. Usage records have no model field; attribute them
  using the corresponding turn_context position.

<a id="usage-数值期望"></a>

## Usage expectations

Totals across 221 individual token_usage_record records:

| Field | Sum |
| --- | --- |
| input_tokens | 32,061,426 |
| cached_input_tokens | 31,055,872 |
| cache_write_input_tokens | 0 |
| output_tokens | 143,381 |
| reasoning_output_tokens | 45,498 |
| total_tokens | 32,204,807 |

- Every record satisfies total=input+output.
- **Compaction resets cumulative snapshots.** The final token_count.total_token_usage is
  {input 31,590,140, cached 30,596,992, cache_write 0, output 134,195, reasoning 45,498,
  total 31,724,335}, differing from the per-call sum by approximately 470,000 input.
  Two compacted records contain payload.latest_token_usage_record.usage:
  line 395 has total=250,108; line 815 has total=230,364.
  Sum individual token_usage_record for real Codex totals; cumulative differences
  must be segmented at reset boundaries.
- One turn_aborted record identifies an aborted turn. Its returned partial usage still counts.
