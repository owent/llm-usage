# session-error-zero-usage._expectations.md

Source: `<HOME>/.pi/agent/sessions/--C--Users-owt50--/2026-09-24T16-37-28-439Z_<UUID>.jsonl`.
Seven lines, Pi 0.87.1, created at 2026-09-24T16:37Z; extracted on 2026-09-25.
Sessions were empty (no_data) at M0. This recovery scan found the native session and
extracted permitted fields with anonymization.

<a id="结构期望"></a>

## Structural expectations

- All seven lines parse: parseErrors=0.
- Types: session ×1 (version=3), model_change ×1, thinking_level_change ×1,
  custom ×1, custom_message ×1, message ×2 (one system and one assistant).
- The first line is the session header; OMP starts with title instead.
- Assistant: stopReason="error", model=kimi-for-coding, provider=kimi-coding.
  All five usage fields and cost report zero for the actual failed call. The sample's
  token zeros are reported values. Reasoning is absent and stays unknown; no responseId,
  duration or ttft (Pi has no duration/ttft fields).

<a id="usage-数值期望允许字段原字段"></a>

## Expected usage values (selected original fields)

Exactly one primary model call:

| input | output | cacheRead | cacheWrite | totalTokens | reasoning | cost.total |
| --- | --- | --- | --- | --- | --- | --- |
| 0 | 0 | 0 | 0 | 0 | absent | 0 |

- error_status="error", schema_version="3".
- cost.total=0 maps to no cost, because zero is indistinguishable from missing prices.
- One imported event: input_total=0, output_total=0, cache_read=0, cache_write=0,
  total_tokens=0 (reported/derived zeros); output_reasoning unknown.

Seven anonymized IDs: anon-1…anon-7.
