# synthetic-unmapped-usage-keys._expectations.md (synthetic)

<a id="synthetic-unmapped-usage-keys_expectationsmdsynthetic"></a>

**All files in this directory are synthetic test data, not extracts from real sessions.** Usage contains keys beyond the four documented fields: service_tier or
unexpected_flag. Import mapped fields and record unmapped_usage_keys **once per file**,
using persistent parsing context to avoid duplicate diagnostics.

<a id="场景与期望人工核算"></a>

## Scenario and manually calculated expectations

Three lines: one user and two assistants.

- syn-req-k1: usage 10/3/2/1 plus service_tier → input_total=13, total=16.
- syn-req-k2: usage 20/4/0/0 plus unexpected_flag → input_total=20, total=24.
- Both produce events: files[0].events=2, added=2. SQL checks every mapped field.
- Exactly one unmapped_usage_keys diagnostic; the second extra-key record adds none.
- UTC 2026-09-24 summary: call_count=2, input_total_known=33, cache_read_known=2,
  cache_write_known=1, output_total_known=7, total_tokens_known=40.
