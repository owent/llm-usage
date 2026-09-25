# synthetic-usage-on-user 期望（人工核算，全合成样本）

user 消息携带 tokens 对象：超出文档化形状（tokens 只属于 gemini 消息），
整文件 fail closed（`usage_on_unexpected_message_type`）。

## 期望

- files[0].status="pending"；0 事件入库；游标不推进（无 checkpoint）。
- diagnostics 每轮 1 条 code=usage_on_unexpected_message_type；二次扫描仍 pending、
  诊断再记 1 条（确定性再拒，非静默成功零）。
