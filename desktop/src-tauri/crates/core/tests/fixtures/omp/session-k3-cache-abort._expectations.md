# session-k3-cache-abort._expectations.md

来源：`<HOME>/.omp/agent/sessions/--D--workspace-projs-test-minimind--/2026-09-15T17-33-37-534Z_<UUID>.jsonl`
（31 行，oh-my-pi 18.2.7，2026-09-15T17:33Z）。提取时间：2026-09-25。

## 结构期望

- 31 行全部可解析（parseErrors=0）。
- 记录类型计数：title ×1、session ×1（version=3）、model_change ×1、
  thinking_level_change ×1、title_change ×1、credential_pin ×1、custom ×9
  （tool_execution_start ×8 + session_exit ×1）、message ×16
  （user ×2 + assistant ×7 + toolResult ×7）。
- assistant 全部 model=`k3-256k`、provider=`kimi-code`；stopReason：toolUse ×6 + aborted ×1。
- 全部 assistant 带 duration/ttft 与 responseId；toolResult 均无 usage（6643 条全局亦同）。

## usage 数值期望（白名单原字段，jq 逐条求和）

7 次模型调用（主调用）：

| # | stopReason | input | output | cacheRead | cacheWrite | totalTokens |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | toolUse | 140 | 385 | 17408 | 0 | 17933 |
| 2 | toolUse | 995 | 1152 | 17408 | 0 | 19555 |
| 3 | toolUse | 1593 | 483 | 18176 | 0 | 20252 |
| 4 | toolUse | 15051 | 2225 | 19712 | 0 | 36988 |
| 5 | toolUse | 2470 | 177 | 34560 | 0 | 37207 |
| 6 | toolUse | 380 | 406 | 36864 | 0 | 37650 |
| 7 | aborted | 683 | 0 | 37120 | 0 | 37803 |
| Σ | | 21312 | 4828 | 181248 | 0 | 207388 |

- 逐条包含关系均成立：totalTokens = input+output+cacheRead+cacheWrite；无 reasoningTokens 字段。
- 第 7 条 error_status="aborted"（output=0 是报告值）。
- 期望入库：7 事件全 primary；汇总 input_total=202560（派生口径
  input+cacheRead+cacheWrite=21312+181248+0）、input_uncached=21312、
  cache_read=181248、output_total=4828、total_tokens=207388；cacheWrite 合计 0（reported）。

匿名 ID：37 个（anon-1…anon-37）。
