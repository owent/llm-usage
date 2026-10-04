# synthetic-unknown-version._expectations.md（SYNTHETIC）

**本目录全部为合成样本（synthetic），不是真实会话提取。** 覆盖 V30 未知 session
版本策略：未收录数值默认回退最新内置解析器（latest_fallback），有固定版本源码依据
不兼容的形态才 fail closed。结构仿 pi session JSONL（固定源码
CURRENT_SESSION_VERSION=3；v1 无 version 字段）。

## 场景与期望

- `..._syn-sess-v4.jsonl`：首行 session 头 version=4（未收录数值，其后条目结构与
  v3 相同）。detect → Supported(format_version="4", basis=latest_fallback)。
- `..._syn-sess-legacy.jsonl`：首行 session 头无 version 字段（v1 形态；固定源码
  证实 v1/v2 时代不写该字段且落盘无 id/parentId）。
  detect → UnsupportedVersion(found=None, reason 注明 legacy)。

## 期望（V30 新语义）

- v4 文件：latest_fallback 成功扫描入库（1 事件 syn-v4-1：1/1/0/0/2），
  事件 parse_basis=latest_fallback、schema_version="4"；文件状态 active_compat；
  diagnostics 出现 latest_fallback ×1；重复扫描不增量（另见
  synthetic-version-fallback 的专项用例）。
- legacy 文件：已确认不兼容，整文件跳过（0 事件），status=unsupported_version；
  diagnostics 出现 unsupported_version ×1。
- 显式拒绝落诊断，不是静默的「成功 0 条」。
