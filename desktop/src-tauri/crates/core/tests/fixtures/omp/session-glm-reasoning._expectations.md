# session-glm-reasoning._expectations.md

来源：`<HOME>/.omp/agent/sessions/-AppData-Local-Temp/2026-09-24T16-39-54-647Z_<UUID>.jsonl`
（7 行，oh-my-pi 18.2.7，2026-09-24T16:39Z）。提取时间：2026-09-25。

## 结构期望

- 7 行全部可解析（parseErrors=0）。
- 记录类型计数：title ×1（首行，omp 特征布局，v=1）、session ×1（version=3）、
  model_change ×1、thinking_level_change ×1、message ×2（user + assistant）、
  custom ×1（customType=session_exit）。
- assistant 条目：model=`glm-5.3-flash`，provider=`zhipu-coding-plan`，stopReason=`stop`，
  有 duration/ttft（omp 特有浮点毫秒）、responseId；usage 含 `reasoningTokens`。

## usage 数值期望（白名单原字段）

恰有 1 次模型调用（主调用）：

| input | output | cacheRead | cacheWrite | totalTokens | reasoningTokens | cost.total |
| --- | --- | --- | --- | --- | --- | --- |
| 17542 | 65 | 0 | 0 | 17607 | 54 | 0 |

- 包含关系：17607 = 17542 + 65 + 0 + 0；reasoningTokens 54 ⊆ output 65。
- 映射后：input_uncached=17542、input_total=17542、output_total=65、
  output_reasoning=54（reported）、total_tokens=17607（derived）、source_total=17607。
- duration/ttft 为浮点毫秒，入库取整（四舍五入到 i64 毫秒）。
- cost.total=0 ⇒ 不映射费用。

匿名 ID：7 个（anon-1…anon-7）。
