# synthetic-cost-estimated._expectations.md（SYNTHETIC）

<a id="synthetic-cost-estimated_expectationsmd-synthetic"></a>

**本目录全部为合成样本（synthetic），不是真实会话提取。** usage.cost 为 Agent
自带价目估算（非供应商账单）：>0 映射为 estimated，0 与无价目不可区分不映射。
本机真实核验结果：2 个会话观测到 cost.total>0，其余均为 0。

<a id="scenario-and-expectations"></a>

## 场景与期望

- 4 行：title、session（version=3，id=syn-omp-cost）、assistant syn-c1
  （cost.total=0.058278）、assistant syn-c2（cost.total=0）。
- 整轮：1 文件 complete、4 行、2 事件全 primary、added=2。
- syn-c1：cost → (amount_minor=58278, currency=USD, kind=estimated)
  （0.058278 USD × 1e6 四舍五入）；syn-c2：cost.total=0 ⇒ 不映射（unknown）。
- 汇总（2026-01-05 UTC）：call_count=2、input_total_known=105（100+5）、
  output_total_known=55、total_tokens_known=160、cache_read_known=Some(0)、
  cache_write_known=Some(0)。
