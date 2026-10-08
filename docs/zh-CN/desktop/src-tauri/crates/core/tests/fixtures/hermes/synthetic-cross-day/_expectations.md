# synthetic-cross-day 期望（全合成）

<a id="synthetic-cross-day-expectations-synthetic"></a>

场景：跨两日累计行。2026-09-23T10:00:00Z .. 2026-09-25T18:30:00Z；
token=1000/api_call_count=3，只有 first_seen/last_seen。

时间值（epoch 秒，人工核算）：2026-09-23T10:00:00Z=1790157600，
2026-09-25T18:30:00Z=1790361000。

期望（人工核算，V03 固定数学样本）：

- **1 条** `source_aggregates` 行：interval_start_ms=1790157600000、
  interval_end_ms=1790361000000；不拆成多天、不把 1000 记入最后一天、
  不按时长摊分。
- reported_call_count=3；`usage_events` 0 条（不产生 3 条 model_call）。
- 2026-09-23/24/25 任一日的日汇总 call_count=0、token 全未知（无详单）。
- sum_exclusive_aggregates：input_uncached=1000、output_total=200、
  reported_call_count=3、exclusive_rows=1。
- 默认缓存零未知，input_total/total_tokens 不补全。
