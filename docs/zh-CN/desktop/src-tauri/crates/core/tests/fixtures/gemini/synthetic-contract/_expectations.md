# synthetic-contract 期望（人工核算，全合成样本）

<a id="synthetic-contract-expectations-manually-calculated-synthetic"></a>

本目录为**合成** fixture（本机 not_found，无真实样本）；文件内 ID 均带 `syn-` 前缀。
布局：`<root>/tmp/<project_hash>/chats/session-*.json`（整写 JSON，非 JSONL）。

<a id="contents"></a>

## 内容

- 4 条 messages：1 条 user（无 tokens）+ 2 条 gemini 带 tokens + 1 条 gemini 无 tokens。
- `syn-msg-1`：tokens {input 1000, output 50, cached 400, thoughts 10, tool 5, total 1050}。
- `syn-msg-2`：tokens {input 2000, output 100, total 2100}（cached/thoughts/tool 缺省，各自可选）。
- `syn-msg-3`：无 tokens，不产事件。

<a id="expectations"></a>

## 期望

- 事件数 = 2（syn-msg-1、syn-msg-2），records_seen = 4。
- 映射：input_total=tokens.input（reported）；total_tokens=直报 tokens.total（reported）；
  cached→input_cache_read（reported）；thoughts/tool 不并入任何字段（output_reasoning、
  input_uncached 均为 NULL）；source_total 未知。
- 汇总（2026-01-05）：call_count=2；input_total_known=3000；output_total_known=150；
  cache_read_known=400；total_tokens_known=3150；cache_write_known=None（格式内无该字段，
  未知不补零）。
- provider_id="google"；session_id="syn-sess-g1"；model_raw="gemini-3.0-flash"；
  schema_version="session-doc-1"；call_category="primary"；agent="gemini-cli"。
- 事件键：gemini:syn-sess-g1:syn-msg-1 / gemini:syn-sess-g1:syn-msg-2。

核算方式：tokens 六键逐消息对照上表求和；1050+2100=3150，1000+2000=3000，
50+100=150，cached 仅 syn-msg-1 直报 400。
