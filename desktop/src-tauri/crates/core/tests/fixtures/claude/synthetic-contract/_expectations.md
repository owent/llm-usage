# synthetic-contract._expectations.md (synthetic)

<a id="synthetic-contract_expectationsmdsynthetic"></a>

**All files in this directory are synthetic test data, not extracts from real sessions.** Claude Code was not_found locally during the M2-C inspection, so no real
sample was available. The A01 documented layout is represented by
`projects/<project>/<session>.jsonl`. Assistant records contain all four required
message.usage fields. One API response is split into multiple content-block records;
upsert by requestId deduplicates them. All file IDs have the syn- prefix.

<a id="场景与期望人工核算"></a>

## Scenario and manually calculated expectations

Eight lines: two user, one system and five assistant records. User/system records have
no usage and produce no events.

| requestId | input | output | cache_read | cache_creation | input_total (derived) | total (derived) |
| --- | --- | --- | --- | --- | --- | --- |
| syn-req-1 (lines 3/4/5 share requestId and usage; one deduplicated call) | 100 | 20 | 40 | 10 | 150 | 170 |
| syn-req-2 | 200 | 30 | 0 | 50 | 250 | 280 |
| syn-req-3 | 50 | 5 | 25 | 0 | 75 | 80 |

- Scan: five events, files[0].events=5, lines_read=8, records_seen=8.
  Import: added=3, unchanged=2, errors=0, conflicts=0. Repeated requestId/usage with
  only a different timestamp is treated as the same content.
- UTC 2026-09-24 summary: call_count=3, input_total_known=475, cache_read_known=65,
  cache_write_known=60, output_total_known=55, uncached_known=350, total_tokens_known=530.
- Per-record mapping: input_uncached=input_tokens (reported); input_total and total_tokens
  are derived; provider_id="anthropic", call_category="primary", schema_version="transcript-doc-1",
  parser_version="claude-transcript-doc1", model_attribution="request_field", origin_call_id=requestId.
  No source field establishes output_reasoning/source_total/cost/error_status; they remain NULL.
