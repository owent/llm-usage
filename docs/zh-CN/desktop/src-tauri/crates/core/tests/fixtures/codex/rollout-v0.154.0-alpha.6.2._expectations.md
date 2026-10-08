# rollout-v0.154.0-alpha.6.2._expectations.md

来源：`<HOME>/.codex/sessions/…/rollout-<ts>-<UUID>.jsonl`（原始 36 行，本机实读），
2026-09-25 按脱敏流程提取（build/desktop-usage-validation/tools/extract-codex-versions.mjs）：
数字/布尔/null 保留、字符串默认 REDACTED、ID 稳定映射 anon-N（52 个）、cwd → `<PATH>`。
泄漏核查：无 UUID/路径/正文残留；保留字符串仅为记录类型、枚举、版本与模型名
（含 custom_tool_call/WebSearch 等工具类型枚举）。

<a id="structural-expectations-independently-calculated-with-jq"></a>

## 结构期望（jq 独立核算）

- 36 行全部可解析（parseErrors=0）。
- 记录类型计数：session_meta ×1、event_msg ×12、response_item ×16、
  world_state ×1、turn_context ×1、token_usage_record ×5。
- session_meta.cli_version = `0.154.0-alpha.6.2`（本样本 的版本核验依据）。

<a id="usage-expectations-summed-by-first-occurrence-of-response_id"></a>

## usage 数值期望（按 response_id 首次出现求和）

| 指标 | 期望 |
| --- | --- |
| 模型调用（token_usage_record 去重后） | 5 |
| input_tokens 合计 | 115,209 |
| cached_input_tokens 合计 | 97,152 |
| cache_write_input_tokens 合计 | 0 |
| output_tokens 合计 | 548 |
| reasoning_output_tokens 合计 | 10 |
| total_tokens 合计 | 115,757 |

包含关系（5/5 逐条成立）：total=input+output；cached⊆input；reasoning⊆output。
快照对账：token_count 最终快照 total = 115,757 = Σ逐次（matched，无 compaction）。

<a id="dispatch-expectations-v30"></a>

## 分派期望（V30）

- 版本 `0.154.0-alpha.6.2` 在注册表登记为已验证（本样本 核验结果）⇒ detect 返回
  Supported { format_version: Some("0.154.0-alpha.6.2"), basis: KnownVersion }；
  事件 parse_basis = known_version，文件状态 active（非 active_compat）。
- 本版本在本机数据量最大（46 个文件），是 fallback 收益的主要对象之一。
