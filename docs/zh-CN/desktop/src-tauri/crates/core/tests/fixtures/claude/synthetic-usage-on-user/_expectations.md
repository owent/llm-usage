# synthetic-usage-on-user._expectations.md（SYNTHETIC）

<a id="synthetic-usage-on-user_expectationsmd-synthetic"></a>

**本目录全部为合成样本（synthetic），不是真实会话提取。** 非 usage 记录
（user）携带 usage 字段：格式偏离（V17 fail closed），整文件拒绝——本轮事件
清空、游标不推进（不提交 checkpoint），下轮确定性再拒；不猜格式。

<a id="scenario-and-manually-calculated-expectations"></a>

## 场景与期望（人工核算）

3 行：

- 第 1 行 user 正常（detect 识别条件：首行 type=user → Supported）；
- 第 2 行 assistant syn-req-would 带 usage 10/5/0/0 —— 本可产 1 事件，
  但 fail closed 清空本轮事件；
- 第 3 行 user 携带顶层 usage 键 → 第 3 行触发
  `usage_on_unexpected_record_type`，整文件拒绝。

每轮扫描期望：

- files[0]：status=pending（ScanStatus::Pending）、lines_read=3、
  records_seen=3、events=0；
- usage_events 恒为 0 行；ingestion_checkpoints 恒为 0 行（游标不推进）；
- diagnostics 每轮新增 1 行 usage_on_unexpected_record_type（field=type、
  position="line 3"）；source_files.status="degraded"。
- 同文件二次扫描：仍然 pending、0 事件、诊断再 +1（累计 2 行）——确定性再拒。
