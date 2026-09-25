# synthetic-unknown-record-type._expectations.md（SYNTHETIC）

**本目录全部为合成样本（synthetic），不是真实会话提取。** 覆盖未知条目类型的
忽略与诊断（同类型每文件只记一次）。结构仿 pi session JSONL v3。

## 场景与期望

4 行：session 头（syn-sess-urt）、brand_new_thing（syn-x-1）、assistant syn-m-1
（usage 7/3/0/0/10）、brand_new_thing（syn-x-2）。

- 未知类型条目被忽略，不产事件；unknown_record_type 诊断每文件每类型只记一次（×1）。
- 事件数 = 1（syn-m-1，primary）；records_seen=4；status=complete。
- 汇总（2026-01-05）：call_count=1、input_total_known=7、output_total_known=3、
  total_tokens_known=10。
