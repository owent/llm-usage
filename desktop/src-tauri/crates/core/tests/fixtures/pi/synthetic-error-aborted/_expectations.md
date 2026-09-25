# synthetic-error-aborted._expectations.md（SYNTHETIC）

**本目录全部为合成样本（synthetic），不是真实会话提取。** 覆盖 stopReason=error/aborted
的 error_status 落库。结构仿 pi session JSONL v3。

## 场景与期望

3 行：session 头（syn-sess-err）、assistant syn-e-1（stopReason=error，
usage 10/20/0/0/30）、assistant syn-e-2（stopReason=aborted，usage 5/5/5/0/15）。

- 事件数 = 2，均 primary；error_status 分别为 "error"、"aborted"。
- 汇总（2026-01-05）：call_count=2、input_total_known=20（10 + 5+5）、
  cache_read_known=5、cache_write_known=0、output_total_known=25（20+5）、
  total_tokens_known=45（30+15）。
- 两条 usage 五字段齐全且 totalTokens 一致：无诊断。
