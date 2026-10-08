# subagent-agent-0._expectations.md (REAL, anonymized local extraction)

<a id="subagent-agent-0_expectationsmdreal本机脱敏提取"></a>

Source: `~/.kimi-code/sessions/<wd>/session_<uuid>/agents/agent-0/wire.jsonl`.
The original has 1,619 lines and protocol_version=1.5; this is a subagent wire file from
the same session as session-main. Extracted on 2026-09-25: selected fields retain 455
lines, with bodies REDACTED and IDs anon-N.

<a id="结构期望"></a>

## Structural expectations

- First line: metadata with protocol_version="1.5".
- 150 usage.record entries, all turn scope with no session scope. All 150 step.end
  repeats include usage; repeat and record counts are equal in this file.
- All 150 records use kimi-code/k3-256k.

<a id="usage-数值期望工具输出与源文件-jq-独立求和一致roundtrip-复核相等"></a>

## Expected usage values (tool output, independent source jq sums and roundtrip agree)

| n | inputOther | output | inputCacheRead | inputCacheCreation |
| --- | --- | --- | --- | --- |
| 150 | 198,401 | 72,798 | 18,428,416 | 0 |

<a id="子代理对账m0-结论在本机复证"></a>

## Subagent comparison (M0 result rechecked locally)

- The file **continued growing after** main-stream subagent.completed at
  time=1790269234273; the subagent was restarted and reused until cancelled.
  The first 22 records with time≤completed sum to {66876, 4631, 1264384, 0}, matching
  completed.usage field by field.
- completed.usage is a **snapshot at completion**, not the larger complete-file sum.
  Neither snapshot is added separately; individual records in this file are the sole usage source.

<a id="期望入库人工核算"></a>

## Expected import (manually calculated)

150 sub_agent events: input_uncached=198,401, cache_read=18,428,416,
cache_write=0 (reported), output_total=72,798, input_total=18,626,817 (derived),
total_tokens=18,699,615 (derived).

302 anonymized IDs: anon-1…anon-302.
