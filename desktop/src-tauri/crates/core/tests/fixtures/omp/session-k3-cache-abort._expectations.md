# session-k3-cache-abort._expectations.md

Source: `<HOME>/.omp/agent/sessions/--D--workspace-projs-test-minimind--/2026-09-15T17-33-37-534Z_<UUID>.jsonl`.
31 lines, oh-my-pi 18.2.7, 2026-09-15T17:33Z; extracted 2026-09-25.

<a id="结构期望"></a>

## Structural expectations

- All 31 lines parse: parseErrors=0.
- Types: title ×1, session ×1 (version=3), model_change ×1, thinking_level_change ×1,
  title_change ×1, credential_pin ×1, custom ×9 (eight tool_execution_start, one session_exit),
  message ×16 (two user, seven assistant, seven toolResult).
- All assistants use model=k3-256k and provider=kimi-code; six toolUse and one aborted stopReason.
- All assistants have duration/ttft and responseId. toolResult has no usage, also true
  across all 6,643 globally checked toolResult records.

<a id="usage-数值期望允许字段原字段jq-逐条求和"></a>

## Expected usage values (selected original fields, summed individually with jq)

Seven primary model calls:

| # | stopReason | input | output | cacheRead | cacheWrite | totalTokens |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | toolUse | 140 | 385 | 17408 | 0 | 17933 |
| 2 | toolUse | 995 | 1152 | 17408 | 0 | 19555 |
| 3 | toolUse | 1593 | 483 | 18176 | 0 | 20252 |
| 4 | toolUse | 15051 | 2225 | 19712 | 0 | 36988 |
| 5 | toolUse | 2470 | 177 | 34560 | 0 | 37207 |
| 6 | toolUse | 380 | 406 | 36864 | 0 | 37650 |
| 7 | aborted | 683 | 0 | 37120 | 0 | 37803 |
| Σ | | 21312 | 4828 | 181248 | 0 | 207388 |

- Every totalTokens=input+output+cacheRead+cacheWrite; reasoningTokens absent.
- Record 7 has error_status="aborted"; output=0 is reported.
- Seven primary events: input_total=202560 (derived: 21312+181248+0),
  input_uncached=21312, cache_read=181248, output_total=4828, total_tokens=207388;
  cacheWrite sum=0 (reported).

37 anonymized IDs: anon-1…anon-37.
