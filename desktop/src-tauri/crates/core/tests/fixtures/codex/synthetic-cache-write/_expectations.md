# synthetic-cache-write._expectations.md (synthetic)

<a id="synthetic-cache-write_expectationsmdsynthetic"></a>

**All files in this directory are synthetic test data, not extracts from real sessions.** Test cache_write>0, absent from M0 native Codex samples, where this field
was always zero. The shape follows Codex 0.155.0-alpha.16.3.

<a id="场景与期望"></a>

## Scenario and expectations

- One call: input=1000, cached=400, **cache_write=100**, output=50, reasoning=10,
  total=1050. The snapshot matches these values; reconciliation=matched.
- Mapping: input_cache_write=100 (reported); input_uncached=1000−400−100=
  **500 (derived)**; input_total=1000 (reported); total_tokens=1050 (derived),
  source_total=1050 (reported).
- This synthetic sample alone defines the cache_write⊆input assumption. If real data
  contradicts it, producing uncached<0, record a diagnostic rather than truncating the value.
