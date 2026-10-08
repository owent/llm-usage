# synthetic-auxiliary-carriers._expectations.md (SYNTHETIC)

<a id="synthetic-auxiliary-carriers_expectationsmdsynthetic"></a>

**All samples in this directory are synthetic, rather than extracted native sessions.**
This covers four auxiliary usage locations and an assistant without usage, following
Pi session JSONL v3 at fixed pi-mono revision b45597504eeaba1f11a9920a1d1048c361ed4b8e.

<a id="场景与期望"></a>

## Scenario and expectations

Eight lines: session (version=3, id=syn-sess-aux), model_change (syn-prov/syn-model-a),
two assistants (syn-a-1 with complete usage; syn-a-2 without), standalone usage
(kind=cache_warm), compaction with usage, branch_summary with usage, and toolResult with usage.

Manual calculation: input/cacheRead/cacheWrite are exclusive; totalTokens sums all four fields.

| Entry | Category | input_total (derived) | cacheRead | cacheWrite | output | total (derived) | reasoning |
| --- | --- | --- | --- | --- | --- | --- | --- |
| syn-a-1 | primary | 100+800+100=1000 | 800 | 100 | 50 | 1050 | 10 |
| syn-a-2 | primary | no usage; tokens unknown, no filled zeros | — | — | — | — | — |
| syn-u-1 | auxiliary | 10+90+0=100 | 90 | 0 | 0 | 100 | absent |
| syn-c-1 | auxiliary | 1000 | 0 | 0 | 200 | 1200 | absent |
| syn-b-1 | auxiliary | 500+300+0=800 | 300 | 0 | 100 | 900 | absent |
| syn-t-1 | auxiliary | 5+0+10=15 | 0 | 10 | 5 | 20 | absent |

- Six events: two primary, four auxiliary; Pi has no sub_agent category.
- Summary (2026-01-05): call_count=6, input_total_known=2915, cache_read_known=1190,
  cache_write_known=110, output_total_known=355, total_tokens_known=3270:
  1615 uncached +1190 cache read +110 cache write +355 output.
- Model: syn-a-1/syn-u-1 request_field; syn-c-1/syn-b-1 use the latest preceding
  model_change for syn-model-a (structured_change); syn-t-1 unknown.
- One usage_shape_deviation for syn-a-2. Other entries have all five usage fields
  and consistent totalTokens; no other diagnostics.
