# rollout-multimodel-full._expectations.md

来源：`<HOME>/.codex/sessions/2026/09/23/rollout-2026-09-23T23-27-28-<UUID>.jsonl`（约 6.7 MB，1457 行）。
提取时间：2026-09-24。多模型 + 压缩 + 中止场景的完整会话。

## 结构期望

- 1457 行全部可解析。cli_version=0.155.0-alpha.16.3，multi_agent_version=v2。
- 顶层类型：response_item ×709（reasoning 229、custom_tool_call 214、custom_tool_call_output 214、message 52）、
  event_msg ×511（item_completed 268、token_count 222、thread_settings_applied 9、task_started 6、
  task_complete 5、turn_aborted 1）、token_usage_record ×221、turn_context ×8、world_state ×5、
  compacted ×2、session_meta ×1。
- 模型变更：turn_context 序列 = gpt-6-astra（行 8/397/782/817）→ gpt-6-sol（行 1019/1136/1301/1445）。
  usage 记录本身不携带 model 字段，模型归属必须按 turn_context 位置关联。

## usage 数值期望

221 条 token_usage_record（逐次）合计：

| 字段 | 合计 |
| --- | --- |
| input_tokens | 32,061,426 |
| cached_input_tokens | 31,055,872 |
| cache_write_input_tokens | 0 |
| output_tokens | 143,381 |
| reasoning_output_tokens | 45,498 |
| total_tokens | 32,204,807 |

- 每条记录 total = input + output。
- **累计快照在 compaction 处重置**：最后一条 token_count.total_token_usage =
  {input 31,590,140, cached 30,596,992, cache_write 0, output 134,195, reasoning 45,498, total 31,724,335}，
  与逐次合计不相等（差约 47 万 input）。2 条 compacted 记录携带
  `payload.latest_token_usage_record.usage`（行 395：total 250,108；行 815：total 230,364）。
  结论：codex 真实总量应对逐次 token_usage_record 求和；累计快照只能按重置边界分段差分。
- turn_aborted ×1：存在中止场景记录（对应 usage 仍按已返回部分记账）。
