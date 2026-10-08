# rollout-legacy-v0.139.0._expectations.md

来源：`<HOME>/.codex/sessions/…/rollout-<ts>-<UUID>.jsonl`（原始 81 行，本机实读），
2026-09-26 按脱敏流程提取（build/desktop-usage-validation/tools/extract-codex-legacy.mjs）：
数字/布尔/null 保留、字符串默认 REDACTED、ID 稳定映射 anon-N（31 个）、cwd → `<PATH>`。
泄漏核查：无 UUID/路径/正文残留；保留字符串仅为记录类型、枚举、版本与模型名。

本文件是 0.139–0.151 旧记录格式系列（无 `token_usage_record`，逐次用量依据 =
`event_msg/token_count` 的 `info.last_token_usage`）的代表样本；判据与
全量核验见 `adapters/codex/versions/rollout_legacy.rs` 文件头。

<a id="structural-expectations-independently-calculated-with-javascript"></a>

## 结构期望（JS 独立核算）

- 81 行全部可解析（parseErrors=0）。
- 记录类型计数：session_meta ×1、event_msg ×41、response_item ×31、
  turn_context ×8（model 均存在：`codex-auto-review`）。
- token_count ×10：首条 last==total（首调）；其后 8 次增量、1 次 delta==0
  且 last 未变（同一调用的重复上报，去重跳过）。
- session_meta：parent_thread_id 存在且 source.subagent 存在 ⇒ sub_agent 会话。

<a id="usage-expectations-summed-over-events-emitted-by-the-increment-rules"></a>

## usage 数值期望（按增量判据发出的事件求和）

| 指标 | 期望 |
| --- | --- |
| 模型调用（去重后发出） | 9 |
| input_tokens 合计 | 343,705 |
| cached_input_tokens 合计 | 253,824 |
| cache_write_input_tokens 合计 | 0 |
| output_tokens 合计 | 1,002 |
| reasoning_output_tokens 合计 | 347 |
| total_tokens 合计 | 344,707 |

包含关系（9/9 逐条成立）：total=input+output；cached⊆input；reasoning⊆output。
快照对账：token_count 最终快照 total = 344,707 = Σ逐次（matched，无 compaction）。

<a id="dispatch-expectations-v30"></a>

## 分派期望（V30）

- 版本 `0.139.0` 在注册表登记为已验证 ⇒ detect 返回
  Supported { format_version: Some("0.139.0"), basis: KnownVersion }，
  分派 `rollout_legacy`；事件 parse_basis = known_version、parser_version =
  codex-rollout-legacy-1、身份 seq:{session}:{行号}（无 response_id）。
