# synthetic-step-finish expectations (all synthetic)

<a id="synthetic-step-finish-期望全合成"></a>

Scenario: two sessions (main and parent_id-linked child), two assistant messages,
three step-finish parts and one text part, which has no usage and is excluded.
message.data.tokens aggregates a turn's message parts: **do not read or add it**.
Under projector applyUsage, five session.tokens_* columns sum current parts for comparison.

Manually calculated expectations:

- Three usage_events (model_call; daily call_count=3 on 2026-06-13):
  - opencode:part:part_syn_1: primary, model_raw=claude-sonnet-4-6, provider=anthropic,
    time_basis=observed_at, source_revision=1781337600500. input_uncached=1000,
    input_total=9500 (derived: 1000+8000+500), output_total=250 (200+50),
    output_reasoning=50, cache_read=8000, cache_write=500, total_tokens=9750,
    source_total=9750 (matches, no diagnostic), cost=12000 micro-USD (estimated).
  - opencode:part:part_syn_2: primary, same model; input_total=9500 (500+9000+0),
    output_total=80 (80+0), total_tokens=9580 (derived), source_total=NULL (no total field).
  - opencode:part:part_syn_3: sub_agent with nonempty session.parent_id,
    model_raw=gpt-5.2, provider=openai. input_total=1300 (200+1000+100),
    output_total=50 (40+10), total_tokens=1350, source_total=1350 (matches).
- Text part part_syn_text creates no event and is not counted in records.
- Daily summary (2026-06-13 UTC): call_count=3, input_total=20300, cache_read=18000,
  cache_write=600, output=380, total_tokens=20680.
- Two matched reconciliations:
  - ses_syn_main: part sum 9750+9580=19330 matches five columns
    1500+280+50+17000+500=19330.
  - ses_syn_sub: 1350=200+40+10+1000+100.
- Exactly one latest_fallback diagnostic: registry empty; implementation based on documentation/source.
- Repeat scanning leaves call_count=3: same key/content is unchanged.
