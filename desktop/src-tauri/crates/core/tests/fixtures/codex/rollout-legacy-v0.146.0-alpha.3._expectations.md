# rollout-legacy-v0.146.0-alpha.3._expectations.md

来源：`<HOME>/.codex/sessions/…/rollout-<ts>-<UUID>.jsonl`（原始 454 行，本机实读），
2026-09-26 按脱敏流程提取（build/desktop-usage-validation/tools/extract-codex-legacy.mjs）：
数字/布尔/null 保留、字符串默认 REDACTED、ID 稳定映射 anon-N（675 个）、cwd → `<PATH>`。
泄漏核查：无 UUID/路径/正文残留；保留字符串仅为记录类型、枚举、版本与模型名。

本样本覆盖旧载体系列的三个已核验异常类别，是行为最丰富的代表：
压缩摘要回声（carried）、源端计数回退（regression）、多次调用区间（delta > last）。

## 结构期望（JS 独立核算）

- 454 行全部可解析（parseErrors=0）。
- 记录类型计数：session_meta ×1、event_msg ×155、response_item ×290、
  world_state ×2、turn_context ×5（model 均存在：`gpt-5.6-sol`）、compacted ×1。
- token_count ×86：85 次常规发出 + 1 次 delta==0 且 last 变化且紧随 compacted
  （压缩摘要调用回声 (0,0,0,0,0,16894)，源端排除出累计 total ⇒ 发事件但计入
  carried 对账排除）。
- 源端回退 ×3（实读 L338/L341/L444 附近：total 微降 319/607 等）⇒
  `snapshot_regression` 诊断 + 基线重定；其中伴随 last 实际变化的仍发事件。

## usage 数值期望（按增量判据发出的事件求和）

| 指标 | 期望 |
| --- | --- |
| 模型调用（regular） | 85 |
| 压缩摘要回声（carried） | 1 |
| input_tokens 合计（regular） | 11,169,365 |
| cached_input_tokens 合计（regular） | 10,530,048 |
| cache_write_input_tokens 合计（regular） | 0 |
| output_tokens 合计（regular） | 42,138 |
| reasoning_output_tokens 合计（regular） | 20,692 |
| total_tokens 合计（regular） | 11,211,503 |
| carried total_tokens 合计 | 16,894 |

包含关系（85/85 常规逐条成立）：total=input+output；cached⊆input；reasoning⊆output。
carried 回声天然 total != input+output（已核验的记录形状）⇒ 1 条 source_total_mismatch
矛盾诊断（可见、不隐藏）。

## 快照对账期望

- 最终快照 total = 10,911,604；detail_sum = 11,228,397（regular + carried）；
  carried_sum = 16,894；difference = +299,899 ⇒ **mismatch**（进诊断，不伪造数据）。
- 差异来源（核验）：源端两次计数回退重定基线后、回退伴随的真实调用按
  宁多勿漏发出（超出快照），以及 delta > last 的多次调用区间只回声最新一次
  （该方向本应少计）；两类残差一并在对账差异中显形。

## 分派期望（V30）

- 版本 `0.146.0-alpha.3` 在注册表登记为已验证 ⇒ detect 返回
  Supported { format_version: Some("0.146.0-alpha.3"), basis: KnownVersion }，
  分派 `rollout_legacy`；事件 parse_basis = known_version、parser_version =
  codex-rollout-legacy-1、身份 seq:{session}:{行号}（无 response_id）。
- reconcile_mismatch ⇒ 文件状态 degraded（事件照常入库）。
