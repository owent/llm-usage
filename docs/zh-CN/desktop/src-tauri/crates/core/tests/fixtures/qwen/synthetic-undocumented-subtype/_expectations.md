# synthetic-undocumented-subtype 期望（人工核算，全合成样本）

<a id="synthetic-undocumented-subtype-expectations-manually-calculated-synthetic"></a>

第 2 行 assistant 的 subtype="brand_new_subtype" 超出固定源码 33 值枚举：整文件
fail closed（`undocumented_record_subtype`）。

<a id="expectations"></a>

## 期望

- files[0].status="pending"；0 事件入库；游标不推进。
- diagnostics 1 条 code=undocumented_record_subtype。
