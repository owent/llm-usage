# synthetic-contract 期望（人工核算，全合成样本）

本目录为**合成** fixture（本机 not_found，无真实样本）；文件内 ID 均带 `syn-` 前缀。
布局：`<root>/tmp/<project_id>/chats/<sessionId>.jsonl`（ChatRecord JSONL）。

## 内容（8 行）

1. user（syn-u-1）——detect 首行锚点（type 四值 + uuid/sessionId/timestamp 齐全）。
2. assistant syn-a-1（primary）：usageMetadata {prompt 1000, candidates 50, cached 400,
   thoughts 10, toolUsePrompt 5, total 1050}。
3. assistant syn-a-2（isSidechain=true ⇒ sub_agent）：{prompt 2000, candidates 100,
   cached 0, total 2100}。
4. assistant syn-a-3（agentId="agent-x" ⇒ sub_agent）：{prompt 500, candidates 25,
   total 525}。
5. tool_result（无 usageMetadata）——正常形状，不产事件。
6. system/session_model——daemon 恢复绑定，不产事件。
7. system/goal_state（goal.tokensUsed=4,000,000,000 跨 turn 累计表）——明确忽略，
   绝不计入（计入即双计）。
8. system/chat_compression——压缩检查点控制记录，明确忽略。

## 期望

- 事件数 = 3（1 primary + 2 sub_agent），records_seen=8，files[0].status="complete"。
- 事件键 qwen:syn-a-1 / qwen:syn-a-2 / qwen:syn-a-3；schema_version="0.5.0"
  （record.version 逐条透传）；session_id="syn-sess-1"；provider_id 为 NULL
  （ChatRecord 无 provider 字段）。
- 映射：input_total=promptTokenCount；total_tokens=直报 totalTokenCount；
  cachedContentTokenCount→input_cache_read；thoughts/toolUsePrompt 不并入任何字段。
- 汇总（2026-01-05）：call_count=3；input_total_known=1000+2000+500=3500；
  output_total_known=50+100+25=175；cache_read_known=400+0=400（a3 未直报，不补零）；
  total_tokens_known=1050+2100+525=3675；cache_write_known=None。
- goal_state 的 4,000,000,000 不出现在任何汇总字段。
