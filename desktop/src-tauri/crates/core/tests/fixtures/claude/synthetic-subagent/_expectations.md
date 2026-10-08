# synthetic-subagent._expectations.md (synthetic)

<a id="synthetic-subagent_expectationsmdsynthetic"></a>

**All files in this directory are synthetic test data, not extracts from real sessions.** Test both sub-agent associations: isSidechain=true in the main file,
and a separate transcript under `projects/<proj>/<session>/subagents/`.

<a id="场景与期望人工核算"></a>

## Scenario and manually calculated expectations

Main file `projects/proj/sess-1.jsonl`, three lines:

- One user, producing no event.
- syn-req-shared: usage 100/10/0/0 → input_total=100, total=110, primary.
- syn-req-side, isSidechain=true: 30/5/10/0 → input_total=40, total=45,
  sub_agent, parent_session_id=NULL because its path is outside subagents/.

Sub-agent file `projects/proj/sess-1/subagents/agent-a.jsonl`, three lines:

- One user used for detection, producing no event.
- syn-req-shared: same requestId and usage as the main file, with different uuid/timestamp;
  this is a cross-file repeat.
- syn-req-subonly: 70/15/20/5 → input_total=95, total=110.

Discovery compares path components: directory sess-1 precedes file sess-1.jsonl, so
subagents/agent-a.jsonl is scanned first. Its syn-req-shared is stored as sub_agent,
parent=sess-1. The main-file copy has the same key but different category/parent
(primary, parent NULL). Equal-priority records with different content conflict:
keep the stored value, set conflict=1 and record one update_conflict diagnostic.
**No double counting:** syn-req-shared counts once.

- Import: added=3, conflicts=1, unchanged=0; three usage_events rows.
- UTC 2026-09-24 summary: call_count=3, input_total_known=235, cache_read_known=30,
  cache_write_known=5, output_total_known=30, total_tokens_known=265, conflict_count=1.
- Categories: syn-req-shared=sub_agent/parent=sess-1/conflict=1 (first imported);
  syn-req-side=sub_agent/parent NULL; syn-req-subonly=sub_agent/parent=sess-1.
