# GitHub Copilot for JetBrains plugin inspection, 2026-10-01

<a id="github-copilot-for-jetbrains-插件核验2026-10-01"></a>

<a id="背景与方法"></a>

<a id="github-copilot-for-jetbrains-插件取证2026-10-01"></a>

## Background and method

The user requested analysis of plugin structure/documentation and JetBrains Copilot usage
collection. No JetBrains IDE or %APPDATA%\JetBrains existed locally. Following the
documentation/source inspection process, the Windows x64 Marketplace plugin
github-copilot-intellij 1.18.0-261, updateId 1173985, was downloaded/unpacked. Kotlin/Java
constant-pool strings were checked in ignored build/jb-copilot-analysis/, alongside official
GitHub troubleshooting documentation. No IDE started and no paid usage was generated.

<a id="载体结论"></a>

<a id="本地记录格式"></a>

## Local formats

- **Default local data lacks per-call tokens.**
  - Embedded Nitrite session database, bundled h2-mvstore 2.2.224:
    NitriteAgentSessionPersistenceService names copilot-agent-sessions-nitrite.db.
    NtAgentSession has turns, modelName/modelProvider/modelIdType, sessionStatus and
    turnCreditsJson; NtAgentTurn/NtAgentMessage contain model information. turnCreditsJson
    decodes RestoredTurnCredits(messageId, credits): credits/quota, **not tokens**, comparable
    to CLI nano AIU/premium multipliers. Quotas are neither converted nor added to tokens.
    Project.getDirectoryStorePath builds the parent from project .idea-related storage;
    the exact subpath needs a real sample.
  - idea.log is diagnostic output; official instructions use Help → Show Log.
  - telemetry.properties contains an App Insights key for remote telemetry, outside local-source scope.
- **Per-call tokens require enabled OTel export.** Its names are related to VS Code
  github.copilot.chat.otel.*:
  - CopilotApplicationState: otelEnabled, otelExporterType (file/otlp-http/otlp-grpc/console;
    default protocol otlp-http), otelEndpoint, otelOutfile, otelServiceName,
    otelResourceAttributes and otelCaptureContent. CopilotOtelSettings sends them to
    copilot-language-server.
  - Official viewing-logs documentation describes optional Agent debug File Logging under
    Settings → Tools → Copilot → Chat, corresponding to file export.
  - The plugin's Agent Debug Panel OTelSpanProvider$getEventRows$2 contains
    gen_ai.response.model, gen_ai.usage.input_tokens, gen_ai.usage.output_tokens,
    gen_ai.usage.cache_read.input_tokens and gen_ai.usage.cache_creation.input_tokens.
    OtlpSpan reads line-oriented JSON attributes/status through readSpansFromFile;
    GetAgentDebugLogPath, debug/getAgentDebugLogPath, returns the path.
- Native agent: copilot-agent/native/*/copilot-language-server.exe, a Node SEA executable
  with compressed strings that cannot be read directly, related to CLI/VS agents.

<a id="适配决定"></a>

## Adapter decision

- No new adapter for default files: they hold credits, which the user's requirements keep
  separate from the main token statistics.
- JetBrains intake uses the existing otel adapter (M5). Enable otelExporterType=file and
  otelOutfile in plugin settings, then add outfile as a manual root. NDJSON span format is
  related to the VS Code exporter, but without a local JetBrains environment this remains
  documentation/source analysis. Verify a real sample under V30 when available. The otel
  capability table records this format and limitation.
- otelCaptureContent puts prompt/response bodies in outfile. The application reads only
  permitted statistical keys; bodies are neither stored nor output, as for VS Code OTel.
- The manually selected JetBrains outfile and the application's OTLP receiver must not
  count overlapping calls. Reading the same file in JetBrains Debug Panel is harmless.

<a id="核验资料清单"></a>

<a id="证据清单"></a>

## Inspection references

- Marketplace updateId 1173985, 1.18.0-261 windows-x64 plugin.
- Constants: NitriteAgentSessionPersistenceService (DB name), NtAgentSession/
  RestoredTurnCredits, CopilotApplicationState (settings/default protocol), OtelExporterType
  (four values), CopilotOtelSettings, OTelSpanProvider* (gen_ai fields/readSpansFromFile),
  GetAgentDebugLogPathCommand (debug/getAgentDebugLogPath), telemetry.properties (App Insights key).
- Official [Viewing logs for GitHub Copilot in your environment](https://docs.github.com/copilot/troubleshooting-github-copilot/viewing-logs-for-github-copilot-in-your-environment):
  JetBrains idea.log and optional Agent Debug Panel/Agent debug File Logging.
- Outstanding: actual outfile rows, default otelServiceName and exact Nitrite database path
  require a real JetBrains environment/sample and subsequent local checks.
