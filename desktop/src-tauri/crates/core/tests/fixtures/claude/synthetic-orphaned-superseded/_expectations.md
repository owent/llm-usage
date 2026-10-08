# synthetic-orphaned-superseded._expectations.md (synthetic)

<a id="synthetic-orphaned-superseded_expectationsmdsynthetic"></a>

**All files in this directory are synthetic test data, not extracts from real sessions.** Test the replaced transcript variants documented by A01:
`<session>.orphaned-<ts>-<suffix>.jsonl` and `<session>.jsonl.superseded-<ts>`.
The second filename does not end in .jsonl; discovery must explicitly accept
.jsonl.superseded-. Variants overlap the current transcript. Upsert by stable
`req:{requestId}` prevents double counting.

<a id="场景与期望人工核算"></a>

## Scenario and manually calculated expectations

Three files, discovered in filename-component order: sess-1.jsonl, then
sess-1.jsonl.superseded-…, then sess-1.orphaned-….

| File | Record | usage (in/out/cr/cw) | input_total | total |
| --- | --- | --- | --- | --- |
| sess-1.jsonl | syn-req-a | 10/2/0/0 | 10 | 12 |
| sess-1.jsonl.superseded-20260101T010000Z | syn-req-c | 30/6/0/10 | 40 | 46 |
| sess-1.orphaned-20260101T000000Z-a1.jsonl | syn-req-a (all fields match current record) | 10/2/0/0 | 10 | 12 |
| Same orphaned file | syn-req-b (variant-only record) | 20/4/5/0 | 25 | 29 |

- Discover and scan all three: files.len()=3, all complete. Acceptance of the superseded
  variant is the behavior this scenario checks.
- Import: added=3 (req-a/req-c/req-b), unchanged=1, conflicts=0. The identical orphaned
  req-a is kept without change. usage_events has three rows; req-a counts once.
- UTC 2026-09-24 summary: call_count=3, input_total_known=75, cache_read_known=5,
  cache_write_known=10, output_total_known=12, total_tokens_known=87.
