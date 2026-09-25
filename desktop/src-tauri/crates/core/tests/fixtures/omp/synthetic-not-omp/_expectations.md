# synthetic-not-omp._expectations.md（SYNTHETIC）

**本目录全部为合成样本（synthetic），不是真实会话提取。** V17：未知格式必须
fail closed，不能返回“成功 0 条”。

## 场景与期望

- 首行 `type="event_msg"`（既不是 omp 的 title 也不是 session 头）：即使第 2 行
  是合法 session v3 头也不得识别 —— 首行闸口先判。
- detect ⇒ UnknownFormat{reason: `first record type "event_msg" is neither title nor session`}。
- 整轮：status="unknown_format"、0 事件；diagnostics 表 `unknown_format` ×1；
  usage_events 为空；source_files status="unsupported"。
