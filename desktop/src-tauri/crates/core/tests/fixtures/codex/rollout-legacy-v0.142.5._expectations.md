# rollout-legacy-v0.142.5._expectations.md

Source: `<HOME>/.codex/sessions/…/rollout-<ts>-<UUID>.jsonl`,
19 original lines read locally. Extracted on 2026-09-26 with
`build/desktop-usage-validation/tools/extract-codex-legacy.mjs`. Numbers, booleans and null remain
unchanged. Strings default to REDACTED, IDs use stable anon-N mappings (14 IDs),
and cwd becomes `<PATH>`. Redaction checks found no remaining UUIDs, paths or bodies;
only record types, enum values, versions and model names remain as strings.

Version 0.142.5 had the most local legacy-format files (44). This minimal session checks duplicate
detection when delta==0 and last is unchanged: two token_count records produce one event.

<a id="结构期望js-独立核算"></a>

## Structural expectations independently calculated with JavaScript

- All 19 lines parse; parseErrors=0.
- Record counts: session_meta=1, event_msg=9, response_item=7,
  turn_context=2; every context has model=codex-auto-review.
- Two token_count records: the first has last==total (27,757); the second leaves both total and last unchanged and is skipped as a repeat of the same call.
- session_meta contains parent_thread_id and source.subagent, identifying a sub_agent session.

<a id="usage-数值期望按增量判据发出的事件求和"></a>

## Usage expectations summed over events emitted by the increment rules

| Metric | Expected value |
| --- | --- |
| Deduplicated model calls | 1 |
| Summed input_tokens | 27,648 |
| Summed cached_input_tokens | 7,040 |
| Summed cache_write_input_tokens | 0 |
| Summed output_tokens | 109 |
| Summed reasoning_output_tokens | 91 |
| Summed total_tokens | 27,757 |

All 1/1 records satisfy total=input+output, cached⊆input and reasoning⊆output.
The final token_count snapshot total=27,757 equals summed per-call usage:
reconciliation=matched, with no compaction.

<a id="分派期望v30"></a>

## Dispatch expectations (V30)

- Version 0.142.5 is registered as verified. detect returns
  Supported { format_version: Some("0.142.5"), basis: KnownVersion } and selects rollout_legacy.
  Events use parse_basis=known_version, parser_version=codex-rollout-legacy-1,
  and identity seq:{session}:{line}, because response_id is absent.
