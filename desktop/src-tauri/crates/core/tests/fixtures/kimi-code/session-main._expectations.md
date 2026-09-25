# session-main._expectations.md（REAL，本机脱敏提取）

来源：`~/.kimi-code/sessions/<wd>/session_<uuid>/agents/main/wire.jsonl`
（原始 4225 行；Kimi Code desktop 1.0.3，wire protocol_version=1.5）。
提取时间：2026-09-25（tools/extract-kimi.mjs 白名单投影，保留 1141 行，
非用量类型按 droppedByType 计数丢弃；正文 REDACTED、ID anon-N）。
快照时点：提取时文件已停止增长（会话结束）。

## 结构期望

- 首行 `metadata`（protocol_version="1.5"）；重建 JSONL 后 1141 行全部可解析。
- 保留计数：usage.record ×372（turn ×369 + session ×3）、llm.request ×221、
  context.append_loop_event(step.end 回声) ×372（其中带 usage 回声 ×368）、
  subagent.spawned/started/failed ×21、subagent.completed ×1（agent-0）、
  full_compaction.begin/complete ×6。
- 模型分布（按 usage.record 计）：`Kimi For Coding - Backup/k3-256k` ×324、
  `kimi-code/k3-256k` ×27、`kimi-code/kimi-for-coding` ×21。

## usage 数值期望（工具输出与源文件 jq 独立求和一致；roundtrip 复核相等）

| scope | n | inputOther | output | inputCacheRead | inputCacheCreation |
| --- | --- | --- | --- | --- | --- |
| turn | 369 | 1,002,786 | 245,794 | 46,991,104 | 0 |
| session | 3 | 503,907 | 11,579 | 75,264 | 0 |
| Σ | 372 | 1,506,693 | 257,373 | 47,066,368 | 0 |

step.end 回声 Σ（368 条）：inputOther 996,163 / output 245,368 /
inputCacheRead 46,858,496 / creation 0 —— 是 turn 记录的**子集**
（368<369：1 条 turn 记录因 swarm 中断无回声，回声绝不单独计账）。

## 对账与防双计

- `subagent.completed`（agent-0，time=1790269234273）usage
  {66876, 4631, 1264384, 0}：与 subagent-agent-0 fixture 中 time≤该时刻的
  22 条 wire 逐次 Σ **逐字段相等**（独立 jq 核验）。
- completed.usage **不产生事件**（子代理 wire 已逐次入账，计入即双计）；
  session scope（compaction 摘要调用）按 auxiliary 计入（独立调用）。

## 期望入库（人工核算）

372 事件：369 primary + 3 auxiliary；input_uncached=1,506,693；
cache_read=47,066,368；cache_write=0（reported）；output_total=257,373；
input_total=48,573,061（派生）；total_tokens=48,830,434（派生，无 source total）。

匿名 ID：746 个（anon-1…anon-746）。
