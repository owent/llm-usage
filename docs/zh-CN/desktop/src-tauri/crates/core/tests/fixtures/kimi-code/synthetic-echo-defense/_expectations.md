# synthetic-echo-defense._expectations.md（SYNTHETIC）

<a id="synthetic-echo-defense_expectationsmd-synthetic"></a>

防双计三连：step.end 回声与 usage.record 逐字段相同（只计 usage.record）；
session scope（compaction）独立计 auxiliary；subagent.completed.usage
是子代理 wire Σ 的快照（本合成无对应子代理文件）不得入账。
期望：2 事件（1 primary {100,50,400,0} + 1 auxiliary {900,70,0,0}）；
input_uncached=1000、cache_read=400、output_total=120、input_total=1400、
total_tokens=1520；completed 的 {123,45,678,0} 不出现在任何合计中；
回声对账行 echo=记录侧（turn 记 1 条/回声 1 条相等）。
