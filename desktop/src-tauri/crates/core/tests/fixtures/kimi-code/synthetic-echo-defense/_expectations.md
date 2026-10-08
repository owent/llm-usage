# synthetic-echo-defense._expectations.md (synthetic)

<a id="synthetic-echo-defense_expectationsmdsynthetic"></a>

Prevent three duplicate routes: step.end repeats usage.record field for field, so only
usage.record counts; session-scope compaction counts independently as auxiliary;
subagent.completed.usage is a snapshot of summed sub-agent wire usage and is excluded,
even though this synthetic directory has no corresponding sub-agent file.
Expected: two events, primary {100,50,400,0} and auxiliary {900,70,0,0};
input_uncached=1000, cache_read=400, output_total=120, input_total=1400, total_tokens=1520.
The completed snapshot {123,45,678,0} appears in no total. Echo reconciliation equals
record-side usage: one turn record and one equal repeat.
