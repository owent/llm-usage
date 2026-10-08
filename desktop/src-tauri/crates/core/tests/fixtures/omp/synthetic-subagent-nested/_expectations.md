# synthetic-subagent-nested._expectations.md (SYNTHETIC)

<a id="synthetic-subagent-nested_expectationsmdsynthetic"></a>

**All samples in this directory are synthetic, rather than extracted native sessions.**
The layout follows observed local files: child Agent files are at
`<cwd>/<ts>_<parentUUID>/<Name>.jsonl`, with nested children one level deeper at
`<Name>/<Name>.<sub>.jsonl`. Parent identity comes from the nearest
`<ts>_<uuid>` ancestor: the UUID after its first underscore. Intermediate Agent names
do not identify additional parent sessions.

<a id="场景与期望"></a>

## Scenario and expectations

- Discover three files under one configured root:
  - Main `…d070.jsonl`, named `<ts>_<uuid>`: primary, parent_session_id=None;
    assistant syn-ma-1 (10/5/0/0/15).
  - Child `…d070/Research.jsonl`: sub_agent,
    parent_session_id=00000000-0000-7000-8000-00000000d070 from the directory name.
    Its session header has id=syn-omp-sub1; assistant syn-sa-1 (100/50/0/0/150).
  - Nested child `…d070/Research/Research.Compactor.jsonl`: sub_agent with the same
    parent_session_id from the nearest `<ts>_<uuid>` ancestor. Its own session header
    has id=syn-omp-sub2; assistant syn-sa-2 (200/60/40/0/300).
- Scan: three complete files, three events, added=3; primary=1, sub_agent=2.
- Summary (2026-01-05 UTC): call_count=3, input_total_known=350
  (derived: (10+0+0)+(100+0+0)+(200+40+0)), cache_read_known=40,
  output_total_known=115, total_tokens_known=465; three distinct sessions from their own headers.
