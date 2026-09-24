# synthetic-not-codex._expectations.md（SYNTHETIC）

**本目录全部为合成样本（synthetic），不是真实会话提取。** V17：未知格式必须
fail closed，不能返回“成功 0 条”。

## 场景与期望

- 首行不是 session_meta（仿 kimi wire 的 usage.record 形状）。
- 期望：detect 返回 UnknownFormat；运行报告文件状态 = unknown_format；
  事件数 = 0；diagnostics 含 unknown_format；source_files.status = unsupported。
