# synthetic-detect-supported._expectations.md（SYNTHETIC）

**本目录全部为合成样本（synthetic），不是真实会话提取。** 结构仿 oh-my-pi 18.2.7
session JSONL（首行 title、次行 session 头、条目基座 {type,id,parentId,timestamp}）。

## 场景与期望

- 5 行：title、session（version=3，id=syn-omp-detect）、model_change（omp 真实
  组合形状 model="provider/model"，供结构化归属）、user message、assistant
  message（带 usage 与 omp 特有 duration/ttft 浮点毫秒）。
- detect：首行 title、前 4 行内 session 头 version=3 ⇒ Supported
  （format=omp-session-jsonl，format_version="3"，basis=known_version——V30
  注册表分派：已收录版本 3 按映射为 KnownVersion）。
- 整轮：1 文件 complete、records_seen=5、1 事件（primary）、added=1。
- assistant usage：input=100、output=10、cacheRead=0、cacheWrite=0、totalTokens=110。
  映射后 input_uncached=100、input_total=100（derived，100+0+0）、total_tokens=110
  （derived，input_total+output）、source_total=110；duration 100.4→100、ttft 50.4→50
  （四舍五入）。
- 汇总（2026-01-05 UTC）：call_count=1、input_total_known=100、cache_read_known=Some(0)、
  output_total_known=10、total_tokens_known=110。
