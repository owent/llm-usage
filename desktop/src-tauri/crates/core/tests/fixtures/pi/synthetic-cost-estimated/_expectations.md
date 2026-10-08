# synthetic-cost-estimated._expectations.md (synthetic)

<a id="synthetic-cost-estimated_expectationsmdsynthetic"></a>

**All files are synthetic, not native session extracts.** Test estimated usage.cost.total:
map positive values, leave zero unknown because it cannot be distinguished from missing
prices. Shape: Pi session JSONL v3.

<a id="场景与期望"></a>

## Scenario and expectations

Three lines: session syn-sess-cost; assistant syn-c-1, usage 100/50/0/0/150,
cost.total=0.005; assistant syn-c-2, usage 5/5/0/0/10, cost.total=0.

- syn-c-1: cost_amount_minor=0.005×1_000_000=5000, USD, kind=estimated;
  absent price_version remains NULL.
- syn-c-2: cost.total=0 leaves cost_amount_minor/cost_currency/cost_kind NULL.
- 2026-01-05 summary: call_count=2, input_total_known=105 (100+5),
  output_total_known=55 (50+5), total_tokens_known=160 (150+10).
