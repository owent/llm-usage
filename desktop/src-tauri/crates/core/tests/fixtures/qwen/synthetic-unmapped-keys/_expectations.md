# synthetic-unmapped-keys 期望（人工核算，全合成样本）

两条 assistant 的 usageMetadata 各带一个固定六分类之外的键（serviceTier /
anotherUnknown）：保留已映射字段，事件照常入账；`unmapped_usage_keys` 每文件只记一次
（解析上下文跨轮次持久化该标志）。

## 期望

- 事件数 = 2；汇总：call_count=2；input_total_known=300；output_total_known=50；
  total_tokens_known=350；cache_read_known=None（两条均未直报，未知不补零）。
- diagnostics 恰好 1 条 code=unmapped_usage_keys。
