# rollout-v0.153.0._expectations.md

来源：`<HOME>/.codex/sessions/…/rollout-<ts>-<UUID>.jsonl`（原始 276 行，本机实读），
2026-09-25 按脱敏流程提取（build/desktop-usage-validation/tools/extract-codex-versions.mjs）：
数字/布尔/null 保留、字符串默认 REDACTED、ID 稳定映射 anon-N（185 个）、cwd → `<PATH>`。
泄漏核查：无 UUID/路径/正文残留；保留字符串仅为记录类型、枚举、版本与模型名。

## 结构期望（jq 独立核算）

- 276 行全部可解析（parseErrors=0）。
- 记录类型计数：session_meta ×1、event_msg ×149、response_item ×75、
  world_state ×1、turn_context ×25、token_usage_record ×25。
- session_meta.cli_version = `0.153.0`（本 fixture 的版本核验依据）。

## usage 数值期望（按 response_id 首次出现求和）

| 指标 | 期望 |
| --- | --- |
| 模型调用（token_usage_record 去重后） | 25 |
| input_tokens 合计 | 932,041 |
| cached_input_tokens 合计 | 844,544 |
| cache_write_input_tokens 合计 | 0 |
| output_tokens 合计 | 2,326 |
| reasoning_output_tokens 合计 | 858 |
| total_tokens 合计 | 934,367 |

包含关系（25/25 逐条成立）：total=input+output；cached⊆input；reasoning⊆output。
快照对账：token_count 最终快照 total = 934,367 = Σ逐次（matched，无 compaction）。

## 分派期望（V30）

- 版本 `0.153.0` 在注册表登记为已验证（本 fixture 核验结果）⇒ detect 返回
  Supported { format_version: Some("0.153.0"), basis: KnownVersion }；
  事件 parse_basis = known_version，文件状态 active（非 active_compat）。
