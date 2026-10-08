# agent-44-subagent._expectations.md (REAL, anonymized local extraction)

<a id="agent-44-subagent_expectationsmdreal本机脱敏提取"></a>

Source: Kimi Work's embedded kimi-code home,
`…/sessions/<wd>/conv-<hexid>/agents/agent-44/wire.jsonl`.
The original has 138 lines and protocol_version=1.4. Extracted on 2026-09-25: selected
fields retain 26 lines, with bodies REDACTED and identities replaced by anon-N.

<a id="结构期望"></a>

## Structural expectations

- First line: metadata with protocol_version="1.4".
- Eight usage.record entries: seven turn-scoped and one session-scoped compaction
  summary call within full_compaction.begin…complete. Seven step.end repeats;
  session scope has no repeat because repeats cover only the turn loop.
- Model k3-agent on all eight records; directory agent-44 classifies the file as sub_agent.

<a id="usage-数值期望工具输出与源文件-jq-独立求和一致roundtrip-复核相等"></a>

## Expected usage values (tool output, independent source jq sums and roundtrip agree)

| scope | n | inputOther | output | inputCacheRead | inputCacheCreation |
| --- | --- | --- | --- | --- | --- |
| turn | 7 | 204,485 | 23,343 | 374,784 | 0 |
| session | 1 | 29,663 | 12,440 | 183,040 | 0 |
| Σ | 8 | 234,148 | 35,783 | 557,824 | 0 |

The seven repeats sum to the turn sum; they exclude session scope.

<a id="期望入库人工核算"></a>

## Expected import (manually calculated)

Eight events: seven sub_agent and one auxiliary. input_uncached=234,148,
cache_read=557,824, cache_write=0 (reported), output_total=35,783,
input_total=791,972 (derived), total_tokens=827,755 (derived).

Fourteen anonymized IDs: anon-1…anon-14.
