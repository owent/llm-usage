# synthetic-fork-inherited._expectations.md (SYNTHETIC)

<a id="synthetic-fork-inherited_expectationsmdsynthetic"></a>

**All samples in this directory are synthetic, rather than extracted native sessions.**
A fork copies entries verbatim into its session file, preserving type/id/parentId/timestamp.
Inherited entries are not new calls. None of 58 observed local files had a fork or
parentSession; this synthetic scenario uses the same rules as Pi.

<a id="场景与期望"></a>

## Scenario and expectations

- Source `…d060.jsonl`: five lines, session id=syn-omp-src, assistant syn-fa-1
  (100/50/0/0/150) and syn-fa-2 (200/60/40/0/300): two events.
- Fork `…d061.jsonl`: six lines, session id=syn-omp-fork,
  parentSession=syn-omp-src. Verbatim source L3–L5 copies model_change and both assistants;
  new assistant syn-fa-3 (10/5/0/0/15) gives three events.
- Two scans, source before fork: first added=2; second skips the unchanged source and
  reads three events from the fork.

<a id="已知偏差与-pi-同一定案"></a>

<a id="已知偏差与-pi-相同"></a>

## Known difference (same result as Pi)

Copied entry fields are identical, but event session_id/parent_session_id come from the
fork header. The same event key therefore has different content: report conflict and
retain the first-scanned event, rather than Keep for identical content. The resulting
statistics count once and retain the source session read first. Second outcome:
added=1 (syn-fa-3), conflicts=2, unchanged=0; two update_conflict diagnostics.

- syn-fa-3 keeps session_id=syn-omp-fork and parent_session_id=syn-omp-src.
- Summary (2026-01-05 UTC): call_count=3, input_total_known=350
  (derived: (100+0+0)+(200+40+0)+(10+0+0)), cache_read_known=40,
  cache_write_known=0, output_total_known=115, total_tokens_known=465.
