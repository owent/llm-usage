# synthetic-undocumented-say 期望（全合成）

场景：真实文件常见的 say="text" 流式文本消息——固定源码（f780647）只文档化
`api_req_started`/`api_req_finished`/`condense_context` 三种 say 载体，
text 等其余种类未枚举 ⇒ 按 V17 fail closed 整文件拒绝（与 cline 适配器同
保守合同），待真实样本扩展文档化集合。

期望：

- detect 指纹成立（type/say 在场）⇒ Supported；
- 扫描遇 say="text"：status=pending、游标不推进、0 事件、
  诊断含 undocumented_say_kind（下轮确定性再拒）。
