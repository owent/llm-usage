# GitHub Copilot for JetBrains 插件取证（2026-10-01）

## 背景与方法

- 用户要求分析插件结构与文档、尝试适配 JetBrains 的 Copilot 用量提取。
  本机未安装 JetBrains IDE（无 `%APPDATA%\JetBrains`），按"文档级证据先行"
  流程：从 JetBrains 市场下载 Windows x64 插件包（github-copilot-intellij
  1.18.0-261，updateId 1173985）解包，对 Kotlin/Java 字节码做常量池字符串
  取证（产物在已忽略的 build/jb-copilot-analysis/），并核对 GitHub 官方
  故障排查文档。不做运行时探测（不启动 IDE/不产生付费用量）。

## 载体结论

- **默认开启的本地数据无逐次 token**：
  - 会话库为嵌入式 **Nitrite**（打包 h2-mvstore 2.2.224）：
    `NitriteAgentSessionPersistenceService` 常量 `copilot-agent-sessions-nitrite.db`，
    实体 `NtAgentSession`（turns、modelName/modelProvider/modelIdType、
    sessionStatus、**`turnCreditsJson`**）/`NtAgentTurn`/`NtAgentMessage`（模型
    信息）；`turnCreditsJson` 编解码为 `RestoredTurnCredits(messageId, credits)`
    ——credit/额度概念，**非 token**（与 CLI 的 nano AIU/premium 倍率同类，
    按合同不折算不入 token）。父路径经 `Project.getDirectoryStorePath`
    （项目 `.idea` 相关存储）组合，精确子路径待真实样本锚定；
  - `idea.log`（官方文档：Help → Show Log，诊断输出）；
  - `telemetry.properties` = App Insights 密钥（远程遥测，按本机来源边界排除）。
- **逐次 token 载体 = 需启用的 OTel 导出**（与 VS Code `github.copilot.chat.otel.*`
  同族同词汇）：
  - 设置项 `CopilotApplicationState`：`otelEnabled`、`otelExporterType`
    （枚举 `file`/`otlp-http`/`otlp-grpc`/`console`，默认协议 `otlp-http`）、
    `otelEndpoint`、`otelOutfile`、`otelServiceName`、`otelResourceAttributes`、
    `otelCaptureContent`；经 agent 命令 `CopilotOtelSettings` 下发给
    copilot-language-server；
  - 官方文档（viewing-logs）：JetBrains 侧有可选 **Agent debug File Logging**
    （Settings → Tools → Copilot → Chat），即上述 file 导出；
  - **五桶 token 解析证据**：插件自身 Agent Debug Panel 的
    `OTelSpanProvider$getEventRows$2` 常量包含 `gen_ai.response.model`、
    `gen_ai.usage.input_tokens`、`gen_ai.usage.output_tokens`、
    `gen_ai.usage.cache_read.input_tokens`、`gen_ai.usage.cache_creation.input_tokens`；
    span 经 `OtlpSpan`（attributes/status，逐行 JSON）从 outfile 读取
    （`readSpansFromFile`），路径由 agent 命令 `GetAgentDebugLogPath`
    （`debug/getAgentDebugLogPath`）返回。
- 原生 agent 为 `copilot-agent/native/*/copilot-language-server.exe`
  （Node SEA 打包，字符串压缩不可直读），与 CLI/VS 同族。

## 适配决定

- **不新增默认载体适配器**：默认本地数据只含 credit（用户明确要求不把
  credit 当作用量主口径；合同规定额度不折算不入 token）。
- **JetBrains 面接入路径 = 既有 otel 适配器（M5）**：用户在插件设置启用
  `otelExporterType=file` + `otelOutfile` 后，把 outfile 作为本应用手工根
  添加。行格式与 VS Code file exporter 同族（NDJSON span 记录），
  本机无 JetBrains 无法真实锚定，维持文档级；真实样本出现后按 V30 流程
  锚定。otel 适配器能力表已登记该同族载体与边界。
- 隐私注意：启用 `otelCaptureContent` 时 outfile 含提示/响应正文；本应用
  解析只读白名单键，正文不入库不输出（与 VS Code otel 载体同一约定）。
- 双计边界：JetBrains outfile（手工根）与本应用 OTLP 接收器不重叠；
  与 JetBrains 自身 Debug Panel 仅读取不冲突（同文件多方只读）。

## 证据清单

- 插件包：JetBrains 市场 updateId 1173985（1.18.0-261 windows-x64）。
- 字节码常量：`NitriteAgentSessionPersistenceService`（db 文件名）、
  `NtAgentSession`/`RestoredTurnCredits`、`CopilotApplicationState`
  （otel 设置族与默认协议）、`OtelExporterType`（四值枚举）、
  `CopilotOtelSettings`、`OTelSpanProvider*`（gen_ai 五桶键 + readSpansFromFile）、
  `GetAgentDebugLogPathCommand`（debug/getAgentDebugLogPath）、
  `telemetry.properties`（App Insights key）。
- 官方文档：[Viewing logs for GitHub Copilot in your environment](https://docs.github.com/copilot/troubleshooting-github-copilot/viewing-logs-for-github-copilot-in-your-environment)
  （JetBrains → idea.log；Agent Debug Panel/Agent debug File Logging 可选）。
- 未完成：无 JetBrains 真实环境，outfile 行格式、默认 otelServiceName、
  Nitrite db 精确路径未锚定——真实样本出现后补验证并升级证据等级。
