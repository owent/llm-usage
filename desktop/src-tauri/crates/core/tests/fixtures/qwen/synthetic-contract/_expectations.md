# synthetic-contract expectations (manual calculation, all synthetic)

<a id="synthetic-contract-期望人工核算全合成样本"></a>

This directory contains synthetic test data with syn- IDs. It was created when local
detection reported not_found; it contains no native samples. Layout:
`<root>/tmp/<project_id>/chats/<sessionId>.jsonl` (ChatRecord JSONL).

<a id="内容8-行"></a>

## Contents (eight lines)

1. user syn-u-1: first-line detection fields, with one of four types and complete
   uuid/sessionId/timestamp.
2. assistant syn-a-1, primary: usageMetadata {prompt 1000, candidates 50, cached 400,
   thoughts 10, toolUsePrompt 5, total 1050}.
3. assistant syn-a-2, isSidechain=true → sub_agent: {prompt 2000, candidates 100,
   cached 0, total 2100}.
4. assistant syn-a-3, agentId="agent-x" → sub_agent: {prompt 500, candidates 25, total 525}.
5. tool_result without usageMetadata: valid shape, no event.
6. system/session_model: daemon recovery binding, no event.
7. system/goal_state with cross-turn cumulative goal.tokensUsed=4,000,000,000:
   explicitly ignored, because adding it would duplicate usage.
8. system/chat_compression: compression checkpoint control record, explicitly ignored.

<a id="期望"></a>

## Expectations

- Three events: one primary and two sub_agent; records_seen=8, files[0].status="complete".
- Keys qwen:syn-a-1, qwen:syn-a-2, qwen:syn-a-3; schema_version="0.5.0" passed from
  each record.version; session_id="syn-sess-1"; provider_id=NULL because ChatRecord has no provider.
- input_total=promptTokenCount, total_tokens=reported totalTokenCount,
  cachedContentTokenCount→input_cache_read; thoughts/toolUsePrompt are not added to any field.
- Summary (2026-01-05): call_count=3, input_total_known=1000+2000+500=3500,
  output_total_known=50+100+25=175, cache_read_known=400+0=400 (a3 omits it; no filled zero),
  total_tokens_known=1050+2100+525=3675, cache_write_known=None.
- goal_state's 4,000,000,000 appears in no summary field.
