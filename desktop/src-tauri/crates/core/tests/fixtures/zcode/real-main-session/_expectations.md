# real-main-session expectations (anonymized native sample, local ZCode 3.14.3)

<a id="real-main-session-期望真实脱敏样本本机-zcode-3143"></a>

Native data extracted read-only locally on 2026-09-25 with
build/desktop-usage-validation/tools/extract-zcode.mjs and stable UUID→anon-N mapping.
Source: first four records of the first `cli/rollout/model-io-<sessionId>.jsonl`, which
contained six lines at extraction. Each request.headers["x-zcode-app-version"]="3.14.3"
establishes product version. Same-machine daily logs have context.schemaVersion=1 and
context.version=0.16.9 (protocol client); database schema_migration≤0.16.5.

<a id="记录形状实读"></a>

## Observed record structure

Each record has type="model_io", attempt=1, sessionId, requestId, turnId, traceId,
querySource=main_turn in this file, model{modelId,providerId}, ISO8601 UTC millisecond
startedAt/completedAt strings and durationMs. Anonymized request retains only
x-zcode-app-version. Response retains finishReason, modelId, responseId, usage and
providerMetadata.anthropic.{usage,cacheCreationInputTokens}.

Two representations of the same request in one record:

- AI SDK camelCase response.usage reports all five fields: inputTokens **including**
  cache read, outputTokens, totalTokens, cacheReadTokens, cacheWriteTokens.
- Anthropic snake_case response.providerMetadata.anthropic.usage reports input_tokens
  **excluding** cache, output_tokens, cache_read_input_tokens, server_tool_use and
  service_tier. cache_creation_input_tokens is absent, with cacheCreationInputTokens=null.

<a id="逐条数值人工核对"></a>

## Individual values (manually checked)

| line | Request (req) | AI SDK in/out/total/cr/cw | anthropic in/out/cr | Consistency |
| --- | --- | --- | --- | --- |
| 1 | anon-2 | 391115/120/391235/390976/0 | 139/120/390976 | 139+390976=391115 ✓ total=in+out ✓ |
| 2 | anon-6 | 391297/595/391892/391104/0 | 193/595/391104 | 193+391104=391297 ✓ |
| 3 | anon-8 | 392343/345/392688/391296/0 | 1047/345/391296 | 1047+391296=392343 ✓ |
| 4 | anon-10 | 392729/114/392843/392320/0 | 409/114/392320 | 409+392320=392729 ✓ |

All querySource=main_turn (primary), attempt=1, turn_anon-3, sessionId=sess_anon-1,
model=GLM-5.3 / account:bigmodel-individual-coding-plan; durations 4132/11330/7876/4338 ms.

<a id="期望单文件"></a>

## Single-file expectations

- Four events, identity zcode:{requestId}:1 (anon-2/anon-6/anon-8/anon-10),
  origin_call_id=requestId, schema_version="3.14.3", session_id=sess_anon-1.
- Primary AI SDK mapping: input_total=inputTokens (reported), cache_read=cacheReadTokens
  (reported), cache_write=cacheWriteTokens (all reported zero),
  input_uncached=in−(cr+cw)=139/193/1047/409 (derived), sum=1788;
  output_total reported, total_tokens=in+out (derived)=391235/391892/392688/392843,
  source_total=totalTokens (reported). Reasoning stays unknown without filled zero.
- Summary (UTC 2026-09-25): call_count=4, input_total_known=1,567,484,
  output_total_known=1,174, cache_read_known=1,565,696, cache_write_known=Some(0),
  total_tokens_known=1,568,658.
- Zero diagnostics: both representations agree on every record, with no unknown keys.
