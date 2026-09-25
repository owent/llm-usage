# synthetic-undocumented-type 期望（人工核算，全合成样本）

第 2 行 type="brand_new_record" 超出固定源码 ChatRecord 四值枚举：整文件 fail closed
（`undocumented_record_type`），第 1 行已解析的 assistant 事件一并清空。

## 期望

- files[0].status="pending"；0 事件入库（fail closed 清轮）；游标不推进（无 checkpoint）。
- diagnostics 每轮 1 条 code=undocumented_record_type；二次扫描仍 pending、再记 1 条。
