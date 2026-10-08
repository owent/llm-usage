# synthetic-auxiliary-carriers._expectations.md（SYNTHETIC）

<a id="synthetic-auxiliary-carriers_expectationsmd-synthetic"></a>

**本目录全部为合成样本（synthetic），不是真实会话提取。** 覆盖四类辅助 usage 记录
与无 usage 的 assistant。结构仿 pi session JSONL v3（固定源码
pi-mono b45597504eeaba1f11a9920a1d1048c361ed4b8e）。

<a id="scenario-and-expectations"></a>

## 场景与期望

8 行：session 头（version=3，id=syn-sess-aux）、model_change（syn-prov/syn-model-a）、
assistant ×2（syn-a-1 带完整 usage；syn-a-2 无 usage）、独立 usage 条目（kind=cache_warm）、
compaction 带 usage、branch_summary 带 usage、toolResult 消息带 usage。

逐条手工核算（input/cacheRead/cacheWrite 互斥，totalTokens=四桶之和）：

| 条目 | 类别 | input_total(derived) | cacheRead | cacheWrite | output | total(derived) | reasoning |
| --- | --- | --- | --- | --- | --- | --- | --- |
| syn-a-1 | primary | 100+800+100=1000 | 800 | 100 | 50 | 1050 | 10 |
| syn-a-2 | primary | 无 usage：token 全未知（不补零） | — | — | — | — | — |
| syn-u-1 | auxiliary | 10+90+0=100 | 90 | 0 | 0 | 100 | 无字段 |
| syn-c-1 | auxiliary | 1000 | 0 | 0 | 200 | 1200 | 无字段 |
| syn-b-1 | auxiliary | 500+300+0=800 | 300 | 0 | 100 | 900 | 无字段 |
| syn-t-1 | auxiliary | 5+0+10=15 | 0 | 10 | 5 | 20 | 无字段 |

- 事件数 = 6（2 primary + 4 auxiliary）；pi 无 sub_agent 类别。
- 汇总（2026-01-05）：call_count=6、input_total_known=2915、cache_read_known=1190、
  cache_write_known=110、output_total_known=355、total_tokens_known=3270
  （3270 = 1615 未缓存 + 1190 缓存读 + 110 缓存写 + 355 输出）。
- 模型归属：syn-a-1/syn-u-1 为请求字段（request_field）；syn-c-1/syn-b-1 按不晚于它们的
  model_change 归属 syn-model-a（structured_change）；syn-t-1 无模型（unknown）。
- 诊断：usage_shape_deviation ×1（syn-a-2）；其余条目五字段齐全且 totalTokens 一致，
  无其他诊断。
