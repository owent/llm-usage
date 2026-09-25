# synthetic-negative-usage 期望（人工核算，全合成样本）

syn-a-bad 的 usageMetadata.promptTokenCount=-100（负值，超界）：形状偏离但**不**
fail closed，跳过该记录并记 `usage_shape_deviation`；其余记录正常入账。

## 期望

- 事件数 = 1（syn-a-ok）：input_total=1000、output_total=50、cache_read=400、
  total_tokens=1050；files[0].status="complete"，源文件 health=degraded。
- diagnostics 1 条 code=usage_shape_deviation；qwen:syn-a-bad 无事件。
- 汇总：call_count=1；input_total_known=1000；output_total_known=50；
  cache_read_known=400；total_tokens_known=1050。
