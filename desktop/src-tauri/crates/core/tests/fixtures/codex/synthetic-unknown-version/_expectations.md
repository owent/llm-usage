# synthetic-unknown-version._expectations.md（SYNTHETIC）

**本目录全部为合成样本（synthetic），不是真实会话提取。** V17：未知版本必须
fail closed，不能返回“成功 0 条”。

## 场景与期望

- session_meta 的 `cli_version="0.999.0-synthetic"` 不在支持集内。
- 期望：detect 返回 UnsupportedVersion(found="0.999.0-synthetic")；
  运行报告文件状态 = unsupported_version；事件数 = 0；
  diagnostics 含 unsupported_version；source_files.status = unsupported。
- 该结果必须可区分于“支持但 0 条记录”。
