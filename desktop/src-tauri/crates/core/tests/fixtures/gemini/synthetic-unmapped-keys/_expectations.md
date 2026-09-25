# synthetic-unmapped-keys 期望（人工核算，全合成样本）

两条 gemini 消息的 tokens 各带一个文档六键之外的键（surpriseKey / anotherUnknown）：
保留已映射字段，事件照常入账；`unmapped_usage_keys` 诊断每轮只记一次。

## 期望

- 事件数 = 2；汇总：call_count=2；input_total_known=300；output_total_known=50；
  total_tokens_known=350；cache_read_known=None（两条消息均未直报 cached，未知不补零）。
- diagnostics 恰好 1 条 code=unmapped_usage_keys（两条带额外键的消息不重复记）。
