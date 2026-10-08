# real-main-session 期望（真实脱敏样本，本机 ZCode 3.14.3）

<a id="real-main-session-expectations-anonymized-native-sample-local-zcode-3143"></a>

本目录为**真实** fixture（2026-09-25 本机只读提取，工具
`build/desktop-usage-validation/tools/extract-zcode.mjs`，UUID→anon-N 稳定映射）。
来源：`cli/rollout/model-io-<sessionId>.jsonl` 首个文件的头部 4 条（提取时源文件共 6 行）。
版本核验来源：每条 `request.headers["x-zcode-app-version"]="3.14.3"`；同机日日志
`context.schemaVersion=1`、`context.version=0.16.9`（协议客户端）；db schema_migration ≤0.16.5。

<a id="observed-record-structure"></a>

## 记录形状（实读）

每条：`type="model_io"`、`attempt=1`、`sessionId`、`requestId`、`turnId`、`traceId`、
`querySource ∈ {main_turn}`（本文件）、`model{modelId,providerId}`、
`startedAt/completedAt` ISO8601 UTC 毫秒字符串、`durationMs`、
`request{...}`（脱敏只保留 x-zcode-app-version 头）、
`response{finishReason,modelId,responseId,usage,providerMetadata.anthropic.{usage,cacheCreationInputTokens}}`。

两组用量字段（同一请求同一记录内两个 usage 视图）：

- AI SDK camelCase `response.usage`：`inputTokens`（**含**缓存读）、`outputTokens`、
  `totalTokens`、`cacheReadTokens`、`cacheWriteTokens`（五键全直报）。

- anthropic snake_case `response.providerMetadata.anthropic.usage`：
  `input_tokens`（**不含**缓存）、`output_tokens`、`cache_read_input_tokens`、
  `server_tool_use`、`service_tier`；`cache_creation_input_tokens` 缺席
  （伴随 `cacheCreationInputTokens:null`）。

<a id="individual-values-manually-checked"></a>

## 逐条数值（人工核对）

| line | 请求（req） | AI SDK in/out/total/cr/cw | anthropic in/out/cr | 一致性 |
| --- | --- | --- | --- | --- |
| 1 | anon-2 | 391115/120/391235/390976/0 | 139/120/390976 | 139+390976=391115 ✓ total=in+out ✓ |
| 2 | anon-6 | 391297/595/391892/391104/0 | 193/595/391104 | 193+391104=391297 ✓ |
| 3 | anon-8 | 392343/345/392688/391296/0 | 1047/345/391296 | 1047+391296=392343 ✓ |
| 4 | anon-10 | 392729/114/392843/392320/0 | 409/114/392320 | 409+392320=392729 ✓ |

全部 `querySource=main_turn`（⇒ primary）、`attempt=1`、同一 `turn_anon-3`、
`sessionId=sess_anon-1`、model=GLM-5.3 / account:bigmodel-individual-coding-plan、
duration 4132/11330/7876/4338 ms。

<a id="single-file-expectations"></a>

## 期望（单文件）

- 事件 4 条；身份 `zcode:{requestId}:1`（anon-2/anon-6/anon-8/anon-10）；
  origin_call_id=requestId；schema_version="3.14.3"；session_id=sess_anon-1。

- 映射（AI SDK 主字段）：input_total=inputTokens（reported）；
  cache_read=cacheReadTokens（reported）；cache_write=cacheWriteTokens（reported，全 0）；
  input_uncached=in−(cr+cw)=139/193/1047/409（derived，和 1788）；output_total（reported）；
  total_tokens=in+out（derived）=391235/391892/392688/392843；
  source_total=totalTokens（reported）；reasoning 未知不补零。

- 汇总（UTC 2026-09-25，单文件）：call_count=4；input_total_known=1,567,484；
  output_total_known=1,174；cache_read_known=1,565,696；cache_write_known=Some(0)；
  total_tokens_known=1,568,658。

- 诊断 0（两组用量字段逐条一致，无未知键）。
