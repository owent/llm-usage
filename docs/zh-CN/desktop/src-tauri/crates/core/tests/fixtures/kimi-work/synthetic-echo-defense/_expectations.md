# synthetic-echo-defense._expectations.md（SYNTHETIC）

<a id="synthetic-echo-defense_expectationsmd-synthetic"></a>

防双计：step.end 回声与 usage.record 相同（只计记录）；session scope
（compaction）独立计 auxiliary；micro_compaction.apply 是控制记录不产事件。
期望：2 事件（1 primary {100,50,400,0} + 1 auxiliary {900,70,0,0}）；
input_uncached=1000、cache_read=400、output_total=120、input_total=1400、
total_tokens=1520。
