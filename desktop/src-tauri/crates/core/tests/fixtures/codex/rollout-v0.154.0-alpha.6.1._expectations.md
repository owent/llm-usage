# rollout-v0.154.0-alpha.6.1._expectations.md

Source: `<HOME>/.codex/sessions/…/rollout-<ts>-<UUID>.jsonl`,
37 original lines read locally. Extracted on 2026-09-25 with
`build/desktop-usage-validation/tools/extract-codex-versions.mjs`. Numbers, booleans and null remain
unchanged. Strings default to REDACTED, IDs use stable anon-N mappings (33 IDs),
and cwd becomes `<PATH>`. Redaction checks found no remaining UUIDs, paths or bodies;
only record types, enum values, versions and model names remain as strings.

<a id="结构期望jq-独立核算"></a>

## Structural expectations independently calculated with jq

- All 37 lines parse; parseErrors=0.
- Record counts: session_meta=1, event_msg=17, response_item=12,
  world_state=1, turn_context=3, token_usage_record=3.
- session_meta.cli_version=0.154.0-alpha.6.1 is the version reference for this sample.

<a id="usage-数值期望按-response_id-首次出现求和"></a>

## Usage expectations summed by first occurrence of response_id

| Metric | Expected value |
| --- | --- |
| Deduplicated model calls | 3 |
| Summed input_tokens | 69,373 |
| Summed cached_input_tokens | 48,384 |
| Summed cache_write_input_tokens | 0 |
| Summed output_tokens | 367 |
| Summed reasoning_output_tokens | 151 |
| Summed total_tokens | 69,740 |

All 3/3 records satisfy total=input+output, cached⊆input and reasoning⊆output.
The final token_count snapshot total=69,740 equals summed per-call usage:
reconciliation=matched, with no compaction.

<a id="分派期望v30"></a>

## Dispatch expectations (V30)

- Version 0.154.0-alpha.6.1 is registered as verified based on this sample.
  detect returns Supported { format_version: Some("0.154.0-alpha.6.1"), basis: KnownVersion }.
  Events use parse_basis=known_version; file status=active, rather than active_compat.
