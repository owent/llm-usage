# Visual Studio 内置 Copilot 遥测接入（2026-10-01）

同日 [审查修正](m9-copilot-review.md) 补充每批服务归属检查、无 usage 失败调用计数、
完整 trace/span 身份、TTFT doubleValue 解析与单文件根范围。下文保留首次核验结果。

## 背景与证据链

- 用户确认 Visual Studio 中已有 Copilot 调用，要求接入并验证。此前核查
  （同日上午）：VS 18 Community 已装内置 Copilot 扩展，
  `VSGitHubCopilot\copilot-chat\<hash>\sessions` 当时为空。
- 用户使用后重新只读取证（按准备合同白名单提取、不输出正文）：
  - **会话文件** `…\VSGitHubCopilot\copilot-chat\<hash>\sessions\<uuid>`
    （MessagePack 流：版本标量 + 会话头 + 请求/响应对）：响应含
    `ReasoningTokenCount`/`ThinkingElapsedMs`（样本为 None）、模型目录
    （`InputTokens`=上下文上限 271900、`Multiplier`=6.0 premium 倍率）、
    `Quotas[]` 账户快照（Usage/Limit 均 None）——**无逐次 input/output token**；
    响应正文为 EncryptedContent；
  - `%TEMP%\VSGitHubCopilotLogs\*.chat.log`：纯文本日志，无用量行；
  - **实际载体：`%TEMP%\VSGitHubCopilotLogs\traces\<hex>_VSGitHubCopilot_traces.jsonl`**
    ——VS 自动写入的 OTLP JSON 遥测（每行一个 resourceSpans 批次，无需任何配置）。
- 遥测形态（本机 VS 18.10.1197+4b9e241b86，DevHub/.NET 10.0.12）：
  - resource.attributes：`service.name=vs-copilot`、`service.namespace=visualstudio`；
  - `chat <model>` span（kind=3 CLIENT，**每 LLM 请求一个**）：
    `gen_ai.usage.input_tokens`/`output_tokens`/`cache_read.input_tokens`
    （OTLP `{intValue:"8697"}`，int64 字符串形）、`gen_ai.request.model`/
    `response.model`、`gen_ai.conversation.id`（=会话 UUID）、
    `startTimeUnixNano`/`endTimeUnixNano`（纳秒，数字或字符串）；
  - `invoke_agent GitHub Copilot` 根 span 整 turn 汇总（无 usage）——按官方
    OTel 防双计警告同族规则跳过；
  - `gen_ai.input.messages`/`tool.definitions` 内嵌提示正文：解析只读白名单键。

## 实施

- 新增 `vs_copilot` 适配器（独立目录 + 版本注册表 `traces_v1`，
  agent=`vs-copilot`，与 otel 适配器 service.name 归属一致）：
  - 发现：`%TEMP%\VSGitHubCopilotLogs\traces\*.jsonl`（TEMP/TMP 定位）；
    手工根支持 traces 目录/VSGitHubCopilot 目录/单个 jsonl；
  - 探测：首行 OTLP 信封 + service.name=vs-copilot 指纹（v1 KnownVersion）；
    其他 service 记 UnknownFormat；空文件 Pending；
  - 扫描：标准 JSONL 字节偏移增量（批次行追加写）；逐行解包
    resourceSpans→scopeSpans→spans；chat span 一事件，键
    `vs-copilot:span:<traceId>:<spanId>`；occurred=endTimeUnixNano（完成时刻）、
    duration=两端毫秒差、模型 response.model>request.model>span 名段、
    session=conversation.id、host_application="Visual Studio"；
    intValue 字符串形/纳秒字符串形容错；TTFT 键（本机未出现）容错支持；
    status.code=ERROR 记 error_status；无 usage 的 chat span 跳过；
  - token 桶并列报告（input/output/cache_read；cache_creation/reasoning 键
    容错支持），不派生 uncached（与 otel 家族口径一致：包含关系未由 VS 文档声明）；
  - otel 适配器 agent_of 增补 `vs-copilot`/`visualstudio-copilot` → `vs-copilot`
    （接收器捕获同源 span 时同维度；两载体取其一防双计）；otel 默认发现
    仅自身接收器目录，与 TEMP 遥测目录无重叠。
- 数据库无新增表（usage_events 通用表）。

## 验证与未完成条件

- 合成单测：`cargo test -p llm-usage-core vs_copilot`——14 项通过
  （信封映射含 intValue 字符串/纳秒字符串形、invoke_agent 跳过、无 usage
  跳过、越界诊断、缺 spanId 诊断、坏行诊断、增量续读、默认/手工发现、
  探测四态、版本注册表）。
- 本机真实数据（只读）：
  `cargo run -p llm-usage-core --example real_verify_vs_copilot --
  <traces 目录> build/desktop-usage-validation/vs-copilot-real`：
  files=1、events=2、diagnostics=0；count=2、input=17,470、output=219、
  cache_read=13,184、models=1（gpt-5.3-codex）；重扫 count 不变、added=0，
  verdict=PASS。数值与独立脱敏提取（Python 解包 OTLP）逐 span 一致
  （8697/81/4608 与 8773/138/8576），invoke_agent×2 已跳过。
- 应用管线：同库 `daily_usage` 物化 `2026-10-01 / vs-copilot /
  gpt-5.3-codex / 2 次 / input 17,470 / output 219`，总览与趋势按 agent
  分组即可见。
- 门禁：`cargo fmt`/`clippy -D warnings`/全量测试（240+3…）通过；
  `npm run verify` 退出码 0；`npm run test:browser` 退出码 0。
- 未完成/边界：载体在 TEMP——系统/磁盘清理会删历史遥测，覆盖随 VS 实例
  滚动（不承诺完整历史）；cache_write/reasoning/TTFT 属性本机样本未出现；
  会话文件（MessagePack）无逐次 token，不接入（仅作旁证）；inline 补全
  不经会话遥测时不计；与 otel 接收器载体同维度择一。
