# synthetic-assistant-without-usage._expectations.md (synthetic)

<a id="synthetic-assistant-without-usage_expectationsmdsynthetic"></a>

**All files in this directory are synthetic test data, not extracts from real sessions.** An assistant without `message.usage` has no reported usage and produces
no event. Unknown values are not replaced with zero. Record assistant_without_usage
**once per file**, using persistent parsing context to avoid repeat diagnostics.

<a id="场景与期望人工核算"></a>

## Scenario and manually calculated expectations

Four lines: one user and three assistant records.

- Line 2, syn-req-ok: usage 10/5/0/0 produces one event, input_total=10, output=5, total=15.
- Line 3 has no usage key in message: no event, one diagnostic.
- Line 4 has no message key: no event and **no repeated diagnostic**.
- files[0]: complete, lines_read=4, records_seen=4, events=1.
- Import: added=1, one usage_events row and exactly one assistant_without_usage diagnostic.
- UTC 2026-09-24 summary: call_count=1, input_total_known=10, output_total_known=5,
  total_tokens_known=15.
