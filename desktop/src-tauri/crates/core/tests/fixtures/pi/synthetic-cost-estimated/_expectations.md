# synthetic-cost-estimated._expectations.md（SYNTHETIC）

**本目录全部为合成样本（synthetic），不是真实会话提取。** 覆盖 usage.cost.total 的
estimated 费用映射：>0 映射、=0 不映射（0 与无价目不可区分）。结构仿 pi session JSONL v3。

## 场景与期望

3 行：session 头（syn-sess-cost）、assistant syn-c-1（usage 100/50/0/0/150，
cost.total=0.005）、assistant syn-c-2（usage 5/5/0/0/10，cost.total=0）。

- syn-c-1：cost_amount_minor = 0.005 × 1_000_000 = 5000（USD，kind=estimated，
  price_version 缺失保持 NULL）。
- syn-c-2：cost.total=0 ⇒ cost_amount_minor/cost_currency/cost_kind 全 NULL。
- 汇总（2026-01-05）：call_count=2、input_total_known=155（150+5）、
  output_total_known=55（50+5）、total_tokens_known=160（150+10）。
