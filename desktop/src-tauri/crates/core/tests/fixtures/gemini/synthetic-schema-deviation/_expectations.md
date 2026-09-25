# synthetic-schema-deviation 期望（人工核算，全合成样本）

顶层形状偏离文档化 schema，整文件 fail closed（`session_schema_deviation`）：

- session-sid-not-string.json：`sessionId` 不是字符串（detect 只做键名指纹，扫描期核验类型）。
- session-msgs-not-array.json：`messages` 不是数组。

## 期望

- 两文件 files[*].status 均为 "pending"；0 事件入库；游标不推进。
- diagnostics 2 条 code=session_schema_deviation（每文件一条）。
