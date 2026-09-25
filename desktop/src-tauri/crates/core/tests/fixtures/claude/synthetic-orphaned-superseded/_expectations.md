# synthetic-orphaned-superseded._expectations.md（SYNTHETIC）

**本目录全部为合成样本（synthetic），不是真实会话提取。** 覆盖 A01 文档的
被替换 transcript 变体：`<session>.orphaned-<ts>-<suffix>.jsonl` 与
`<session>.jsonl.superseded-<ts>`。后者文件名不以 `.jsonl` 结尾，验证发现
accept 规则显式覆盖 `.jsonl.superseded-`；变体内容与现行 transcript 重叠，
按稳定身份（req:{requestId}）upsert 防双计。

## 场景与期望（人工核算）

三个文件（发现顺序按文件名分量：sess-1.jsonl → sess-1.jsonl.superseded-…
→ sess-1.orphaned-…）：

| 文件 | 条目 | usage（in/out/cr/cw） | input_total | total |
| --- | --- | --- | --- | --- |
| sess-1.jsonl | syn-req-a | 10/2/0/0 | 10 | 12 |
| sess-1.jsonl.superseded-20260101T010000Z | syn-req-c | 30/6/0/10 | 40 | 46 |
| sess-1.orphaned-20260101T000000Z-a1.jsonl | syn-req-a（与现行逐字段相同） | 10/2/0/0 | 10 | 12 |
| 同上 | syn-req-b（变体独有） | 20/4/5/0 | 25 | 29 |

- 3 文件全部发现并扫描（files.len()=3、均 complete）：superseded 变体被
  accept 规则覆盖是本场景要点。
- 入库：added=3（req-a/req-c/req-b）、unchanged=1（orphaned 内 req-a 与现行
  完全相同 → 幂等 Keep）、conflicts=0；usage_events 共 3 行，req-a 只计一次。
- 汇总（UTC 2026-09-24）：call_count=3、input_total_known=75、
  cache_read_known=5、cache_write_known=10、output_total_known=12、
  total_tokens_known=87。
