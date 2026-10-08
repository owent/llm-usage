# conv-main._expectations.md (REAL, anonymized local extraction)

<a id="conv-main_expectationsmdreal本机脱敏提取"></a>

Source: Kimi Work's embedded kimi-code home,
`…/Kimi/share/daimon-share/daimon/runtime/kimi-code/home/sessions/<wd>/conv-<hexid>/agents/main/wire.jsonl`.
The original has 363 lines, wire protocol_version=1.4 and host daimon. state.json
createdBy=daimon-kernel-adapter also identifies the product. Extracted on 2026-09-25:
tools/extract-kimi.mjs retains 115 lines of selected fields, with bodies REDACTED and IDs anon-N.

<a id="结构期望"></a>

## Structural expectations

- First line: metadata with protocol_version="1.4".
- 38 turn-scoped usage.record entries, with no agentId: the 1.4 main wire omits agentId;
  Agent identity comes from agents/main. All 38 step.end repeats contain usage.
- Model k28-agent-preview ×38; llm.request provider=kimi.

<a id="usage-数值期望工具输出与源文件-jq-独立求和一致roundtrip-复核相等"></a>

## Expected usage values (tool output, independent source jq sums and roundtrip agree)

| n | inputOther | output | inputCacheRead | inputCacheCreation |
| --- | --- | --- | --- | --- |
| 38 | 47,574 | 18,658 | 1,310,720 | 0 |

Repeat and record sums match exactly (38=38); this session has no interrupted steps.

<a id="与-kimi-code15的实测差异独立-fixture-核验结果不因内核同名合并"></a>

<a id="与-kimi-code15的实测差异独立-fixture-证据不因内核同名合并"></a>

## Measured differences from Kimi Code 1.5 (separate native samples)

- Directory: `conv-<hexid>` rather than `session_<uuid>`; this session has no subagent directory.
- usage.record omits agentId, which 1.5 main records include. Models use bare IDs rather
  than alias/model combinations. A shared kernel name does not merge product verification.

<a id="期望入库人工核算"></a>

## Expected import (manually calculated)

38 primary events: input_uncached=47,574, cache_read=1,310,720,
cache_write=0 (reported), output_total=18,658, input_total=1,358,294 (derived),
total_tokens=1,376,952 (derived).

76 anonymized IDs: anon-1…anon-76.
