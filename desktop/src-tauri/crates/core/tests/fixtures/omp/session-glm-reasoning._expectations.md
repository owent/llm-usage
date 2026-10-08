# session-glm-reasoning._expectations.md

Source: `<HOME>/.omp/agent/sessions/-AppData-Local-Temp/2026-09-24T16-39-54-647Z_<UUID>.jsonl`.
Seven lines, oh-my-pi 18.2.7, 2026-09-24T16:39Z. Extracted on 2026-09-25.

<a id="结构期望"></a>

## Structural expectations

- All seven lines parse: parseErrors=0.
- Types: title ×1 (first line, OMP-specific layout, v=1), session ×1 (version=3),
  model_change ×1, thinking_level_change ×1, message ×2 (user and assistant),
  custom ×1 (customType=session_exit).
- Assistant: model=glm-5.3-flash, provider=zhipu-coding-plan, stopReason=stop,
  duration/ttft (OMP-specific floating-point milliseconds), responseId, and usage.reasoningTokens.

<a id="usage-数值期望白名单原字段"></a>

<a id="usage-数值期望提取的原字段"></a>

## Expected usage values (selected original fields)

Exactly one main model call:

| input | output | cacheRead | cacheWrite | totalTokens | reasoningTokens | cost.total |
| --- | --- | --- | --- | --- | --- | --- |
| 17542 | 65 | 0 | 0 | 17607 | 54 | 0 |

- Relationship: 17607=17542+65+0+0; reasoningTokens 54 is included in output 65.
- Mapping: input_uncached=17542, input_total=17542, output_total=65,
  output_reasoning=54 (reported), total_tokens=17607 (derived), source_total=17607.
- Round floating-point duration/ttft to i64 milliseconds before storage.
- cost.total=0 maps to no cost.

Seven anonymized IDs: anon-1…anon-7.
