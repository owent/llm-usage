# synthetic-duplicate-final._expectations.md（SYNTHETIC）

**本目录全部为合成样本（synthetic），不是真实会话提取。** 用于覆盖 M0 真实样本
缺失的场景：同一请求的重复 final。结构仿 codex 0.155.0-alpha.16.3 rollout JSONL。

## 场景与期望

- 8 行：session_meta、turn_context、task_started、token_usage_record ×3（前两条
  **同一 response_id `syn-resp-1`、完全相同的 usage，模拟重复 final**；第三条为独立
  调用 `syn-resp-2`）、token_count、task_complete。
- 期望：模型调用数 = **2**（syn-resp-1 只计一次）；input_total 合计 3000、
  cached 400、cache_write 0、output 150、reasoning 10、total 3150。
- 最终快照 total=3150 == Σ逐次；carried=0；对账 matched。
- 模型均为 synthetic-model-a；category=primary（无 parent_thread_id）。
