# synthetic-missing-id 期望（人工核算，全合成样本）

messages[1] 是带 tokens 的 gemini 消息但缺 id：身份回退数组下标
（`gemini:<sessionId>:idx-<下标>`，append-only 假设），记 `missing_message_id` 诊断。

## 期望

- 事件数 = 1；source_record_key="gemini:syn-sess-noid:idx-1"；origin_call_id 为 NULL。
- diagnostics 1 条 code=missing_message_id。
- 汇总：call_count=1；input_total_known=100；output_total_known=20；
  total_tokens_known=120。
