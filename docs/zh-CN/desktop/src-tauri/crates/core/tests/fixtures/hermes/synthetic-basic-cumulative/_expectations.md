# synthetic-basic-cumulative 期望（全合成）

<a id="synthetic-basic-cumulative-expectations-all-synthetic"></a>

场景：单会话单模型累计行（task=''），2026-06-10T08:00:00.5Z .. 09:00:00.5Z。

输入行：api_call_count=3，input=100 output=40 cache_read=50 cache_write=10
reasoning=5，first_seen=1781337600.5 last_seen=1781341200.5（epoch 秒）。

期望（人工核算）：

- 1 条 `source_aggregates` 行（scope=session，coverage=exclusive）：
  - interval_start_ms=1781337600500，interval_end_ms=1781341200500，
    interval_end_inclusive=1，time_basis=uncertain；
  - input_uncached=100 input_total=160 input_cache_read=50 input_cache_write=10
    output_total=40 output_reasoning=5；
  - total_tokens=200（160+40，reasoning 不重加）、source_total=NULL；
    input_total/total_tokens 质量 derived，原生桶为 reported；
  - reported_call_count=3。
- `usage_events` 0 条：api_call_count 不拆成 model_call；
  2026-06-10 日汇总 call_count=0、token 全未知（不分配到单日）。
- 诊断恰 1 条 latest_fallback（整库 schema_version 不能据此核验逐行客户端版本）。
- sum_exclusive_aggregates：input_uncached=100、input_total=160、total_tokens=200、cache_read=50、cache_write=10、
  output=40、reasoning=5、reported_call_count=3、exclusive_rows=1。
