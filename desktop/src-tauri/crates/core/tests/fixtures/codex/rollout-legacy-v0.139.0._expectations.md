# rollout-legacy-v0.139.0._expectations.md

Source: `<HOME>/.codex/sessions/…/rollout-<ts>-<UUID>.jsonl`,
81 original lines read locally. Extracted on 2026-09-26 with
`build/desktop-usage-validation/tools/extract-codex-legacy.mjs`. Numbers, booleans and null remain
unchanged. Strings default to REDACTED, IDs use stable anon-N mappings (31 IDs),
and cwd becomes `<PATH>`. Redaction checks found no remaining UUIDs, paths or bodies;
only record types, enum values, versions and model names remain as strings.

This represents the 0.139–0.151 legacy format, without token_usage_record. Per-call usage comes from
event_msg/token_count info.last_token_usage. Criteria and full verification are recorded in the
header of adapters/codex/versions/rollout_legacy.rs.

<a id="结构期望js-独立核算"></a>

## Structural expectations independently calculated with JavaScript

- All 81 lines parse; parseErrors=0.
- Record counts: session_meta=1, event_msg=41, response_item=31,
  turn_context=8; every context has model=codex-auto-review.
- Ten token_count records: the first has last==total and represents the first call; eight later
  records increase the cumulative total, and one has delta==0 with unchanged last, so it is skipped
  as a repeat.
- session_meta contains parent_thread_id and source.subagent, identifying a sub_agent session.

<a id="usage-数值期望按增量判据发出的事件求和"></a>

## Usage expectations summed over events emitted by the increment rules

| Metric | Expected value |
| --- | --- |
| Deduplicated model calls | 9 |
| Summed input_tokens | 343,705 |
| Summed cached_input_tokens | 253,824 |
| Summed cache_write_input_tokens | 0 |
| Summed output_tokens | 1,002 |
| Summed reasoning_output_tokens | 347 |
| Summed total_tokens | 344,707 |

All 9/9 records satisfy total=input+output, cached⊆input and reasoning⊆output.
The final token_count snapshot total=344,707 equals summed per-call usage:
reconciliation=matched, with no compaction.

<a id="分派期望v30"></a>

## Dispatch expectations (V30)

- Version 0.139.0 is registered as verified. detect returns
  Supported { format_version: Some("0.139.0"), basis: KnownVersion } and selects rollout_legacy.
  Events use parse_basis=known_version, parser_version=codex-rollout-legacy-1,
  and identity seq:{session}:{line}, because response_id is absent.
