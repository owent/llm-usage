# conv-main._expectations.md（REAL，本机脱敏提取）

来源：Kimi Work 内嵌 kimi-code home
`…/Kimi/share/daimon-share/daimon/runtime/kimi-code/home/sessions/<wd>/conv-<hexid>/agents/main/wire.jsonl`
（原始 363 行；wire protocol_version=1.4；宿主 daimon，state.json
createdBy=daimon-kernel-adapter 佐证产品身份）。
提取时间：2026-09-25（tools/extract-kimi.mjs 白名单投影保留 115 行；
正文 REDACTED、ID anon-N）。

## 结构期望

- 首行 `metadata`（protocol_version="1.4"）。
- usage.record ×38（全部 turn scope、无 agentId 字段——1.4 主线 wire 不带
  agentId，代理身份来自目录 agents/main）；step.end 回声 ×38 全部带 usage。
- 模型：`k28-agent-preview` ×38；llm.request provider=`kimi`。

## usage 数值期望（工具输出与源文件 jq 独立求和一致；roundtrip 复核相等）

| n | inputOther | output | inputCacheRead | inputCacheCreation |
| --- | --- | --- | --- | --- |
| 38 | 47,574 | 18,658 | 1,310,720 | 0 |

回声 Σ 与记录 Σ 完全相等（38=38，此会话无中断步）。

## 与 Kimi Code（1.5）的实测差异（独立 fixture 证据，不因内核同名合并）

- 目录布局：`conv-<hexid>`（而非 `session_<uuid>`）；本会话无子代理目录。
- usage.record 无 agentId（1.5 主线带）；模型为裸 id（非 alias/model 组合）。

## 期望入库（人工核算）

38 事件全 primary；input_uncached=47,574；cache_read=1,310,720；
cache_write=0（reported）；output_total=18,658；input_total=1,358,294（派生）；
total_tokens=1,376,952（派生）。

匿名 ID：76 个（anon-1…anon-76）。
