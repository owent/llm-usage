# synthetic-cost-estimated._expectations.md (SYNTHETIC)

<a id="synthetic-cost-estimated_expectationsmdsynthetic"></a>

**All samples in this directory are synthetic, rather than extracted native sessions.**
usage.cost is the Agent's estimate from its own price list, rather than a provider bill.
Positive values map to estimated; zero cannot be distinguished from missing prices and
does not map. Local native verification found cost.total>0 in two sessions; all others were zero.

<a id="场景与期望"></a>

## Scenario and expectations

- Four lines: title, session (version=3, id=syn-omp-cost), assistant syn-c1
  (cost.total=0.058278), and assistant syn-c2 (cost.total=0).
- Scan: one complete file, four lines, two primary events, added=2.
- syn-c1 cost maps to (amount_minor=58278, currency=USD, kind=estimated), rounding
  0.058278 USD × 1e6. syn-c2 cost.total=0 remains unknown.
- Summary (2026-01-05 UTC): call_count=2, input_total_known=105 (100+5),
  output_total_known=55, total_tokens_known=160, cache_read_known=Some(0),
  cache_write_known=Some(0).
