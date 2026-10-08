# session-main._expectations.md (REAL, anonymized local extraction)

<a id="session-main_expectationsmdreal本机脱敏提取"></a>

Source: `~/.kimi-code/sessions/<wd>/session_<uuid>/agents/main/wire.jsonl`.
The original has 4,225 lines, Kimi Code desktop 1.0.3 and wire protocol_version=1.5.
Extracted on 2026-09-25 through tools/extract-kimi.mjs: selected fields retain 1,141
lines; non-usage types are excluded and counted in droppedByType. Bodies are REDACTED;
IDs are anon-N. At extraction the completed session file had stopped growing.

<a id="结构期望"></a>

## Structural expectations

- First line metadata has protocol_version="1.5"; all 1,141 rebuilt JSONL lines parse.
- Retained counts: usage.record ×372 (369 turn, three session), llm.request ×221,
  context.append_loop_event step.end repeats ×372 (368 with usage),
  subagent.spawned/started/failed ×21, subagent.completed ×1 (agent-0),
  full_compaction.begin/complete ×6.
- Models by usage.record: Kimi For Coding - Backup/k3-256k ×324,
  kimi-code/k3-256k ×27, kimi-code/kimi-for-coding ×21.

<a id="usage-数值期望工具输出与源文件-jq-独立求和一致roundtrip-复核相等"></a>

## Expected usage values (tool output, independent source jq sums and roundtrip agree)

| scope | n | inputOther | output | inputCacheRead | inputCacheCreation |
| --- | --- | --- | --- | --- | --- |
| turn | 369 | 1,002,786 | 245,794 | 46,991,104 | 0 |
| session | 3 | 503,907 | 11,579 | 75,264 | 0 |
| Σ | 372 | 1,506,693 | 257,373 | 47,066,368 | 0 |

The 368 usage-bearing step.end repeats sum to inputOther=996,163, output=245,368,
inputCacheRead=46,858,496, creation=0: a **subset** of turn records (368<369).
One turn had no repeat after swarm interruption. Repeats never count separately.

<a id="对账与防双计"></a>

## Comparison and duplicate prevention

- subagent.completed for agent-0 at time=1790269234273 has usage
  {66876, 4631, 1264384, 0}, matching **every field** of the first 22 wire records
  at time≤completion in subagent-agent-0, independently checked with jq.
- completed.usage creates **no event**: individual subagent wire records already count.
  Session-scoped compaction summary calls are separate auxiliary calls.

<a id="期望入库人工核算"></a>

## Expected import (manually calculated)

372 events: 369 primary, three auxiliary; input_uncached=1,506,693,
cache_read=47,066,368, cache_write=0 (reported), output_total=257,373,
input_total=48,573,061 (derived), total_tokens=48,830,434 (derived, no source total).

746 anonymized IDs: anon-1…anon-746.
