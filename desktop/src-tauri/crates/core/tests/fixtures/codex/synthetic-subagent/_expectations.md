# synthetic-subagent._expectations.md (synthetic)

<a id="synthetic-subagent_expectationsmdsynthetic"></a>

**All files in this directory are synthetic test data, not extracts from real sessions.** Test sub-agent session category and host mapping using the Codex
0.155.0-alpha.16.3 rollout JSONL shape. Native verification found parent_thread_id
in both rollout-single-call and rollout-49calls auto-review sessions.

<a id="场景与期望"></a>

## Scenario and expectations

- session_meta contains `parent_thread_id="syn-parent-1"` and `originator="codex_vscode"`.
- One call, syn-resp-sub-1: input=500, cached=100, cache_write=0, output=20,
  reasoning=5, total=520. Snapshot=520; reconciliation=matched.
- call_category=sub_agent; parent_session_id=syn-parent-1; host_application=vscode
  through the version-specific originator mapping; agent=codex.
