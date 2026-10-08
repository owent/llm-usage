# synthetic-basic-cumulative expectations (all synthetic)

<a id="synthetic-basic-cumulative-期望全合成"></a>

Scenario: one cumulative row for one session and model (task=''),
2026-06-10T08:00:00.5Z..09:00:00.5Z.

Input: api_call_count=3, input=100, output=40, cache_read=50, cache_write=10, reasoning=5;
first_seen=1781337600.5 and last_seen=1781341200.5 (epoch seconds).

Manually calculated expectations:

- One `source_aggregates` row, scope=session and coverage=exclusive:
  - interval_start_ms=1781337600500, interval_end_ms=1781341200500,
    interval_end_inclusive=1, time_basis=uncertain.
  - input_uncached=100, input_total=160, input_cache_read=50, input_cache_write=10,
    output_total=40, output_reasoning=5.
  - total_tokens=200 (160+40; reasoning is not added twice), source_total=NULL.
    input_total/total_tokens are derived; native fields are reported.
  - reported_call_count=3.
- Zero `usage_events`: do not split api_call_count into model_call events.
  The 2026-06-10 daily summary has call_count=0 and unknown tokens; the cumulative row
  is not assigned to a single day.
- Exactly one latest_fallback diagnostic: database schema_version does not verify
  each row's client version.
- sum_exclusive_aggregates: input_uncached=100, input_total=160, total_tokens=200,
  cache_read=50, cache_write=10, output=40, reasoning=5, reported_call_count=3, exclusive_rows=1.
