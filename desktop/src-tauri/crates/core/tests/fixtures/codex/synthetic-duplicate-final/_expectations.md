# synthetic-duplicate-final._expectations.md (synthetic)

<a id="synthetic-duplicate-final_expectationsmdsynthetic"></a>

**All files in this directory are synthetic test data, not extracts from real sessions.** Test a repeated final response for one request, which M0 native samples
did not contain. The shape follows Codex 0.155.0-alpha.16.3 rollout JSONL.

<a id="场景与期望"></a>

## Scenario and expectations

- Eight lines: session_meta, turn_context, task_started, three token_usage_record,
  token_count and task_complete. The first two usage records share response_id
  **syn-resp-1 and identical usage**, simulating a repeated final response.
  The third is a separate call, syn-resp-2.
- **Two model calls**; syn-resp-1 counts once. Summed input_total=3000, cached=400,
  cache_write=0, output=150, reasoning=10, total=3150.
- Final snapshot total=3150 equals summed per-call usage; carried=0, reconciliation=matched.
- All models are synthetic-model-a; category=primary, with no parent_thread_id.
