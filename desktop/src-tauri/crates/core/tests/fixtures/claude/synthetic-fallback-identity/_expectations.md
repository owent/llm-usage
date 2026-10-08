# synthetic-fallback-identity._expectations.md (synthetic)

<a id="synthetic-fallback-identity_expectationsmdsynthetic"></a>

**All files in this directory are synthetic test data, not extracts from real sessions.** Test the identity fallback order:
`req:{requestId}` → `msg:{message.id}` → `uuid:{uuid}` → `seq:{sessionId}:{line}`.
Record missing_request_id for each affected record, rather than once per file.

<a id="场景与期望人工核算"></a>

## Scenario and manually calculated expectations

Four lines: one user used for detection and three assistants with no requestId and different usage.

| Line | requestId | message.id | uuid | Expected source_record_key | origin_call_id |
| --- | --- | --- | --- | --- | --- |
| 2 | Missing | syn-msg-fb1 | syn-uuid-fb1 | msg:syn-msg-fb1 | syn-msg-fb1 |
| 3 | Missing | Missing | syn-uuid-fb2 | uuid:syn-uuid-fb2 | NULL |
| 4 | Missing | Missing | Missing | seq:syn-sess-1:4 | NULL |

- All three produce events: files[0].events=3, added=3. Usage 1/1/0/0, 2/2/0/0 and
  4/4/0/0 yields input_total 1/2/4 and total 2/4/8.
- Three missing_request_id diagnostics, at line 2/3/4 respectively.
- UTC 2026-09-24 summary: call_count=3, input_total_known=7, output_total_known=7,
  cache_read_known=0, cache_write_known=0, total_tokens_known=14.
