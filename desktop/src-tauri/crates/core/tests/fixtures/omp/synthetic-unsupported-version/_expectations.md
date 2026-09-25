# synthetic-unsupported-version._expectations.md（SYNTHETIC）

**本目录全部为合成样本（synthetic），不是真实会话提取。** V17：未知 session
version 必须 fail closed，不能返回“成功 0 条”。

## 场景与期望

- `...d020.jsonl`：session 头 `version=4`（本机证据仅支持 3）⇒ detect
  UnsupportedVersion{format=omp-session-jsonl, found="4"}。
- `...d021.jsonl`：session 头无 `version` 字段（legacy 形状）⇒ detect
  UnsupportedVersion{found="legacy (no version field)"}。
- 整轮：两文件均 status="unsupported_version"、0 事件；diagnostics 表
  `unsupported_version` ×2（框架层逐文件一条）；usage_events 为空；
  source_files 两行均 status="unsupported"。
