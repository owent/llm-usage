# synthetic-aux-task-mutex 期望（全合成）

场景：同一会话同一模型，主累计行（task=''，input=100，api=3）与独立任务
辅助累计行（task='background_review'，input=20，api=1）。

固定源码证据：record_auxiliary_usage 只写任务键行、不进主会话总量；
两行分属六列组合键，累计互斥。

期望（人工核算，V03 固定数学样本）：

- 2 条 `source_aggregates` 行，coverage 均 exclusive，duplicate_rows=0。
- sum_exclusive_aggregates：input_total=100+20=**120**（不是 220——
  辅助行不折算进主会话总量再计一次）、reported_call_count=3+1=4、
  exclusive_rows=2、overlap_unknown_rows=0。
- `usage_events` 0 条。
