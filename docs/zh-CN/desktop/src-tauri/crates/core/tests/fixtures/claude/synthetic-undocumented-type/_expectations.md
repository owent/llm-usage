# synthetic-undocumented-type._expectations.md（SYNTHETIC）

<a id="synthetic-undocumented-type_expectationsmd-synthetic"></a>

**本目录全部为合成样本（synthetic），不是真实会话提取。** 未文档化记录
type（file-history-snapshot）：V17 fail closed，整文件拒绝——本轮事件清空、
游标不推进，不猜格式。注意与 detect 分层：detect 只看首行（type=user →
Supported），fail closed 发生在扫描层。

<a id="scenario-and-manually-calculated-expectations"></a>

## 场景与期望（人工核算）

3 行：

- 第 1 行 user 正常（detect Supported）；
- 第 2 行 assistant syn-req-would 带 usage 10/5/0/0 —— 本可产 1 事件，
  但 fail closed 清空本轮事件；
- 第 3 行 type="file-history-snapshot" → 触发 `undocumented_record_type`，
  整文件拒绝。

期望：

- detect（直测本文件）：Supported(format=claude-transcript-jsonl,
  version=transcript-doc-1)；
- files[0]：status=pending、lines_read=3、records_seen=3、events=0；
- usage_events 0 行；ingestion_checkpoints 0 行；
- diagnostics：undocumented_record_type ×1（field=type、position="line 3"）；
  source_files.status="degraded"。
