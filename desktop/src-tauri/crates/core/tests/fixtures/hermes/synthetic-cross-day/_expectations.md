# synthetic-cross-day expectations (synthetic)

<a id="synthetic-cross-day-期望全合成"></a>

A cumulative row spans 2026-09-23T10:00:00Z–2026-09-25T18:30:00Z, with token=1000,
api_call_count=3 and only first_seen/last_seen. Epoch seconds are 1790157600 and
1790361000 respectively, calculated independently.

Manually calculated V03 expectations:

- **One** source_aggregates row, interval_start_ms=1790157600000,
  interval_end_ms=1790361000000. Do not split days, put all 1000 on the final day,
  or divide proportionally by duration.
- reported_call_count=3, no usage_events; do not invent three model_call records.
- Each 2026-09-23/24/25 daily summary has call_count=0 and unknown tokens, without details.
- sum_exclusive_aggregates: input_uncached=1000, output_total=200,
  reported_call_count=3, exclusive_rows=1.
- Initialized cache zeros remain unknown; do not fill input_total/total_tokens.
