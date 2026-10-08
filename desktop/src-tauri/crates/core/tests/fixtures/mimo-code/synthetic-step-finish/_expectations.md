# synthetic-step-finish expectations (all synthetic)

<a id="synthetic-step-finish-期望全合成"></a>

Scenario: two sessions (main and parent_id-linked child), three step-finish parts and
one filtered text part. MiMo's session table has no tokens_* cumulative columns in fixed
session.sql.ts source, so no session-level comparison is available. message.agent_id
helps identify the format and does not classify calls.

Manually calculated expectations:

- Three `usage_events` (model_call; daily call_count=3 on 2026-06-13):
  - `mimo-code:part:part_syn_1`: primary, model_raw=mimo-latest, provider=mimo,
    time_basis=observed_at. input_uncached=1000, input_total=9500
    (derived: 1000+8000+500), output_total=250 (200+50), output_reasoning=50,
    cache_read=8000, cache_write=500, total_tokens=9750, source_total=9750 (matches),
    cost=12000 micro-USD (estimated).
  - `mimo-code:part:part_syn_2`: input_total=9500, output_total=80,
    total_tokens=9580 (derived); no total field, so source_total=NULL.
  - `mimo-code:part:part_syn_3`: sub_agent with nonempty parent_id;
    input_total=1300, output_total=50, total_tokens=1350, source_total=1350.
- Daily summary (2026-06-13 UTC): call_count=3, input_total=20300, cache_read=18000,
  cache_write=600, output=380, total_tokens=20680.
- Empty reconciliations: no cumulative comparison columns.
- Exactly one latest_fallback diagnostic: the registry is empty; implementation is
  based on documentation or source only.
- Repeat scanning leaves call_count=3.
