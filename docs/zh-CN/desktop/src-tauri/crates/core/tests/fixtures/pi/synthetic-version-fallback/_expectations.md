# synthetic-version-fallback._expectations.md（SYNTHETIC）

<a id="synthetic-version-fallback_expectationsmd-synthetic"></a>

**本目录全部为合成样本（synthetic），不是真实会话提取。** 覆盖 V30 未收录
session version 的 latest_fallback 回退：session 头 version=4（未收录数值），
其后记录结构与 v3 相同（id/parentId/timestamp + assistant message.usage）。

<a id="scenario-and-expectations"></a>

## 场景与期望

3 行：session 头（syn-sess-vf，version=4）、assistant syn-vf-1（10/5/0/0/15）、
assistant syn-vf-2（20/10/0/0/30）。

- detect → Supported(format_version="4", basis=latest_fallback)，不因未收录拒绝。
- 扫描成功入库：2 事件均 primary；事件 parse_basis=latest_fallback、
  schema_version="4"（来源声明）；session_id=syn-sess-vf。
- 汇总（2026-01-05）：call_count=2、input_total_known=30（10+20）、
  cache_read_known=0、cache_write_known=0、output_total_known=15（5+10）、
  total_tokens_known=45（15+30）。
- 文件状态 active_compat；format_status JSON：basis=latest_fallback、
  found_version="4"、compat=unverified；diagnostics latest_fallback ×1
  （version 字段，兼容尝试可见）。
- 两条 usage 五字段齐全且 totalTokens 一致：无 usage_shape_deviation。
- 重复扫描：内容未变则跳过读取，added=0（兼容标记不会导致重复入库）。
