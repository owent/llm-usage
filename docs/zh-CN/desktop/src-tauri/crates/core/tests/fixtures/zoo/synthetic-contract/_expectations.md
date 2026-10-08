# synthetic-contract 期望（全合成）

<a id="synthetic-contract-expectations-all-synthetic"></a>

场景：一个任务文件，两对 api_req_started/finished（LIFO 合并）、一条
condense_context（cost 贡献）、一条未配对 started（无 usage 数字）。

期望（人工核算，consolidateApiRequests/consolidateTokenUsage 语义）：

- `usage_events` 3 条（2026-06-13 日汇总 call_count=3）：
  - `syn-zoo-1:api_req_started:1781337600500`（primary，
    time_basis=source_start）：text 合并后
    {request, apiProtocol, tokensIn:1000, tokensOut:200, cacheWrites:100,
    cacheReads:8000, cost:0.012}——tokensIn **含缓存**（固定源码注释）⇒
    input_total=1000（直报）、input_cache_read=8000、input_cache_write=100、
    input_uncached=NULL、output_total=200、
    total_tokens=1200（in+out，上游 contextTokens 算术）、
    cost=12000 micro-USD（estimated）。
  - `syn-zoo-1:condense_context:1781338000000`（auxiliary，
    time_basis=uncertain）：token 全未知（contextCondense 无 token 字段；
    newContextTokens 是上下文规模不入账）、cost=3000 micro-USD。
  - `syn-zoo-1:api_req_started:1781339000000`（primary）：合并后
    tokensIn=500/cacheReads=9000/cacheWrites=0/tokensOut=80/cost=0.006 ⇒
    input_total=500、total_tokens=580、cost=6000 micro-USD。
- 未配对 started（ts=1781339500000）无 token 数字 ⇒ 不产事件
  （一次性诊断 usage_carrier_without_numbers）。
- api_req_finished 消息本身不产事件（usage 已并入 started，不双计）。
- 日汇总（2026-06-13，UTC）：call_count=3（2 primary + 1 auxiliary）、
  input_total=1500、cache_read=17000、cache_write=100、output=280、
  total_tokens=1780。
- 诊断：latest_fallback 无（文档确认的格式标识恒 KnownVersion）；
  usage_carrier_without_numbers 恰 1 条。
- 再次扫描没有重复入库：call_count 仍 3。
