# real-2.1.197-queue-metadata._expectations.md（REAL 脱敏提取）

<a id="real-21197-queue-metadata_expectationsmd-anonymized-native-sample"></a>

**来源：真实 Claude Code 2.1.197 会话转录（2026-09-30，WSL Debian，
`claude -p hi` 未登录首跑）。** ID/时间保留原形但已匿名化替换，正文为
占位；记录类型、字段结构与真实文件一致（8 行：queue-operation ×2、
user ×1、attachment ×3、assistant ×1（`<synthetic>`、usage 全 0）、
last-prompt ×1）。

<a id="native-format-observations-for-21197"></a>

## 本样本固化的真实格式事实（2.1.197）

1. 文件可以 `queue-operation`（enqueue/dequeue，携带 sessionId/content）
   开头——旧解析器将其判为未知格式 fail closed；
2. `attachment`（上下文附件元数据）与 `last-prompt`（会话指针）为
   非用量记录；
3. 未登录时的占位 assistant 响应 `message.model == "<synthetic>"`，
   `usage` 全 0（含 `server_tool_use`、`cache_creation` 扩展键）——
   无模型调用记录；
4. user 记录携带 `promptSource: "sdk"`、`entrypoint`、`permissionMode`。

<a id="manually-calculated-expectations"></a>

## 期望（人工核算）

- detect：Supported（首行 queue-operation 在放行集合内）；
- 扫描：complete、lines_read=8、records_seen=8、**events=0**；
- `<synthetic>` assistant 记 `synthetic_assistant_skipped` 诊断，不产事件
  （不把 0 用量算作模型调用）；
- queue-operation/attachment/last-prompt 跳过；若任一携带 usage 字段则
  整文件 fail closed（沿用 usage_on_unexpected_record_type）；
- 重复扫描不增量。
