# synthetic-undocumented-type 期望（人工核算，全合成样本）

messages[1] type="info"：真实存在但未文档化的消息类型，整文件 fail closed
（`undocumented_message_type`），不猜格式。

## 期望

- files[0].status="pending"；0 事件入库（fail closed 前已遍历的消息事件一并清空）；
  游标不推进。
- diagnostics 1 条 code=undocumented_message_type。
