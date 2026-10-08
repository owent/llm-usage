# session-error-zero-usage._expectations.md

来源：`<HOME>/.pi/agent/sessions/--C--Users-owt50--/2026-09-24T16-37-28-439Z_<UUID>.jsonl`
（7 行，pi 0.87.1，2026-09-24T16:37Z 创建）。提取时间：2026-09-25。
M0 时 sessions 为空（no_data）；本次恢复探测发现该真实会话，按允许字段提取脱敏提取。

<a id="structural-expectations"></a>

## 结构期望

- 7 行全部可解析（parseErrors=0）。
- 记录类型计数：session ×1（version=3）、model_change ×1、thinking_level_change ×1、
  custom ×1、custom_message ×1、message ×2（role=system ×1 + role=assistant ×1）。
- 首行即 session 头（与 omp 的 title 首行不同）。
- assistant 条目：`stopReason="error"`，model=`kimi-for-coding`，provider=`kimi-coding`；
  usage 五字段 + cost 全部直报 0（真实错误调用：提供商未计量，0 是报告值不是未知）；
  无 reasoning 字段（保持 unknown，不补零）；无 responseId；无 duration/ttft（pi 无此字段）。

<a id="expected-usage-values-selected-original-fields"></a>

## usage 数值期望（允许字段原字段）

恰有 1 次模型调用（主调用，call_category=primary）：

| input | output | cacheRead | cacheWrite | totalTokens | reasoning | cost.total |
| --- | --- | --- | --- | --- | --- | --- |
| 0 | 0 | 0 | 0 | 0 | （无字段） | 0 |

- error_status="error"；schema_version="3"。
- cost.total=0 ⇒ 不映射费用（0 与无价目不可区分）。
- 期望入库：1 事件；input_total=0、output_total=0、cache_read=0、cache_write=0、
  total_tokens=0（均为 reported/derived 0，非 unknown）；output_reasoning unknown。

匿名 ID：7 个（anon-1…anon-7）。
