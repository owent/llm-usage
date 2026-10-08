# synthetic-aux-task-mutex expectations (synthetic)

<a id="synthetic-aux-task-mutex-期望全合成"></a>

Same session/model: a main cumulative row, task='', input=100, api=3; and an independent
auxiliary row, task='background_review', input=20, api=1. Fixed source shows
record_auxiliary_usage writes only the task-key row, excluding it from the main session
total. Their six-column composite keys are different; these cumulative values do not overlap.

Manually calculated V03 expectations:

- Two source_aggregates rows, both coverage=exclusive; duplicate_rows=0.
- sum_exclusive_aggregates: input_uncached=100+20=**120**, never 220 by adding auxiliary
  into main and counting it again; reported_call_count=3+1=4, exclusive_rows=2,
  overlap_unknown_rows=0.
- Initialized cache zeros remain unknown; do not fill input_total/total_tokens.
- No usage_events.
