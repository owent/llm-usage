# synthetic-usage-on-tool-result 期望（人工核算，全合成样本）

第 3 行 tool_result 携带 usageMetadata：非 assistant 记录带用量属格式偏离，整文件
fail closed（`usage_on_unexpected_record_type`），第 2 行已解析的 assistant 事件清空。

## 期望

- files[0].status="pending"；0 事件入库；游标不推进（无 checkpoint）。
- diagnostics 每轮 1 条 code=usage_on_unexpected_record_type；二次扫描仍 pending。
