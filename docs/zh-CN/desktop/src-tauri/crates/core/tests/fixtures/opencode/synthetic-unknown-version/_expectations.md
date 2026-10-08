# synthetic-unknown-version 期望（全合成）

<a id="synthetic-unknown-version-expectations-synthetic"></a>

场景：session.version="9.9.9-syn-uncollected"（未收录版本）——按 V17/V30
未知版本兼容规则走 latest_fallback（带兼容标记，不直接拒绝）。

期望（人工核算）：

- detect：Supported，format_version="9.9.9-syn-uncollected"，
  basis=latest_fallback；`source_files.format_status` 的
  basis=latest_fallback、compat=unverified。
- 诊断含 1 条 latest_fallback。
- 数据照常入库：`usage_events` 1 条 `opencode:part:part_syn_future_1`
  （input_total=9500、output_total=250、total_tokens=9750、source_total=9750）；
  2026-06-13 日汇总 call_count=1、input_total=9500、total_tokens=9750。
- 对账 matched（9750 = 1000+200+50+8000+500）。
