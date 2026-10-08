# rollout-v0.153.0._expectations.md

Source: `<HOME>/.codex/sessions/…/rollout-<ts>-<UUID>.jsonl`,
276 original lines read locally. Extracted on 2026-09-25 with
`build/desktop-usage-validation/tools/extract-codex-versions.mjs`. Numbers, booleans and null remain
unchanged. Strings default to REDACTED, IDs use stable anon-N mappings (185 IDs),
and cwd becomes `<PATH>`. Redaction checks found no remaining UUIDs, paths or bodies;
only record types, enum values, versions and model names remain as strings.

<a id="结构期望jq-独立核算"></a>

## Structural expectations independently calculated with jq

- All 276 lines parse; parseErrors=0.
- Record counts: session_meta=1, event_msg=149, response_item=75,
  world_state=1, turn_context=25, token_usage_record=25.
- session_meta.cli_version=0.153.0 is the version reference for this sample.

<a id="usage-数值期望按-response_id-首次出现求和"></a>

## Usage expectations summed by first occurrence of response_id

| Metric | Expected value |
| --- | --- |
| Deduplicated model calls | 25 |
| Summed input_tokens | 932,041 |
| Summed cached_input_tokens | 844,544 |
| Summed cache_write_input_tokens | 0 |
| Summed output_tokens | 2,326 |
| Summed reasoning_output_tokens | 858 |
| Summed total_tokens | 934,367 |

All 25/25 records satisfy total=input+output, cached⊆input and reasoning⊆output.
The final token_count snapshot total=934,367 equals summed per-call usage:
reconciliation=matched, with no compaction.

<a id="分派期望v30"></a>

## Dispatch expectations (V30)

- Version 0.153.0 is registered as verified based on this sample.
  detect returns Supported { format_version: Some("0.153.0"), basis: KnownVersion }.
  Events use parse_basis=known_version; file status=active, rather than active_compat.
