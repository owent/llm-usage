# synthetic-fork-inherited._expectations.md (SYNTHETIC)

<a id="synthetic-fork-inherited_expectationsmdsynthetic"></a>

**All samples in this directory are synthetic, rather than extracted native sessions.**
This covers cross-file deduplication of inherited fork entries. Structure follows Pi
session JSONL v3. Fixed session-manager.ts source copies **verbatim** all non-header
entries into the new file, preserving id/parentId/timestamp; the new header records parentSession.

<a id="场景与期望"></a>

## Scenario and expectations

- Source `..._syn-sess-src.jsonl`, four lines: session header id=syn-sess-src,
  model_change, assistant syn-fa-1 (100/50/0/0/150), assistant syn-fa-2 (200/60/40/0/300).
- Fork `..._syn-sess-fork.jsonl`, five lines: header id=syn-sess-fork,
  parentSession=syn-sess-src, verbatim source L2–L4, then new assistant syn-fa-3
  (10/5/0/0/15; timestamp 2026-01-05T11:00:01.000Z).

<a id="期望"></a>

## Expectations

- Two scans, source alone followed by the fork, give three calls: syn-fa-1, syn-fa-2,
  syn-fa-3. Copies do not count twice.
- Summary (2026-01-05): input_total_known=350 (derived input+cacheRead+cacheWrite:
  100+240+10), cache_read_known=40, output_total_known=115 (50+60+5),
  total_tokens_known=465 (150+300+15).
- New syn-fa-3: session_id=syn-sess-fork, parent_session_id=syn-sess-src.
- Event key: `pi:message:<id>:<parentId>:<timestamp>`; absent parentId uses "-".

<a id="已知偏差实测核验"></a>

## Known difference (measured)

Copied entries acquire the fork's session_id/parent_session_id from its local header.
They therefore have the same key but different content from stored source events:
**conflict, retaining the first-scanned event**, rather than the capability declaration's
Keep for identical content. Statistics still count once and retain the first event;
outcome.conflicts=2 and both stored events have conflict=1. See gap-test comments and the delivery report.
