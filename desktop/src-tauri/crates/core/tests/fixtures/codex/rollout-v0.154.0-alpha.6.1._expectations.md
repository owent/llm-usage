# rollout-v0.154.0-alpha.6.1._expectations.md

来源：`<HOME>/.codex/sessions/…/rollout-<ts>-<UUID>.jsonl`（原始 37 行，本机实读），
2026-09-25 按脱敏流程提取（build/desktop-usage-validation/tools/extract-codex-versions.mjs）：
数字/布尔/null 保留、字符串默认 REDACTED、ID 稳定映射 anon-N（33 个）、cwd → `<PATH>`。
泄漏核查：无 UUID/路径/正文残留；保留字符串仅为记录类型、枚举、版本与模型名。

## 结构期望（jq 独立核算）

- 37 行全部可解析（parseErrors=0）。
- 记录类型计数：session_meta ×1、event_msg ×17、response_item ×12、
  world_state ×1、turn_context ×3、token_usage_record ×3。
- session_meta.cli_version = `0.154.0-alpha.6.1`（本 fixture 的版本证据）。

## usage 数值期望（按 response_id 首次出现求和）

| 指标 | 期望 |
| --- | --- |
| 模型调用（token_usage_record 去重后） | 3 |
| input_tokens 合计 | 69,373 |
| cached_input_tokens 合计 | 48,384 |
| cache_write_input_tokens 合计 | 0 |
| output_tokens 合计 | 367 |
| reasoning_output_tokens 合计 | 151 |
| total_tokens 合计 | 69,740 |

包含关系（3/3 逐条成立）：total=input+output；cached⊆input；reasoning⊆output。
快照对账：token_count 最终快照 total = 69,740 = Σ逐次（matched，无 compaction）。

## 分派期望（V30）

- 版本 `0.154.0-alpha.6.1` 在注册表登记为已验证（本 fixture 证据）⇒ detect 返回
  Supported { format_version: Some("0.154.0-alpha.6.1"), basis: KnownVersion }；
  事件 parse_basis = known_version，文件状态 active（非 active_compat）。
