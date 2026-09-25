# synthetic-missing-uuid 期望（人工核算，全合成样本）

第 2 行 assistant 缺 uuid：身份回退 `seq:{sessionId}:{行号}`，记 `missing_uuid` 诊断
（首行 user 带 uuid，detect 锚点不受影响）。

## 期望

- 事件数 = 1；source_record_key="seq:syn-sess-mu:2"；origin_call_id 为 NULL。
- diagnostics 1 条 code=missing_uuid。
- 汇总：call_count=1；input_total_known=100；output_total_known=20；
  total_tokens_known=120。
