# synthetic-dual-mismatch 期望（全合成）

缺口：AI SDK inputTokens=1000（含缓存读）而 anthropic
  input_tokens=999+cache_read=400=1399 ≠ 1000 → dual_caliber_mismatch 诊断，AI
  SDK 主口径保留。

详见 tests/zcode_gaps_synthetic.rs 头部人工核算注释。
