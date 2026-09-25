# synthetic-bom 期望（人工核算，全合成样本）

UTF-8 BOM 头的会话 JSON：detect 与整写解析均剥 BOM，正常入账。

## 期望

- detect=Supported；files[0].status="complete"；事件数 = 1（syn-msg-1）。
- 汇总：call_count=1；input_total_known=100；output_total_known=20；
  total_tokens_known=120；无诊断。
