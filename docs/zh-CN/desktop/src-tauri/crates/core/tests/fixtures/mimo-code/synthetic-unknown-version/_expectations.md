# synthetic-unknown-version 期望（全合成）

<a id="synthetic-unknown-version-expectations-synthetic"></a>

场景：session.version="9.9.9-syn-uncollected"（未收录版本）——latest_fallback
兼容尝试（V17/V30：不因版本未收录直接拒绝）。

期望（人工核算）：

- detect：Supported，format_version="9.9.9-syn-uncollected"，
  basis=latest_fallback；format_status compat=unverified。
- 诊断含 1 条 latest_fallback。
- 数据照常入库：`usage_events` 1 条 `mimo-code:part:part_syn_future_1`
  （input_total=9500、output_total=250、total_tokens=9750、source_total=9750）；
  2026-06-13 日汇总 call_count=1、input_total=9500、total_tokens=9750。
- reconciliations 为空。
