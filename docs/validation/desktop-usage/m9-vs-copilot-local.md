# Built-in Visual Studio Copilot telemetry intake, 2026-10-01

<a id="visual-studio-内置-copilot-遥测接入2026-10-01"></a>

Same-day [review fixes](m9-copilot-review.md) add per-batch service ownership, failed-call
counting without usage, full trace/span identity, TTFT doubleValue and single-file-root
scope. Initial results remain below.

[Cross-version inspection/discovery fix](m9-vs-copilot-discovery.md), 2026-10-07, adds
TMP/TEMP differences, official installation discovery and static VS 2022/old-extension
limits. Automatic-exporter findings below apply only to inspected VS 18 components/native
samples, not every VS version.

<a id="背景与核验过程"></a>

<a id="背景与证据链"></a>

## Background and inspection

- User confirmed Copilot calls in Visual Studio and requested intake/validation. Earlier
  that morning, VS 18 Community had built-in Copilot; VSGitHubCopilot/copilot-chat/hash/sessions
  was empty.
- After use, read-only checks extracted permitted statistics without bodies:
  - Session files …/VSGitHubCopilot/copilot-chat/hash/sessions/uuid: MessagePack stream with
    version scalar, header and request/response pairs. Responses had ReasoningTokenCount/
    ThinkingElapsedMs, None here; model catalog InputTokens=271900 is context capacity,
    Multiplier=6.0 is premium multiplier; Quotas[] Usage/Limit were None. **No per-call
    input/output tokens.** Bodies were EncryptedContent.
  - %TEMP%/VSGitHubCopilotLogs/*.chat.log: plain text without usage rows.
  - Actual usage files: %TEMP%/VSGitHubCopilotLogs/traces/hex_VSGitHubCopilot_traces.jsonl.
    VS writes OTLP JSON automatically, one resourceSpans batch per line, with no setup
    needed for this inspected component.
- Local version: VS 18.10.1197+4b9e241b86, DevHub/.NET 10.0.12:
  - resource attributes service.name=vs-copilot, service.namespace=visualstudio.
  - chat plus model spans, kind=3 CLIENT, **one per LLM request**: gen_ai.usage.input_tokens,
    output_tokens and cache_read.input_tokens, OTLP string intValue such as 8697;
    gen_ai.request.model/response.model, gen_ai.conversation.id=session UUID,
    startTimeUnixNano/endTimeUnixNano as number or string.
  - invoke_agent GitHub Copilot root spans summarize a turn without usage and are skipped
    following the related official OTel double-counting guidance.
  - gen_ai.input.messages/tool.definitions contain bodies; parser selects statistical keys only.

<a id="实施"></a>

## Implementation

- Independent vs_copilot directory/registry, traces_v1; agent=vs-copilot matches otel service ownership.
  - Discovery: TEMP/TMP VSGitHubCopilotLogs/traces/*.jsonl. Manual roots: traces directory,
    VSGitHubCopilot directory or one JSONL file.
  - Detection: first-line OTLP envelope plus service.name=vs-copilot; v1 KnownVersion.
    Other services UnknownFormat; empty files Pending.
  - JSONL byte-offset incremental scan of appended batch lines; resourceSpans→scopeSpans→spans.
    Each chat span becomes one event, key vs-copilot:span:traceId:spanId. occurred uses
    endTimeUnixNano; duration is endpoint millisecond difference; model prefers response.model,
    then request.model, then span-name suffix. Session uses conversation.id; host_application
    is Visual Studio. String intValue/nanoseconds accepted; TTFT keys accepted but absent
    locally. ERROR status becomes error_status; initially skipped chat spans without usage.
  - Report input/output/cache_read separately; accept cache_creation/reasoning keys. Do not
    derive uncached input, since VS documentation did not establish inclusion semantics.
  - otel agent_of maps vs-copilot/visualstudio-copilot to vs-copilot. Receiver copies use
    the same grouping; choose one format to prevent double counts. Default otel discovery
    covers only its receiver directory, not TEMP files.
- No new database tables; generic usage_events used.

<a id="验证与未完成条件"></a>

## Validation and remaining conditions

- cargo test -p llm-usage-core vs_copilot: 14 synthetic tests pass, covering string integers/
  timestamps, root/no-usage skipping, bounds/missing spanId/bad rows, incremental reads,
  default/manual discovery, four detection states and registry.
- Read-only native check: cargo run -p llm-usage-core --example real_verify_vs_copilot --
  traces-directory build/desktop-usage-validation/vs-copilot-real. files=1, events=2,
  diagnostics=0; count=2, input=17,470, output=219, cache_read=13,184, one model gpt-5.3-codex.
  Rescan count unchanged, added=0, verdict=PASS. Independent redacted Python extraction
  agrees per span: 8697/81/4608 and 8773/138/8576; two invoke_agent spans skipped.
- Same-database daily_usage: 2026-10-01/vs-copilot/gpt-5.3-codex, two calls, input 17,470,
  output 219, visible by agent in overview/trend.
- fmt, clippy -D warnings and full tests (240+3…) pass; npm run verify and
  npm run test:browser exit 0.
- TEMP cleanup can delete history; coverage rotates with VS instances and is not promised
  complete. Local samples lack cache_write/reasoning/TTFT. MessagePack sessions lack
  per-call tokens and are not collected, serving only as supporting inspection. Inline
  completions without session telemetry are not counted. Receiver/native copies use one
  contribution per group.
