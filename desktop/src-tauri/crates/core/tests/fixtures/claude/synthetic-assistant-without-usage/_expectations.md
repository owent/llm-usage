# synthetic-assistant-without-usage._expectations.md（SYNTHETIC）

**本目录全部为合成样本（synthetic），不是真实会话提取。** assistant 条目无
`message.usage`：未提供用量字段，不产事件（未知不补零），记
`assistant_without_usage` 诊断，**每文件一次**（解析上下文持久化去重）。

## 场景与期望（人工核算）

4 行：user ×1 + assistant ×3：

- 第 2 行 syn-req-ok 正常携带 usage 10/5/0/0 → 1 事件（input_total=10、
  output=5、total=15）；
- 第 3 行 message 无 usage 键 → 不产事件，记诊断一次；
- 第 4 行连 message 键都无 → 不产事件，**不再重复记诊断**。

- files[0]：complete、lines_read=4、records_seen=4、events=1。
- 入库 added=1；usage_events 共 1 行；assistant_without_usage 诊断恰好 1 行。
- 汇总（UTC 2026-09-24）：call_count=1、input_total_known=10、
  output_total_known=5、total_tokens_known=15。
