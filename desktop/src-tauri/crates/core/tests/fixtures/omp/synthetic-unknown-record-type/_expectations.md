# synthetic-unknown-record-type._expectations.md（SYNTHETIC）

**本目录全部为合成样本（synthetic），不是真实会话提取。** 未知条目类型不
fail closed：忽略并记诊断，每文件每类型只记一次，文件状态保持 active。

## 场景与期望

- 5 行：title、session（version=3，id=syn-omp-urt）、brand_new_thing ×2、
  assistant syn-m1（usage 7/3/0/0/10）。
- 整轮：1 文件 complete、records_seen=5、1 事件（primary）、added=1、
  diagnostics=1（unknown_record_type，同类型第二次不重复记）。
- 汇总（2026-01-05 UTC）：call_count=1、input_total_known=7、
  output_total_known=3、total_tokens_known=10。
- source_files 状态保持 active（未知类型不是格式/版本错误）。
