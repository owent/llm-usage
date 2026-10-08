# synthetic-auxiliary-carriers._expectations.md (SYNTHETIC)

<a id="synthetic-auxiliary-carriers_expectationsmdsynthetic"></a>

**All samples in this directory are synthetic, rather than extracted native sessions.**
This tests four auxiliary usage locations absent from 58 locally read OMP files:
standalone usage, compaction, branch_summary and toolResult.usage. It also tests an
assistant without usage.

<a id="场景与期望"></a>

## Scenario and expectations

- Nine lines: title, session (version=3, id=syn-omp-aux), model_change using native OMP
  combined model="syn-provider/syn-model-a", assistant syn-a1 (usage and duration/ttft),
  assistant syn-a2 (no usage), usage syn-u1 (kind=cache_warm), compaction syn-c1,
  branch_summary syn-b1, toolResult syn-t1.
- Scan: one complete file, lines_read=9, records_seen=9, six events, added=6,
  diagnostics=1: missing syn-a2 usage produces one usage_shape_deviation.
- Categories: syn-a1/syn-a2 primary; syn-u1/syn-c1/syn-b1/syn-t1 auxiliary.
- Attribution: syn-a1 request fields (request_field); syn-u1's own provider/model
  (request_field); syn-c1/syn-b1 have no model fields and use model_change
  (structured_change → syn-model-a / syn-provider). syn-t1 model is unknown.
- Usage tuples (input/output/cacheRead/cacheWrite/totalTokens):
  syn-a1=100/50/10/5/165, syn-u1=0/0/0/1000/1000, syn-c1=200/100/0/0/300,
  syn-b1=300/150/0/0/450, syn-t1=10/5/0/0/15. Each total equals the four-field sum.
- Summary (2026-01-05 UTC): call_count=6, input_total_known=1625
  (derived: 115+1000+200+300+10), uncached_known=610, cache_read_known=10,
  cache_write_known=1005, output_total_known=305, total_tokens_known=1930,
  input_unknown_count=0: syn-a2 counts a call, without counting unknown usage fields.
- Round OMP floating-point milliseconds: syn-a1 duration 200.4→200, ttft 60.6→61;
  syn-a2 duration 150.5→151, absent ttft=None. Auxiliary entries have no latency fields.
