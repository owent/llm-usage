# synthetic-fallback-identity._expectations.md（SYNTHETIC）

**本目录全部为合成样本（synthetic），不是真实会话提取。** 覆盖事件身份回退链：
`req:{requestId}` → `msg:{message.id}` → `uuid:{uuid}` → `seq:{sessionId}:{行号}`，
逐级记 `missing_request_id` 诊断（逐条记，非每文件一次）。

## 场景与期望（人工核算）

4 行：user ×1（detect 锚点）+ assistant ×3（均缺 requestId，usage 互不相同）：

| 行 | requestId | message.id | uuid | 期望 source_record_key | origin_call_id |
| --- | --- | --- | --- | --- | --- |
| 2 | 缺 | syn-msg-fb1 | syn-uuid-fb1 | msg:syn-msg-fb1 | syn-msg-fb1 |
| 3 | 缺 | 缺 | syn-uuid-fb2 | uuid:syn-uuid-fb2 | NULL |
| 4 | 缺 | 缺 | 缺 | seq:syn-sess-1:4 | NULL |

- 3 条均产事件（files[0].events=3，added=3），usage 1/1/0/0、2/2/0/0、4/4/0/0
  → input_total 1/2/4、total 2/4/8。
- 诊断：missing_request_id ×3（每条一次，position 分别为 line 2/3/4）。
- 汇总（UTC 2026-09-24）：call_count=3、input_total_known=7、
  output_total_known=7、cache_read_known=0、cache_write_known=0、
  total_tokens_known=14。
