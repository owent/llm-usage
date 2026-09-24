# synthetic-cache-write._expectations.md（SYNTHETIC）

**本目录全部为合成样本（synthetic），不是真实会话提取。** 覆盖 cache_write>0
（M0 真实 codex 样本该字段全 0，属已知缺口）。结构仿 codex 0.155.0-alpha.16.3。

## 场景与期望

- 1 次调用：input 1000、cached 400、**cache_write 100**、output 50、reasoning 10、
  total 1050；快照同值，对账 matched。
- 映射期望：input_cache_write = 100（reported）；input_uncached = 1000−400−100 =
  **500（derived）**；input_total = 1000（reported）；total_tokens = 1050（derived，
  source_total 1050 reported）。
- 该 `cache_write ⊆ input` 的映射假设仅由本合成样本定义；真实数据若矛盾
  （uncached<0）应进诊断而不是钳制。
