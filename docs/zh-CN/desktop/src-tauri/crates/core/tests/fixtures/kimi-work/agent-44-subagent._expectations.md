# agent-44-subagent._expectations.md（REAL，本机脱敏提取）

<a id="agent-44-subagent_expectationsmd-real-anonymized-local-extraction"></a>

来源：Kimi Work 内嵌 kimi-code home
`…/sessions/<wd>/conv-<hexid>/agents/agent-44/wire.jsonl`
（原始 138 行；protocol_version=1.4）。提取时间：2026-09-25
（按允许字段提取保留 26 行；正文 REDACTED、ID anon-N）。

<a id="structural-expectations"></a>

## 结构期望

- 首行 `metadata`（protocol_version="1.4"）。
- usage.record ×8：turn ×7 + session ×1（compaction 摘要调用，
  与 full_compaction.begin…complete 区间对应）；step.end 回声 ×7
  （session scope 无回声——回声只覆盖 turn 循环）。
- 模型：`k3-agent` ×8；目录名 agent-44 ⇒ sub_agent 分类。

<a id="expected-usage-values-tool-output-independent-source-jq-sums-and-roundtrip-agree"></a>

## usage 数值期望（工具输出与源文件 jq 独立求和一致；roundtrip 复核相等）

| scope | n | inputOther | output | inputCacheRead | inputCacheCreation |
| --- | --- | --- | --- | --- | --- |
| turn | 7 | 204,485 | 23,343 | 374,784 | 0 |
| session | 1 | 29,663 | 12,440 | 183,040 | 0 |
| Σ | 8 | 234,148 | 35,783 | 557,824 | 0 |

回声 Σ = turn Σ（7 条），session scope 不在回声内。

<a id="expected-import-manually-calculated"></a>

## 期望入库（人工核算）

8 事件：7 sub_agent + 1 auxiliary；input_uncached=234,148；
cache_read=557,824；cache_write=0（reported）；output_total=35,783；
input_total=791,972（派生）；total_tokens=827,755（派生）。

匿名 ID：14 个（anon-1…anon-14）。
