# synthetic-unmapped-usage-keys._expectations.md（SYNTHETIC）

<a id="synthetic-unmapped-usage-keys_expectationsmd-synthetic"></a>

**本目录全部为合成样本（synthetic），不是真实会话提取。** usage 对象携带
文档四字段之外的额外键（service_tier / unexpected_flag）：已映射字段保留
入账，记 `unmapped_usage_keys` 诊断，**每文件一次**（解析上下文持久化去重）。

<a id="scenario-and-manually-calculated-expectations"></a>

## 场景与期望（人工核算）

3 行：user ×1 + assistant ×2：

- syn-req-k1：usage 10/3/2/1 + service_tier → input_total=13、total=16；
- syn-req-k2：usage 20/4/0/0 + unexpected_flag → input_total=20、total=24。

- 两条均产事件（files[0].events=2，added=2），映射字段完整保留（SQL 逐键核验）。
- 诊断：unmapped_usage_keys 恰好 1 行（第二条带额外键的条目不重复记）。
- 汇总（UTC 2026-09-24）：call_count=2、input_total_known=33、
  cache_read_known=2、cache_write_known=1、output_total_known=7、
  total_tokens_known=40。
