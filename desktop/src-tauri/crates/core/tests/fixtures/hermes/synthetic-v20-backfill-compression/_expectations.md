# synthetic-v20-backfill-compression expectations (all synthetic)

<a id="synthetic-v20-backfill-compression-期望全合成"></a>

Scenario: a compressed parent session (end_reason='compression',
2026-09-20T09:00Z..12:00Z) has one v20 backfilled row. A historical summary was populated
as a model row with first/last_seen=NULL. Its compressed child, linked through
parent_session_id, has its own live row.

Epoch seconds: parent started=1789894800, ended=1789905600; child started=1789905660.
The live row has first=1789905660.5 and last=1789916000.0.

Manually calculated expectations:

- Two `source_aggregates` rows:
  - Backfill: interval_start_ms=NULL (unknown first_seen), interval_end_ms=1789905600000
    (session ended_at fallback), reported_call_count=NULL (default zero is unknown),
    input_uncached=500.
  - Child live row: interval 1789905660500..1789916000000, input_uncached=300,
    cache_read=20, reported_call_count=2.
- Compression does not duplicate inherited usage: input_uncached sum=500+300=**800**;
  the parent summary is not copied into the child.
- Default cache zeros remain unknown. Do not fill input_total/total_tokens. The known
  source-call subtotal is 2, without claiming a complete call count.
- Zero `usage_events` rows; all aggregate coverage is exclusive.
- The backfilled row does not establish that historical calls used legacy-model
  (v20 semantics). An api_call_count zero without a validity marker is unknown.
