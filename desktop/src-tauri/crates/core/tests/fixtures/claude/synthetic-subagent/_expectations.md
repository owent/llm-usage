# synthetic-subagent._expectations.md（SYNTHETIC）

**本目录全部为合成样本（synthetic），不是真实会话提取。** 覆盖子 Agent 两种
关联依据：主文件内 `isSidechain=true` 条目，与
`projects/<proj>/<session>/subagents/` 路径下的独立子 Agent transcript。

## 场景与期望（人工核算）

主文件 `projects/proj/sess-1.jsonl`（3 行）：

- user ×1（不产事件）；
- syn-req-shared：usage 100/10/0/0 → input_total=100、total=110，primary；
- syn-req-side（isSidechain=true）：30/5/10/0 → input_total=40、total=45，
  sub_agent，parent_session_id=NULL（路径不在 subagents/ 下）。

子 Agent 文件 `projects/proj/sess-1/subagents/agent-a.jsonl`（3 行）：

- user ×1（detect 锚点，不产事件）；
- syn-req-shared：与主文件同 requestId 同 usage（仅 uuid/时间戳不同）——
  跨文件重报；
- syn-req-subonly：70/15/20/5 → input_total=95、total=110。

发现排序按路径分量比较：`sess-1`（目录分量）< `sess-1.jsonl`（文件分量），
故 subagents/agent-a.jsonl 先扫、主文件后扫。syn-req-shared 先入的是子 Agent
副本（sub_agent、parent=sess-1）；主文件副本同键但分类/父会话不同（primary、
parent NULL）→ 同层级不同内容判 conflict，已存值保持，行标 conflict=1，
诊断 update_conflict ×1。**不双计**：syn-req-shared 只计一次。

- 入库：added=3、conflicts=1、unchanged=0；usage_events 共 3 行。
- 汇总（UTC 2026-09-24）：call_count=3、input_total_known=235、
  cache_read_known=30、cache_write_known=5、output_total_known=30、
  total_tokens_known=265、conflict_count=1。
- 分类核验：syn-req-shared=sub_agent/parent=sess-1/conflict=1（先入者）；
  syn-req-side=sub_agent/parent NULL；syn-req-subonly=sub_agent/parent=sess-1。
