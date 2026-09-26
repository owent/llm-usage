# rollout-legacy-v0.142.5._expectations.md

来源：`<HOME>/.codex/sessions/…/rollout-<ts>-<UUID>.jsonl`（原始 19 行，本机实读），
2026-09-26 按脱敏流程提取（build/desktop-usage-validation/tools/extract-codex-legacy.mjs）：
数字/布尔/null 保留、字符串默认 REDACTED、ID 稳定映射 anon-N（14 个）、cwd → `<PATH>`。
泄漏核查：无 UUID/路径/正文残留；保留字符串仅为记录类型、枚举、版本与模型名。

0.142.5 是本机旧载体数据量最大的版本（44 文件）；本样本为最小会话，
锁定 delta==0 且 last 未变 ⇒ 重复上报去重判据（2 条 token_count 只发 1 条事件）。

## 结构期望（JS 独立核算）

- 19 行全部可解析（parseErrors=0）。
- 记录类型计数：session_meta ×1、event_msg ×9、response_item ×7、
  turn_context ×2（model 均存在：`codex-auto-review`）。
- token_count ×2：首条 last==total（27,757）；第二条 total 不变、last 不变
  ⇒ 同一调用的重复上报，去重跳过。
- session_meta：parent_thread_id 存在且 source.subagent 存在 ⇒ sub_agent 会话。

## usage 数值期望（按增量判据发出的事件求和）

| 指标 | 期望 |
| --- | --- |
| 模型调用（去重后发出） | 1 |
| input_tokens 合计 | 27,648 |
| cached_input_tokens 合计 | 7,040 |
| cache_write_input_tokens 合计 | 0 |
| output_tokens 合计 | 109 |
| reasoning_output_tokens 合计 | 91 |
| total_tokens 合计 | 27,757 |

包含关系（1/1 成立）：total=input+output；cached⊆input；reasoning⊆output。
快照对账：token_count 最终快照 total = 27,757 = Σ逐次（matched，无 compaction）。

## 分派期望（V30）

- 版本 `0.142.5` 在注册表登记为已验证 ⇒ detect 返回
  Supported { format_version: Some("0.142.5"), basis: KnownVersion }，
  分派 `rollout_legacy`；事件 parse_basis = known_version、parser_version =
  codex-rollout-legacy-1、身份 seq:{session}:{行号}（无 response_id）。
