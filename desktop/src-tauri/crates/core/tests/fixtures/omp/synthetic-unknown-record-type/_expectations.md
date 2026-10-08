# synthetic-unknown-record-type._expectations.md (synthetic)

<a id="synthetic-unknown-record-type_expectationsmdsynthetic"></a>

**All files are synthetic, not native session extracts.** Ignore unknown record types
and diagnose each type once per file, without rejecting the file or changing active state.

<a id="场景与期望"></a>

## Scenario and expectations

- Five lines: title, session (version=3, id=syn-omp-urt), two brand_new_thing records,
  assistant syn-m1 with usage 7/3/0/0/10.
- One complete file, records_seen=5, one primary event, added=1, diagnostics=1
  (unknown_record_type); the repeated unknown type adds no diagnostic.
- UTC 2026-01-05 summary: call_count=1, input_total_known=7, output_total_known=3, total_tokens_known=10.
- source_files remains active; an unknown record type is not a format/version error.
