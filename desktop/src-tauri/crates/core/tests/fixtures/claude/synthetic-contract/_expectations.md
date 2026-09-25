# synthetic-contract._expectations.md（SYNTHETIC）

**本目录全部为合成样本（synthetic），不是真实会话提取。** Claude Code 本机
not_found（M2-C 探测无真实样本）；结构按 A01 文档口径合成：
`projects/<project>/<session>.jsonl`，assistant 条目携带
`message.usage` 四字段（全部必填），同一 API 响应按内容块拆为多条目、
按 `requestId` upsert 去重。文件内 ID 均带 `syn-` 前缀。

## 场景与期望（人工核算）

8 行：user ×2、system ×1（均非 usage 载体、不带 usage，不产事件）、
assistant ×5。assistant 条目：

| requestId | input | output | cache_read | cache_creation | input_total(derived) | total(derived) |
| --- | --- | --- | --- | --- | --- | --- |
| syn-req-1（第 3/4/5 行同 requestId 同 usage，去重后 1 调用） | 100 | 20 | 40 | 10 | 150 | 170 |
| syn-req-2 | 200 | 30 | 0 | 50 | 250 | 280 |
| syn-req-3 | 50 | 5 | 25 | 0 | 75 | 80 |

- 扫描产出 5 条事件（files[0].events=5，lines_read=8，records_seen=8）；
  入库：added=3、unchanged=2（同 requestId 重报仅时间戳不同，视为同一内容）、
  errors=0、conflicts=0。
- 汇总（UTC 2026-09-24）：call_count=3、input_total_known=475、
  cache_read_known=65、cache_write_known=60、output_total_known=55、
  uncached_known=350、total_tokens_known=530。
- 逐行映射：input_uncached=input_tokens（reported）、input_total 与
  total_tokens 为 derived；provider_id="anthropic"、call_category="primary"、
  schema_version="transcript-doc-1"、parser_version="claude-transcript-doc1"、
  model_attribution="request_field"、origin_call_id=requestId；
  output_reasoning/source_total/cost/error_status 均无证据保持 NULL。
