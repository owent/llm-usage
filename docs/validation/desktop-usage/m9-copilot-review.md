# Copilot 未提交实现审查（2026-10-01）

## 范围与方案

审查 VS Code 原生会话日志、Visual Studio OTLP 遥测、CLI 新布局探测、账户额度、
SQLite 入库与总览展示。保留既有未提交改动；不启动 Agent、不产生模型调用、
不读取远端用量接口、不提交或推送。

| 确认的问题 | 修正与验收行为 |
| --- | --- |
| user turn 和逐模型汇总被当作 model_call | turn/modelTotals 写 usage_observation；仅带稳定 ID 和合法时间的 toolCallRounds 计调用，覆盖限于已落盘主循环 |
| 末次输入与整轮输出派生完整总 token | 输入按已报告下界保存，输出按来源累计保存；默认路径 total_tokens 未知，并给出覆盖诊断 |
| modelTotals 到达后旧键仍贡献用量，多模型换序改变身份 | 模型键不使用数组下标；完整白名单快照带单调修订，替换旧用量贡献并重算日汇总；更正可降低数值 |
| 采样快照同生命周期更新冲突，截断读取可能发布旧前缀 | 完整重放后修订；半行、预算不足、坏行、无效 token 或重复模型键不提交新用量；日志删历史与源文件消失保留既有消费 |
| 通用 chatSessions 中其他扩展被归属 Copilot | 按请求 agent.id 的 github.copilot 命名空间过滤；缺失归属不推断 |
| 空窗口新路径遗漏、迁移副本双计、单文件手工根扩大为目录 | 补 globalStorage/emptyWindowChatSessions；同安装工作区与空窗口共用来源；手工单文件只采指定文件 |
| 探测读取首行后才检查上限 | 实际读取限制为上限加一字节，未完成首行保持 Pending |
| VS 仅首行检查 service，失败无 usage 调用漏计 | 每 resourceSpans 批次核对 service；chat CLIENT span 有 trace/span 身份及时间即计调用，缺 token 不补零；汇总 span 跳过 |
| VS trace 身份缺失和 doubleValue TTFT 未处理 | 缺 trace/span ID 拒绝；doubleValue 秒转毫秒并检查上限 |
| 额度缺字段补零、小数取整、快照时刻被刷新覆盖 | 保留 unknown；整数 milli_requests 保存千分之一请求；以来源 timestamp_utc 分日和去重；同刻元数据更正可更新 |
| 额度清空遗漏、硬期限可重采旧缓存、无 token 时隐藏额度、异步旧响应覆盖、时区错用 | 纳入清空/硬保留；旧缓存不恢复硬过期快照；空用量时仍显示额度，失效请求响应不写回卡片；逐源调度不顺带采账户额度；日快照查询和日期显示使用统计时区 |

不从采样更新次数推测调用次数，不把 thinking、premium credits、账户额度或模型上下文
上限折算为 token。modelTotals 按模型保存时，同一个 turn 的不同模型依然只是用量汇总。
缓存细分及 token 数值非法时记录诊断，不截零或择大。

VS Code toolCallRounds 是已观测主循环调用下界；无此字段的 turn 不虚构一调用。
辅助、子 Agent、重试以及 inline 的完整覆盖未获证明。末次 promptTokens 可形成已知输入
下界，不能解释为这些调用的完整输入。输出累计的准确性受上游去重/重新发报行为限制。

## 依据

- 重新只读本机载体，独立 Python 重放 mutation log、解包 OTLP；输出仅产品标识、
  字段名、计数和 token 合计。原始正文未输出，临时脚本与核验库在 build/copilot-review。
- VS Code 上游 [usage 类型](https://github.com/microsoft/vscode/blob/main/src/vs/workbench/contrib/chat/common/chatService/chatService.ts)
  区分末次 promptTokens 与整轮 modelTotals；
  [chatModel](https://github.com/microsoft/vscode/blob/main/src/vs/workbench/contrib/chat/common/model/chatModel.ts)
  负责 completion 累计，原先已有快照源码亦复核。
- Copilot 上游 [toolCallingLoop](https://github.com/microsoft/vscode-copilot-chat/blob/main/src/extension/intents/node/toolCallingLoop.ts)
  在 runOne 返回后保存 round；
  [toolCallRound](https://github.com/microsoft/vscode-copilot-chat/blob/main/src/extension/prompt/common/toolCallRound.ts)
  声明 timestamp 为轮次开始时刻。main 是本轮研究快照，不作为未来版本的默认合同。
- [OpenTelemetry token 属性](https://opentelemetry.io/docs/specs/semconv/registry/attributes/gen-ai/)
  声明缓存是输入子集、推理是输出子集。VS 实际实现的包含关系仍未逐版本证实，
  不凭通用约定为 VS 派生 uncached。
- 版本基线沿用已有本机记录：VS Code 1.140.0/Copilot Chat 0.68.0、VS 18.10.1197。
  本轮重新核对载体；运行中 Code 进程/标准扩展安装路径未取得新的安装版本元数据。

## 验证进度

独立核对：VS Code 10 个有用量 turn，217 个带 ID/时间戳的 round；
promptTokens 下界合计 3,408,279，completionTokens 累计 320,141，样本无 modelTotals。
Visual Studio 为 2 个 chat span，input=17,470/output=219/cache_read=13,184；
2 个 invoke_agent span 不纳入调用或用量。

已补 SQLite 全链回归：多调用、终态降低更正、modelTotals 替换、模型换序、复制/迁移、
其他扩展、单文件范围、半行续写、源历史清理、失败无 usage、额度硬保留；
单元回归覆盖预算、null Set、非法 token、TTFT 数值形态与额度缺字段/时间/小数。
浏览器回归检查额度单位显示及无 token 历史时仍可见。

修复后的真实采集库与独立预期、daily_usage 对照一致：

| 本机载体 | 已观测调用 | 用量 observation | 输入 | 输出 | 缓存读 | 完整总 token |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| VS Code workspaceStorage，3 个日志 | 217 | 10 | 3,408,279（下界） | 320,141（来源累计） | 未知 | 未知 |
| Visual Studio traces，1 个日志 | 2 | 0 | 17,470 | 219 | 13,184 | 未报告 |

VS Code 的 10 个诊断均为输入覆盖提示，不是损坏行；两适配器重扫新增为 0，
调用和用量不变。VS 的 cache_write/reasoning 保持 None，不在核验工具中补零。
VS Code 原始模型字段出现两种标识表示，本轮保留来源值，不凭相似名称模糊合并。
额度缓存当前有 1500 请求上限、1500 剩余、0 已用；真实提取/落库/回读通过。
此前 137.4 剩余的样本仅用于小数回归，不拿它替代当前额度快照。

从仓库根执行的真实核验均退出码 0：

- `python build/copilot-review/audit.py`：独立只读原始载体，白名单汇总。
- `cargo run --quiet --locked --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core
  --example real_verify_copilot_chat -- <workspaceStorage> <build/copilot-review/chat-real>`。
- `cargo run --quiet --locked --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core
  --example real_verify_vs_copilot -- <traces> <build/copilot-review/vs-real>`。
- `python build/copilot-review/compare.py`：真实明细与日汇总均匹配独立预期，PASS。
- `cargo run --quiet --locked --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core
  --example copilot_quota_probe`：账户额度回读 PASS；不输出路径、login 或正文。

新增 7 项 Rust 单元回归、6 项 SQLite 集成回归，以及浏览器的额度单位、空历史、
统计时区显示断言。最终验证环境为本机 Windows、PowerShell 7.6.6、Node.js
24.21.0、Cargo 1.98.1。以下命令均从仓库根执行，退出码均为 0：

- `npm run verify`：Markdown/资产/脚本检查、Svelte 检查（0 错误、0 警告）、
  Rust fmt/Clippy、716 项 Rust 测试及前端生产构建通过。
- `npm run test:browser`：使用本机 Edge，交互与新增额度断言通过。
- `git diff --check`：通过。

日志保留在忽略目录 `build/copilot-review/verify-final.log`、
`build/copilot-review/browser-final.log` 与 `build/copilot-review/real-compare.log`。
这些结果为本机静态、测试及真实落盘数据证据，不代替三平台或真实 IDE 操作验收。

## 保留的边界

同面原生载体与 OTel file/接收器同时启用仍需在数据源界面择一；共享 agent 名称
不会自动去重，没有跨载体调用关联证据时禁止相加。这是现有来源选择边界，
本轮不宣称已完成自动跨载体去重。VS 的 TEMP 载体不承诺完整历史。
后续 [配置设计](../../design/desktop-usage/copilot-otel.md)进一步确认：暂停只停止扫描，
不排除历史贡献；引入 OTel 前需统计主来源选择，不能仅停用原生源后混合总计。
JetBrains 仅有插件静态取证，没有真实 IDE/outfile 样本，仍为文档级。
CLI chronicle 无逐次 token 的结论仅限已核验载体，未知新版不猜字段。

数据库仍遵守既有预发布版本不匹配提示重建的规则，不实施逐版本迁移。
本轮 parser/来源命名空间已变化，schema 更新至 10，旧试验库按既有流程提示重建。
本轮以新隔离核验库对照，未改用户应用数据库；未运行真实 JetBrains 服务或生产环境。
