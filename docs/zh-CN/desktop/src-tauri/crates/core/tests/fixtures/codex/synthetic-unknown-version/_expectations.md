# synthetic-unknown-version._expectations.md（SYNTHETIC）

<a id="synthetic-unknown-version_expectationsmd-synthetic"></a>

**本目录全部为合成样本（synthetic），不是真实会话提取。** V17 新语义：
未知版本默认回退最新内置解析器，通过校验的数据带兼容标记入库，
不因版本号未收录直接拒绝；未知格式仍 fail closed。

<a id="scenario-and-expectations"></a>

## 场景与期望

- session_meta 的 `cli_version="0.999.0-synthetic"` 不在已验证注册表内，
  但记录结构与已验证版本相同（session_meta 携带会话 id + token_usage_record 六字段完整）。
- 期望：detect 返回 `Supported { format_version: Some("0.999.0-synthetic"),
  basis: LatestFallback }`；运行报告文件状态 = complete、事件数 = 1；
  diagnostics 含 1 条 `latest_fallback`（"using latest built-in parser;
  version compatibility unverified"）；source_files.status = active_compat，
  format_status JSON = { basis: latest_fallback, found_version: 0.999.0-synthetic,
  compat: unverified }；usage_events.parse_basis = latest_fallback。
- 事件数值：1 次调用，input=10、output=5、total=15（syn-resp-uv-1）。
- 重复扫描不增量（rescan_added=0）。
- 该结果必须可区分于"已验证版本的正常解析"（parse_basis/compat 标记）与
  "成功 0 条"（兼容尝试有诊断与状态）。
