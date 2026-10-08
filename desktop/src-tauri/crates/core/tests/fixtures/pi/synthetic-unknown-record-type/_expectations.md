# synthetic-unknown-record-type._expectations.md (synthetic)

<a id="synthetic-unknown-record-type_expectationsmdsynthetic"></a>

**All files are synthetic, not native session extracts.** Pi session JSONL v3 ignores
unknown types and diagnoses each type once per file.

<a id="场景与期望"></a>

## Scenario and expectations

Four lines: session syn-sess-urt, brand_new_thing syn-x-1, assistant syn-m-1 with
usage 7/3/0/0/10, then brand_new_thing syn-x-2.

- Unknown records produce no events; one unknown_record_type diagnostic per file/type.
- One primary event, syn-m-1; records_seen=4, status=complete.
- 2026-01-05 summary: call_count=1, input_total_known=7, output_total_known=3, total_tokens_known=10.
