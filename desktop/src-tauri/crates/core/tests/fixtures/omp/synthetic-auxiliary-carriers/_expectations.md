# synthetic-auxiliary-carriers._expectations.md（SYNTHETIC）

**本目录全部为合成样本（synthetic），不是真实会话提取。** 覆盖 omp 真实样本
缺失的四类辅助 usage 载体（独立 usage 条目、compaction、branch_summary、
toolResult.usage；本机 58 文件实读均未携带 usage）与「无 usage 的 assistant」。

## 场景与期望

- 9 行：title、session（version=3，id=syn-omp-aux）、model_change（omp 真实
  组合形状 model="syn-provider/syn-model-a"）、assistant syn-a1（带 usage 与
  duration/ttft）、assistant syn-a2（无 usage）、usage syn-u1（kind=cache_warm）、
  compaction syn-c1、branch_summary syn-b1、toolResult syn-t1。
- 整轮：1 文件 complete、lines_read=9、records_seen=9、6 事件、added=6、
  diagnostics=1（syn-a2 无 usage ⇒ usage_shape_deviation ×1）。
- 分类：syn-a1/syn-a2 primary；syn-u1/syn-c1/syn-b1/syn-t1 auxiliary。
- 归属：syn-a1 请求字段自带（request_field）；syn-u1 条目自有 provider/model
  （request_field）；syn-c1/syn-b1 无模型字段，按 model_change 组合字段归属
  （structured_change ⇒ syn-model-a / syn-provider）；syn-t1 无模型证据（unknown）。
- 逐事件 usage（input/output/cacheRead/cacheWrite/totalTokens）：
  syn-a1=100/50/10/5/165；syn-u1=0/0/0/1000/1000；syn-c1=200/100/0/0/300；
  syn-b1=300/150/0/0/450；syn-t1=10/5/0/0/15。逐条 total=四桶之和成立。
- 汇总（2026-01-05 UTC）：call_count=6；
  input_total_known=1625（派生口径 115+1000+200+300+10）；
  uncached_known=610；cache_read_known=10；cache_write_known=1005；
  output_total_known=305；total_tokens_known=1930；input_unknown_count=0（syn-a2 无 usage 计调用不计未知字段）。
- 延迟（omp 特有浮点毫秒四舍五入）：syn-a1 duration 200.4→200、ttft 60.6→61；
  syn-a2 duration 150.5→151、ttft 缺字段保持 None；辅助载体一律无延迟字段。
