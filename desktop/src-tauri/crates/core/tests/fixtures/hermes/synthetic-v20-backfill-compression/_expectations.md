# synthetic-v20-backfill-compression 期望（全合成）

场景：压缩父会话（end_reason='compression'，2026-09-20T09:00Z..12:00Z）
带一条 v20 回填行（历史汇总种成模型行，first/last_seen NULL）；
压缩子会话（parent_session_id 指向父）有自己的实时行。

时间值（epoch 秒）：父 started=1789894800 ended=1789905600；
子 started=1789905660，实时行 first=1789905660.5 last=1789916000.0。

期望（人工核算）：

- 2 条 `source_aggregates` 行：
  - 回填行：interval_start_ms=NULL（first_seen 未知）、
    interval_end_ms=1789905600000（回退 session ended_at 时间窗）、
    reported_call_count=0、input_total=500；
  - 子会话实时行：interval 1789905660500..1789916000000、
    input_total=300、cache_read=20、reported_call_count=2。
- 压缩继承不双计：sum input_total=500+300=**800**（不是把父汇总复制给子）。
- `usage_events` 0 条；coverage 均 exclusive。
- 注意：回填行不证明历史调用使用 legacy-model（v20 语义），api_call_count
  种子值未逐字核验，按列面值 0 处理。
