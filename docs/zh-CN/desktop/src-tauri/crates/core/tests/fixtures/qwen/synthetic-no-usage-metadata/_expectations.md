# synthetic-no-usage-metadata 期望（人工核算，全合成样本）

<a id="synthetic-no-usage-metadata-expectations-manually-calculated-synthetic"></a>

assistant 无 usageMetadata 属正常形状（固定源码中 tokens 参数本身可选）：
不产事件、**无诊断**；其余记录照常入账。

<a id="expectations"></a>

## 期望

- files[0].status="complete"；事件数 = 1（syn-a-1）；records_seen=3；diagnostics=0。
- 汇总：call_count=1；input_total_known=100；output_total_known=20；
  total_tokens_known=120。
