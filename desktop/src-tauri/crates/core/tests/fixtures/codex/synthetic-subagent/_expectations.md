# synthetic-subagent._expectations.md（SYNTHETIC）

**本目录全部为合成样本（synthetic），不是真实会话提取。** 覆盖子 Agent 会话类别
与宿主映射。结构仿 codex 0.155.0-alpha.16.3 rollout JSONL（真实核验结果：
rollout-single-call / rollout-49calls 的 auto-review 会话均带 parent_thread_id）。

## 场景与期望

- session_meta 含 `parent_thread_id="syn-parent-1"`、`originator="codex_vscode"`。
- 1 次调用（syn-resp-sub-1）：input 500、cached 100、cache_write 0、output 20、
  reasoning 5、total 520；快照 520，对账 matched。
- 期望：call_category = sub_agent；parent_session_id = syn-parent-1；
  host_application = vscode（originator 版本化映射）；agent = codex。
