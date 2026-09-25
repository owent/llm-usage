# synthetic-unknown-version._expectations.md（SYNTHETIC）

**本目录全部为合成样本（synthetic），不是真实会话提取。** 覆盖 V17：未知 session
版本 fail closed，不猜格式。结构仿 pi session JSONL（固定源码
CURRENT_SESSION_VERSION=3；v1 无 version 字段）。

## 场景与期望

- `..._syn-sess-v4.jsonl`：首行 session 头 version=4（其后带一条若被解析会产生事件的
  assistant，用于证明 fail closed 后 0 事件）。detect → UnsupportedVersion(found="4")。
- `..._syn-sess-legacy.jsonl`：首行 session 头无 version 字段（v1 形态）。
  detect → UnsupportedVersion(found="legacy-v1 (no version field)")。

## 期望

- 整轮运行：两文件 status 均 unsupported_version、各 0 事件；usage_events 0 行；
  diagnostics 表 unsupported_version ×2；source_files 两行 status=unsupported。
- 显式拒绝落诊断，不是静默的「成功 0 条」。
