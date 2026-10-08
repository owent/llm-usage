# synthetic-undocumented-say 期望（全合成）

<a id="synthetic-undocumented-say-expectations-synthetic"></a>

场景：`future_undocumented_kind` 不在 3.86.0 固定源码的完整枚举中。
普通 text、reasoning 与 ask 已有源码/真实样本依据，不再作为拒绝示例。

期望：

- detect 指纹成立（type/say 在场）⇒ Supported；
- 扫描遇未验证 say：status=pending、游标不推进、0 事件、
  诊断含 undocumented_say_kind（下轮确定性再拒）。
