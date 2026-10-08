# synthetic-echo-defense._expectations.md (synthetic)

<a id="synthetic-echo-defense_expectationsmdsynthetic"></a>

step.end repeats usage.record and does not count separately. Session-scope compaction
counts independently as auxiliary. micro_compaction.apply is a control record with no event.
Expected: two events, primary {100,50,400,0} and auxiliary {900,70,0,0};
input_uncached=1000, cache_read=400, output_total=120, input_total=1400, total_tokens=1520.
