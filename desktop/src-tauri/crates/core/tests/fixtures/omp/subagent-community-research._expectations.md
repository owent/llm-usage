# subagent-community-research._expectations.md

来源：`<HOME>/.omp/agent/sessions/--D--Comfy-Desktop--/2026-08-21T02-19-22-638Z_<父UUID>/CommunityResearch.jsonl`
（41 行，oh-my-pi 18.2.7，2026-08-21T02:35Z）。提取时间：2026-09-25。

## 结构期望

- 41 行全部可解析（parseErrors=0）。
- 记录类型计数：title ×1、session ×1（version=3，自有会话 ID，无 parentSession 字段）、
  session_init ×1、model_change ×1、thinking_level_change ×1、credential_pin ×1、
  custom ×15、message ×20（user ×1 + assistant ×5 + toolResult ×14）。
- 文件位于 `<ts>_<父UUID>/` 会话目录内：父子关联来自目录名（父会话 UUID），
  目录名时间戳前缀与父会话文件创建时间一致。
- assistant 全部 model=`k3-256k`、provider=`kimi-code`、stopReason=toolUse，
  带 duration/ttft 与 responseId。

## usage 数值期望（白名单原字段，jq 逐条求和）

5 次模型调用（子 Agent 调用，call_category=sub_agent，parent_session_id=目录名中的父 UUID）：

| # | input | output | cacheRead | cacheWrite | totalTokens | duration_ms(原值) | ttft_ms(原值) |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 8574 | 382 | 768 | 0 | 9724 | 17309.34289999993 | 6857.021999999997 |
| 2 | 3542 | 382 | 9216 | 0 | 13140 | 14649.890800000052 | 6983.533100000001 |
| 3 | 3432 | 303 | 12544 | 0 | 16279 | 8387.370200000005 | 1649.5432000000728 |
| 4 | 3467 | 275 | 15872 | 0 | 19614 | 14960.637299999944 | 7235.652699999977 |
| 5 | 1695 | 4793 | 19200 | 0 | 25688 | 126737.42190000007 | 9318.338600000017 |
| Σ | 20710 | 6135 | 57600 | 0 | 84445 | | |

- duration/ttft 为浮点毫秒，入库取整（四舍五入）。
- 期望入库：5 事件全 sub_agent；汇总 input_total=78310（派生口径
  input+cacheRead+cacheWrite=20710+57600+0）、input_uncached=20710、
  cache_read=57600、output_total=6135、total_tokens=84445。

匿名 ID：45 个（anon-1…anon-45）。
