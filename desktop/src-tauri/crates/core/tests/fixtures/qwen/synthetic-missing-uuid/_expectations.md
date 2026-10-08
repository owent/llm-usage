# synthetic-missing-uuid expectations (manually calculated, synthetic)

<a id="synthetic-missing-uuid-期望人工核算全合成样本"></a>

Assistant line 2 lacks uuid. Use seq:{sessionId}:{line} and record missing_uuid.
The first user line has uuid, so detection is unaffected.

<a id="期望"></a>

## Expectations

- One event; source_record_key="seq:syn-sess-mu:2"; origin_call_id=NULL.
- One missing_uuid diagnostic.
- Summary: call_count=1, input_total_known=100, output_total_known=20, total_tokens_known=120.
