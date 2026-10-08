# rollout-legacy-v0.146.0-alpha.3._expectations.md

Source: `<HOME>/.codex/sessions/…/rollout-<ts>-<UUID>.jsonl`,
454 original lines read locally. Extracted on 2026-09-26 with
`build/desktop-usage-validation/tools/extract-codex-legacy.mjs`. Numbers, booleans and null remain
unchanged. Strings default to REDACTED, IDs use stable anon-N mappings (675 IDs),
and cwd becomes `<PATH>`. Redaction checks found no remaining UUIDs, paths or bodies;
only record types, enum values, versions and model names remain as strings.

This sample covers three verified legacy-format cases: carried compaction usage,
source counter regression, and intervals spanning multiple calls (delta>last).

<a id="结构期望js-独立核算"></a>

## Structural expectations independently calculated with JavaScript

- All 454 lines parse; parseErrors=0.
- Record counts: session_meta=1, event_msg=155, response_item=290, world_state=2,
  turn_context=5 (all model=gpt-5.6-sol), compacted=1.
- 86 token_count records: 85 regular events and one with delta==0, changed last and
  an immediately preceding compacted record. The carried compaction call has usage
  (0,0,0,0,0,16894). The source excludes it from cumulative total, so emit the event
  and classify it as carried for reconciliation exclusion.
- Three source counter regressions, observed near native lines L338/L341/L444,
  produce snapshot_regression and reset the baseline. Cumulative total decreases
  include 319/607. If last actually changes, the corresponding event is still emitted.

<a id="usage-数值期望按增量判据发出的事件求和"></a>

## Usage expectations summed over events emitted by the increment rules

| Metric | Expected value |
| --- | --- |
| Regular model calls | 85 |
| Carried compaction events | 1 |
| Regular summed input_tokens | 11,169,365 |
| Regular summed cached_input_tokens | 10,530,048 |
| Regular summed cache_write_input_tokens | 0 |
| Regular summed output_tokens | 42,138 |
| Regular summed reasoning_output_tokens | 20,692 |
| Regular summed total_tokens | 11,211,503 |
| Carried summed total_tokens | 16,894 |

All 85 regular records satisfy total=input+output, cached⊆input and reasoning⊆output.
The verified carried record has total!=input+output and produces one visible
source_total_mismatch diagnostic.

<a id="快照对账期望"></a>

## Snapshot reconciliation expectations

- Final snapshot total=10,911,604; detail_sum=11,228,397 (regular+carried),
  carried_sum=16,894, difference=+299,899. Reconciliation is **mismatch**;
  retain a diagnostic without inventing data.
- Verified difference causes: baseline resets after two decreases in cumulative total
  retain the accompanying real calls to avoid omissions, exceeding the snapshot.
  Multi-call intervals with delta>last expose only the latest call, which undercounts
  the interval. Both residuals remain visible in the reconciliation difference.

<a id="分派期望v30"></a>

## Dispatch expectations (V30)

- Version 0.146.0-alpha.3 is registered as verified. detect returns
  Supported { format_version: Some("0.146.0-alpha.3"), basis: KnownVersion } and selects rollout_legacy.
  Events use parse_basis=known_version, parser_version=codex-rollout-legacy-1,
  and identity seq:{session}:{line}, because response_id is absent.
- reconcile_mismatch makes file status degraded; events are still imported.
