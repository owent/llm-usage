# subagent-agent-0._expectations.md（REAL，本机脱敏提取）

来源：`~/.kimi-code/sessions/<wd>/session_<uuid>/agents/agent-0/wire.jsonl`
（原始 1619 行；与 session-main 同会话的子代理 wire；protocol_version=1.5）。
提取时间：2026-09-25（白名单投影保留 455 行；正文 REDACTED、ID anon-N）。

## 结构期望

- 首行 `metadata`（protocol_version="1.5"）。
- usage.record ×150（全部 turn scope，无 session scope）；
  step.end 回声 ×150 全部带 usage（此文件回声计数==记录计数）。
- 模型：`kimi-code/k3-256k`（全部 150 条）。

## usage 数值期望（工具输出与源文件 jq 独立求和一致；roundtrip 复核相等）

| n | inputOther | output | inputCacheRead | inputCacheCreation |
| --- | --- | --- | --- | --- |
| 150 | 198,401 | 72,798 | 18,428,416 | 0 |

## 子代理对账（M0 结论在本机复证）

- 该文件在主线 `subagent.completed`（time=1790269234273）**之后仍在增长**
  （后续该子代理被重启复用直至 cancelled）：time≤completed 的前 22 条
  Σ = {66876, 4631, 1264384, 0}，与 completed.usage 逐字段相等。
- 因此 completed.usage 是**截至完成时刻的快照**，不是全文件 Σ
  （全文件 Σ 更大）；二者都不得再单独入账——本文件逐次记录是唯一计账源。

## 期望入库（人工核算）

150 事件全 sub_agent；input_uncached=198,401；cache_read=18,428,416；
cache_write=0（reported）；output_total=72,798；input_total=18,626,817（派生）；
total_tokens=18,699,615（派生）。

匿名 ID：302 个（anon-1…anon-302）。
