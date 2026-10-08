# rollout-v0.154.0-alpha.6.2._expectations.md

Source: `<HOME>/.codex/sessions/…/rollout-<ts>-<UUID>.jsonl`,
36 original lines read locally. Extracted on 2026-09-25 with
`build/desktop-usage-validation/tools/extract-codex-versions.mjs`. Numbers, booleans and null remain
unchanged. Strings default to REDACTED, IDs use stable anon-N mappings (52 IDs),
and cwd becomes `<PATH>`. Redaction checks found no remaining UUIDs, paths or bodies;
only record types, enum values, versions and model names remain as strings.
Preserved tool enum values include custom_tool_call/WebSearch.

<a id="结构期望jq-独立核算"></a>

## Structural expectations independently calculated with jq

- All 36 lines parse; parseErrors=0.
- Record counts: session_meta=1, event_msg=12, response_item=16,
  world_state=1, turn_context=1, token_usage_record=5.
- session_meta.cli_version=0.154.0-alpha.6.2 is the version reference for this sample.

<a id="usage-数值期望按-response_id-首次出现求和"></a>

## Usage expectations summed by first occurrence of response_id

| Metric | Expected value |
| --- | --- |
| Deduplicated model calls | 5 |
| Summed input_tokens | 115,209 |
| Summed cached_input_tokens | 97,152 |
| Summed cache_write_input_tokens | 0 |
| Summed output_tokens | 548 |
| Summed reasoning_output_tokens | 10 |
| Summed total_tokens | 115,757 |

All 5/5 records satisfy total=input+output, cached⊆input and reasoning⊆output.
The final token_count snapshot total=115,757 equals summed per-call usage:
reconciliation=matched, with no compaction.

<a id="分派期望v30"></a>

## Dispatch expectations (V30)

- Version 0.154.0-alpha.6.2 is registered as verified based on this sample.
  detect returns Supported { format_version: Some("0.154.0-alpha.6.2"), basis: KnownVersion }.
  Events use parse_basis=known_version; file status=active, rather than active_compat.
- This version had the most local files (46) and was a principal target of the fallback improvement.
