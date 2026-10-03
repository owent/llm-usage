# 调研依据与验证入口

本文件保留设计的来源及适用版本。工作区为 D:/workspace/projs/github/owent/llm-usage；
基础资料核验于 2026-09-24，扩展格式于 2026-09-29，调度与原生验收依据于 2026-10-03。
原型只做静态核对，未执行 collect.py 或认证其运行合同。

来源以官方正文、维护者固定源码和本地原型为层级，搜索摘要只用于定位。
滚动文档不代表本机安装版本；源码字段存在不等于完整运行路径已验收。
各 Agent 当前实现与真实验收以 [接入矩阵](adapters.md) 为准。
Windows 11 x64 首发，三平台 CI；本机真实数据验证已获允许，缺证 IDE 留 F1。
仅统计本机来源，企业 API/账号报表资料只解释排除边界；Hermes 指 Nous Research 产品。

<a id="prototype"></a>

## 原型的静态核对

参考 [previous-draft](../../../previous-draft/README.md)，共检查入口、存储、注册框架、
七个 collectors 和 dashboard/index.html；没有发现业务依赖声明或测试合同。

| 位置 | 已观察到的行为 | 新设计处理 |
| --- | --- | --- |
| collect.py | 加载七个采集器，收集整个 list 后入库；导出日/周/月/今日小时快照 | 有界分批，UI 通过后端刷新；保留维度但不把脚本当成桌面运行时 |
| dashboard/index.html | 独立刷新只 fetch data.json；失败或数据不符时显示 SAMPLE | 真正发起采集；失败保留旧结果并显示状态，不以演示数据冒充真实数据 |
| store.py / dashboard | input + cache_read 被当作总输入，cache_write 没进入该公式 | 三类互斥输入按适配器合同归一化；零分母和未知分开 |
| store.py | request_id 全局唯一，INSERT OR IGNORE；日聚合无 provider | 稳定身份命名空间、可更正记录、provider 维度；不把每行都视为一次调用 |
| collectors/__init__.py | iter_new_lines 先 state_set 再解析；只按大小检测截断 | 游标/解析状态/事件/聚合原子提交；检测同长替换、改名和 generation |
| collect.py / Store | 事件写与聚合分开 commit；不同源共用连接，失败源 state 可能被后来提交 | 每源批次原子性和可重放；禁止保存失败批次游标 |
| kilo_code.py / copilot.py | 分别复制运行中的 db、wal、shm 再读 | SQLite 只读短事务或一致备份，不能把连续文件复制称为一致快照 |
| copilot.py | 窗口外条件只有 pass；仍遍历所有行；request_id 含数据库路径和变化数值 | 源键及修订分离；去重多个库副本，不能声称现有 cursor 已实现有效增量 |
| codex.py | 任意带 model 的非 usage 行用正则；找不到早先模型时回填第一条；缺 response ID 易碰撞 | 只使用结构化模型上下文；无证据保持 unknown；缺 ID 使用已验证替代身份 |
| kimi_code.py / kimi_work.py | 新 wire 候选字段明确，但版本来自注释；Work 固定机器 D 盘路径 | 官方确认路径后仍需 schema fixture；不复制机器路径作为默认值 |
| zcode.py | requestId 和 traceId 都缺失时身份变成 zcode:None | 缺失稳定 ID 单独处理，不能合并所有记录 |
| oh_my_pi.py | assistant usage + title-generator success 日志 | 检验重叠，补充压缩/分支/独立 usage 类别的覆盖 |
| store.py | 保留常量 366 天，依执行机器本地时区；按日期 cutoff 清理 | 可配置、固定时区及明确定义包含天数，清理/重扫/封存汇总协同 |

这些是静态路径和失败条件分析，不是已复现线上错误，不直接修补原型。

用于识别本轮输入的 SHA-256：

| 文件 | SHA-256 |
| --- | --- |
| previous-draft/store.py | 81a6bb9e3a7f7943b40438ed642823ab3969f073e68164ce7eb26133cdc7d6d4 |
| previous-draft/collect.py | aa8a406329afd21b4e49ba230fe150ffeeaf173247409d9f70992a8e256cf945 |
| previous-draft/collectors/__init__.py | c4412b8fef0b7220e93c82901414e3833a681c57ed11480311f15d81cb2c0daa |

## 技术选型依据

| ID | 已核验来源 | 本设计使用的事实与限制 |
| --- | --- | --- |
| T01 | [Tauri WebView](https://v2.tauri.app/reference/webview-versions/)、[Windows installer](https://v2.tauri.app/distribute/windows-installer/) | 各 OS 的 WebView 不同；Windows 运行时分发方式影响体积；不从官网示例推本应用包体 |
| T02 | [SQLite WAL](https://sqlite.org/wal.html)、[Online backup](https://sqlite.org/backup.html) | WAL 并发、只读条件、checkpoint 和一致备份；WAL-reset 修复版本需验证 |
| T03 | [Electron process model](https://www.electronjs.org/docs/latest/tutorial/process-model) | Chromium 多进程及 main/renderer 边界；不引用未经测量的内存差值 |
| T04 | [DuckDB concurrency](https://duckdb.org/docs/lts/connect/concurrency) | 官方明确大量小事务不是主要目标；本应用首先需要增量写入/更正 |
| T05 | [ECharts import](https://echarts.apache.org/handbook/en/basics/import/) | 可按需导入图表、组件与渲染器；本项目裁剪效果未测 |
| T06 | [Tauri GitHub CI](https://v2.tauri.app/distribute/pipelines/github/)、[依赖](https://v2.tauri.app/start/prerequisites/) | 可分 OS 构建；Linux WebKitGTK/系统依赖、macOS/Windows 工具链分别配置；不照搬自动发布示例 |
| T07 | [GitHub runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners) | 版本化标签/架构候选；windows-2022、ubuntu-22.04、macos-15；M0 复核可用性，runner 构建不是目标桌面验收 |
| T08 | [Microsoft WSLg](https://learn.microsoft.com/en-us/windows/wsl/tutorials/gui-apps) | Linux GUI 需 WSL 2，WSLg 不等于完整 Linux 桌面；已有环境/编译证据见验证目录，最新 revision 仍独立验收 |
| T09 | [Task Scheduler](https://learn.microsoft.com/en-us/windows/win32/taskschd/task-scheduler-start-page)、[身份](https://learn.microsoft.com/en-us/windows/win32/taskschd/security-contexts-for-running-tasks)、[错过时点](https://learn.microsoft.com/en-us/windows/win32/taskschd/tasksettings-startwhenavailable) | 2026-10-03 复核正文；COM API 使用当前交互用户、LUA、分钟重复和不唤醒；普通用户注册/删除已实测，OS 实际启动另验 |
| T10 | [Tauri WebDriver](https://v2.tauri.app/develop/tests/webdriver/) | 当前文档有嵌入式跨平台服务；直接 tauri-driver 仍区分 Windows/Linux 与 macOS；测试插件不得进入发行制品 |
| T11 | [WebView2 调试](https://learn.microsoft.com/en-us/microsoft-edge/webview2/how-to/debug-visual-studio-code)、[Playwright WebView2](https://github.com/microsoft/playwright/blob/main/docs/src/webview2.md) | 2026-10-03 复核正文及源码；环境参数提供 CDP，测试从进程外连接真实 IPC，不在应用新增调试监听器 |
| T12 | [libuv Windows spawn](https://github.com/libuv/libuv/blob/v1.x/src/win/process.c) | 2026-10-03 阅读 required_vars 并以 Node 24.21.0 实测；省略子 env 中 USERPROFILE 会补入父值，验收辅助函数须防此路径 |
| T13 | [Tauri CLI 2.12.0](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.0/crates/tauri-cli/src/interface/rust.rs)、[资源代码](https://github.com/tauri-apps/tauri/blob/tauri-codegen-v2.7.0/crates/tauri-codegen/src/embedded_assets.rs) | 2026-10-03 正文与本机锁定源码交叉核验；CLI 自带 custom-protocol，资源缓存存在即复用；生成包不证明嵌入资源可解压/可显示 |
| T14 | [RegGetValueW](https://learn.microsoft.com/en-us/windows/win32/api/winreg/nf-winreg-reggetvaluew)、[删除值](https://learn.microsoft.com/en-us/windows/win32/api/winreg/nf-winreg-regdeletekeyvaluew) | 2026-10-03 正文与 windows 0.62.2 签名核验；读取类型/长度及 Unicode 值，缺值与访问失败分开，原生查询无需 reg.exe |

<a id="agents"></a>

## Agent 官方与源码索引

下表共用字段：verified_at=2026-09-24；method=打开官方正文或下载固定源码只读核对；
installed_version=未探测；owner=本项目维护者；status=研究证据，非运行验收。
implementation trigger=对应阶段开始、产品升级、schema 改变或数值不符时复核；不自动升级工具。

| ID | 官方来源 / 固定源码 | 使用结论及限制 |
| --- | --- | --- |
| A01 | [Claude 目录](https://code.claude.com/docs/en/claude-directory)、[监控](https://code.claude.com/docs/en/monitoring-usage) | 会话/子 Agent transcript 路径，官方 token 分类和 query_source；逐请求 JSONL schema 尚待 fixture |
| A02 | [Codex 监控](https://learn.chatgpt.com/docs/agent-approvals-security) | 可选 OTel、请求/响应完成事件、提示词隐私配置；原型 token_usage_record 未获跨版本保证 |
| A03 | [Cline metrics](https://github.com/cline/cline/blob/dcf8c3c33596e3d561a941202297c564a1cbcd49/apps/vscode/src/shared/getApiMetrics.ts)、[storage](https://github.com/cline/cline/blob/dcf8c3c33596e3d561a941202297c564a1cbcd49/apps/vscode/src/core/storage/disk.ts) | ui_messages.json 与 API/删除/子 Agent usage 汇总；模型逐请求及 CLI 格式未验证 |
| A04 | [CodeBuddy monitoring](https://www.codebuddy.ai/docs/cli/monitoring)、[官方目录](https://www.codebuddy.ai/docs/cli/codebuddy-dir)、[aiusage v1.5.8](https://github.com/juliantanx/aiusage/blob/main/CHANGELOG.md) | CLI `~/.codebuddy/projects` 本地 JSONL 已作第三方源码级接入；`message.usage.input_tokens` 含 cache read，优先 `rawUsage` 分桶；OTLP/HTTP protobuf 是可选来源，双载体重叠未消解；IDE `statsSnapshot` 已有第三方线索但具体字段/本机样本待核 |
| A05 | docs.github.com Copilot CLI OTel 文档（2026-09-29 核验：COPILOT_OTEL_FILE_EXPORTER_PATH JSON-lines、OTLP 默认 http/json、chat/invoke_agent 双层 span 与防双计警告；本地主载体 assistant_usage_events 已真实核对）原行 → [Copilot CLI reference](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-command-reference) | OTLP JSON/protobuf 与 file JSONL、chat span；不依赖私有 SQLite schema 承诺 |
| A06 | microsoft/vscode extensions/copilot agent_monitoring.md bdc5ebe（2026-09-29 核验：`github.copilot.chat.otel.*` 设置族、file exporter NDJSON 非 OTLP、startTime [秒,纳秒]、chat span `gen_ai.usage.*` 属性表）原行 → [VS Code monitoring](https://code.visualstudio.com/docs/agents/guides/monitoring-agents) | chat 与 invoke_agent 不同层级；可选缓存/推理与 TTFT 字段；只覆盖文档指定 Agent |
| A07 | [JetBrains API v2](https://www.jetbrains.com/help/jetbrains-console/analytics-api-v2.html)、[Session explorer](https://www.jetbrains.com/help/jetbrains-console/session-explorer.html) | 仅证实远端企业分析，按新决策排除；不证明本地 schema，本地提取后移 F1 |
| A08 | [DSH token-meter](https://github.com/deepseek-ai/deepseek-harness/blob/46a7f68b0922371ce7144b668b90e377d8e799f4/packages/llm/token-meter/README.md) | 四类用量、final 替换、retry 边界；pressure/composition 非计账值 |
| A09 | [OpenClaw store](https://docs.openclaw.ai/reference/session-management-compaction/store)、[token use](https://docs.openclaw.ai/reference/token-use) | 当前运行时 SQLite 与旧归档分离，usage/费用/上下文概念分离；具体表和兼容版本待验 |
| A10 | [Gemini sessions](https://geminicli.com/docs/cli/session-management/)、[telemetry](https://geminicli.com/docs/cli/telemetry/) | 本地 session 保存 token；官方请求、token 分类和延迟指标；具体映射继续核验 |
| A11 | [Kilo core schema](https://github.com/Kilo-Org/kilocode/blob/9e3f350767477864f7504fab3e01bf6c4d2a7644/packages/core/src/session/schema.ts)、[用量索引迁移](https://github.com/Kilo-Org/kilocode/blob/9e3f350767477864f7504fab3e01bf6c4d2a7644/packages/core/src/database/migration/20260907102000_kilocode_model_usage_index.ts) | 已核验源码树和 core 路径，不能沿用原型旧 message 表；索引具体 SQL 未完整审计 |
| A12 | [Kimi sessions](https://www.kimi.com/code/docs/en/kimi-code-cli/guides/sessions)、[旧源码 types](https://github.com/MoonshotAI/kimi-cli/blob/9ab1286b8fe4e6bcd116949a27ce5e0ac3389c82/src/kimi_cli/wire/types.py) | 官方新版路径已读；该 Python 文件没有证实原型 camelCase usage.record，保持字段待验证 |
| A13 | [原型 kimi_work.py](../../../previous-draft/collectors/kimi_work.py) | 仅作本地线索，无新版官方稳定存储合同或真实样本验收 |
| A14 | [MiMo message](https://github.com/XiaomiMiMo/MiMo-Code/blob/456678b6a5afb0eef3fe2754575637218cfb3c84/packages/opencode/src/session/message-v2.ts)、[global path](https://github.com/XiaomiMiMo/MiMo-Code/blob/456678b6a5afb0eef3fe2754575637218cfb3c84/packages/opencode/src/global/index.ts) | token/cache/reasoning 字段和独立路径解析；不从 fork 关系推兼容 |
| A15 | [oh-my-pi session](https://github.com/can1357/oh-my-pi/blob/62bc57be1b03ef0802a33cf7f5f530e534527531/docs/session.md) | session 路径、entry 模型、持久化和分支；辅助日志以版本样本为准 |
| A16 | [pi Usage](https://github.com/badlogic/pi-mono/blob/b45597504eeaba1f11a9920a1d1048c361ed4b8e/packages/ai/src/types.ts)、[session manager](https://github.com/badlogic/pi-mono/blob/b45597504eeaba1f11a9920a1d1048c361ed4b8e/packages/coding-agent/src/core/session-manager.ts) | reasoning 是 output 子集；独立 usage、压缩及分支总结有 usage |
| A17 | [OpenCode SQL](https://github.com/anomalyco/opencode/blob/0027387dc5c59793c12dfc531abc78f825ed6868/packages/core/src/session/sql.ts)、[schema](https://github.com/anomalyco/opencode/blob/0027387dc5c59793c12dfc531abc78f825ed6868/packages/core/src/session/schema.ts) | 当前 core 层及 session token 累计列；精确逐请求映射尚未实现 |
| A18 | [Qwen recording](https://github.com/QwenLM/qwen-code/blob/085e98c00cac2f8dd29eb39c760409bc6da889a9/packages/core/src/services/chatRecordingService.ts)、[telemetry](https://qwenlm.github.io/qwen-code-docs/en/developers/development/telemetry/) | usageMetadata/model 与会话路径；Goal 累计观测不能直接求和，prompt 日志需关闭 |
| A19 | [Zoo consolidateTokenUsage](https://github.com/Zoo-Code-Org/Zoo-Code/blob/f7806475331fcae5f4e8b5558d04415eeb5da88c/packages/core/src/message-utils/consolidateTokenUsage.ts) | API 请求合并、condense_context、token/cache 字段；不同宿主仍待核验 |
| A20 | [TRAE session API](https://docs.trae.cn/enterprise_query-usage-details-by-session-id)、[企业个人用量](https://docs.trae.cn/enterprise_check-individual-usage) | 企业远端接口/报表按新决策排除；不能推断本地格式，本地提取后移 F1 |
| A21 | [ZCode 用量](https://zcode.z.ai/cn/docs/usage-stats)、[原型 zcode.py](../../../previous-draft/collectors/zcode.py) | 官方区分本地会话与远端 Coding Plan；具体 model-io schema 仍是原型线索 |
| A22 | [WorkBuddy 用量](https://www.workbuddy.cn/docs/workbuddy/Usage)、[AgentHUD provider 合同](https://github.com/jazzenchen/agent-hud-open/blob/main/docs/providers.md)、[tokmesh-core 解析器](https://docs.rs/tokmesh-core/latest/src/tokmesh_core/sessions/tencent_buddy.rs.html) | 官方仅给积分/套餐；第三方源码确认 `.workbuddy/projects` 会话 JSONL 用量，随后本机 8 文件/420 事件真实核对通过；trace 汇总与会话重复及版本范围待核 |
| A23 | [Zed telemetry](https://zed.dev/docs/telemetry) | hosted 计量和本地遥测日志有区别；不能据服务端 token 计量推出本地完整字段 |
| A24 | [Hermes 官网](https://hermes-agent.nousresearch.com/)、[存储文档](https://hermes-agent.nousresearch.com/docs/developer-guide/session-storage/)、[固定存储说明](https://github.com/NousResearch/hermes-agent/blob/ef70b3661cbfcf57e583008ad91dd04d8ba46070/website/docs/developer-guide/session-storage.md)、[usage](https://github.com/NousResearch/hermes-agent/blob/ef70b3661cbfcf57e583008ad91dd04d8ba46070/hermes_state_usage.py)、[schema](https://github.com/NousResearch/hermes-agent/blob/ef70b3661cbfcf57e583008ad91dd04d8ba46070/hermes_state_schema.py)、[response usage](https://github.com/NousResearch/hermes-agent/blob/ef70b3661cbfcf57e583008ad91dd04d8ba46070/agent/turn_usage.py) | 用户已明确产品；本地 state.db/profile、模型/task 累计、增量/absolute、辅助不写 sessions、历史回填已核对；逐次日志和 normalize_usage 完整语义仍待验 |
| A25 | [tokscale](https://github.com/junhoyeo/tokscale)（固定 main `1d9a9395418efc6952944b794097935d7d6fa1e8`） | 第三方开源多 Agent 本地会话解析器（Rust）；本轮只读其 clients/scanner/sessions 源码提取路径与字段线索，未执行代码；其结论不替代逐产品官方核验与本机 fixture，估算/比价逻辑不沿用 |
| A26 | [Goose](https://github.com/aaif-goose/goose)（block/goose 已迁移改名；官方核验 a701bb1，2026-09-29） | 迁移 15 起 `usage_ledger` 逐请求表（Unix 秒 + input/output/total/cache 两列 + cost + cost_source + is_compaction；tokscale 未见）；sessions 表 `accumulated_*` 会话累计（非 accumulated 列是最后快照）；时间列是文本 UTC；input 含 cache 读写；GOOSE_PATH_ROOT（绝对路径）/Windows %APPDATA%\Block\goose/旧 Block 根 |
| A27 | [Crush](https://github.com/charmbracelet/crush)（固定 main `1f3827bcd2d20f38076b2d46123683271e6ed9ba`，官方核验 2026-09-29） | projects.json {projects:[{path,data_dir,last_accessed}]}（CRUSH_GLOBAL_DATA/XDG/LOCALAPPDATA）；每项目 `<data_dir>/crush.db`；__sessions.prompt/completion 是最近 step 上下文规模快照（非用量、摘要后重置）__；cost 累计且子会话回卷父行（取 parent_session_id IS NULL）；messages 无 token 列 |
| A28 | [Amp 官网](https://ampcode.com/)（闭源 CLI；tokscale 1d9a939 sessions/amp.rs，2026-09-29 采信） | `~/.local/share/amp/threads/T-*.json`：messages[].usage（model、inputTokens/outputTokens、cacheRead/cacheCreation、credits）与 usageLedger.events（timestamp/model/credits/tokens）双载体，需对账防双计 |
| A29 | [Roo Code](https://github.com/RooCodeInc/Roo-Code)（官方核验 b867ec9145750d0ae1ff7f02d35406e9bf2a0b16，仓库已归档 2026-05） | VS Code globalStorage `rooveterinaryinc.roo-cline/tasks/<uuid>/`（ui_messages.json 等）及 .vscode-server 变体；Cline 血统同构，删除/子 Agent/压缩行为分版本复测 |
| A30 | [Aider](https://github.com/Aider-AI/aider)（固定 main `5dc9490bb35f9729ef2c95d00a19ccd30c26339c`，官方核验 2026-09-29） | 默认仅 .aider.chat.history.md/.aider.input.history（无逐次 usage）；`--analytics-log`/llm 历史日志为可选本地落盘（需启用、不回填）；逐次 token/cost 字段待 fixture |
| A31 | [Continue](https://github.com/continuedev/continue)（官方核验 5522c6f44ca0ac3528b37b44818fbfa39b5af470，2026-09-29） | `~/.continue`（sessions、logs、index）目录已由官方文档证实；会话载体逐次 token 字段未核验；hub 账号数据不接入 |
| A32 | [Droid](https://factory.ai/)（Factory.ai CLI） | `~/.factory/sessions/{uuid}.settings.json` 的 tokenUsage（input/output/cacheRead/cacheCreation/thinking，累计）+ 同名 jsonl 转录；无费用；累计分摊是估计仅作区间 |
| A33 | [Amazon Q Developer CLI](https://github.com/aws/amazon-q-developer-cli)（官方核验 15cc8f3，2026-09-29） | `~/.aws/amazonq/history/` 按时间戳 JSON 会话（第三方证据）；repo 开源可源码核验；history 内 usage 字段未核验，SSO 重登录可能丢历史 |
| A34 | Grok Build（xAI 闭源 CLI；[新闻报道](https://www.penligent.ai/)证实产品与版本线） | `~/.grok/sessions/<workspace>/<session>/{updates.jsonl,signals.json,summary.json,events.jsonl}` 与 `~/.grok/logs/unified.jsonl`；显式 usage 块五桶可用，累计 totalTokens 增量/压缩差额补偿为推断不采纳 |
| A35 | [Google Antigravity](https://antigravity.google/) CLI/扩展 | `~/.gemini/antigravity[-cli]/conversations/<uuid>.db`：gen_metadata protobuf 逐回合 usage（input=固定提示+新增、cacheRead、output、thinking、responseId）；布局系逆向结论且 1.1.18 时间戳字段变更；IDE 主体用量经 language server，需另行取证 |
| A36 | [Junie](https://junie.jetbrains.com/) CLI（JetBrains；tokscale 1d9a939 sessions/junie.rs，2026-09-29 采信） | `~/.junie/sessions/<session-id>/events.jsonl`：LlmResponseMetadataEvent.modelUsage[] 逐轮（model、input/output、cache、reasoning、cost、time、provider）；timestampMs 为结束时刻；IDE 插件本体仍缺证留 F1 |
| A37 | [Kiro](https://kiro.dev/)（AWS；tokscale 1d9a939 sessions/kiro.rs，2026-09-29 采信） | 三载体：CLI `~/.kiro/sessions/cli/*.json(+jsonl)`、kiro-cli `~/.local/share/kiro-cli/data.sqlite3` conversations_v2、IDE globalStorage `kiro.kiroagent`（.chat/execution/promptLogs）；Auto agent 常记 0，估算路径不采纳；metering credit 为独立计价单位 |
| A38 | [Zed threads 存储](https://github.com/zed-industries/zed)（官方核验 bd74733 + 本机 schema 只读核验 2026-09-29） | `~/.local/share/zed`、`~/Library/Application Support/Zed`、`%LOCALAPPDATA%\Zed` 下 threads/threads.db：threads 表 data blob（json 或 zstd）含 request_token_usage 逐次（input/output/cache_read/cache_creation）与 cumulative；仅 zed.dev hosted 计入，imported 线程跳过 |
| A39 | [Codebuff](https://codebuff.com/)（原 Manicode；官方核验 caec5fc，2026-09-29） | `~/.config/manicode*/projects/*/chats/<chatId>/chat-messages.json`；CODEBUFF_DATA_DIR 覆盖；usage 字段待 fixture |
| A40 | [Command Code](https://commandcode.ai/)、[仓库](https://github.com/CommandCodeAI/command-code)（仓库无源码；npm 分发物 command-code@1.69.0 dist/cli.mjs 逐行核验 2026-09-29） | `~/.commandcode/projects/<slug>/*.jsonl` v3 树形（session/message/model_change；assistant usage inputTokens/outputTokens/cacheRead/cacheWrite/costUsd）；rewind 孤儿分支不计、fork 复制按 id+时间戳去重 |
| A41 | [jcode](https://jcode.sh/)（开源 Rust 终端 Agent；官方核验 1jehuang/jcode 4f6bf8e，2026-09-29） | `~/.jcode/sessions/session_*.json` 快照 + `.journal.jsonl` 追加日志（journal 覆盖快照）；input/output/cache_read/cache_creation/reasoning 五字段；OpenAI/Anthropic 缓存口径差异需归一 |
| A42 | gajae-code（`gjc`；官方核验 Yeachan-Heo/gajae-code 7e54f9c + docs/session.md，2026-09-29） | `~/.gjc/agent/sessions/<slug>/*.jsonl`（pi 血统：session 头 + assistant model/provider/usage 五桶 + cost.total）；深度 1/2 子代理重放需去重；GJC_CONFIG_DIR/PI_CONFIG_DIR/XDG 覆盖 |
| A43 | [Xum](https://github.com/coder/xum)（Coder；原 coder/mux） | `~/.mux/sessions/<workspaceId>/session-usage.json`：byModel 会话级聚合（input/cached/cacheCreate/output/reasoning + cost_usd）；仅会话级；产品更名需双根发现 |
| A44 | [Warp](https://www.warp.dev/) | 本地仅见账户级用量缓存（requestsUsed/spendCents/syncedAt，工作区级），无 token 明细；属额度数据不入 token 统计 |
| A45 | [Cursor CLI 文档](https://cursor.com/docs/cli/overview)、[CursorDump 转录分析](https://github.com/lpalbou/CursorDump) | CLI 转录 `~/.cursor/projects/<slug>/agent-transcripts/<uuid>/*.jsonl` 有会话但无 token/模型逐次字段；逐次用量仅远端 dashboard（get-filtered-usage-events）→ 按本地边界排除；IDE state.vscdb 待证 |
| A46 | [iFlow CLI](https://github.com/iflow-ai/iflow-cli)（阿里心流，闭源；官方核验 4642808：__2026-04-17 停服__，2026-09-29） | `~/.iflow`（settings.json、`tmp/<project_hash>/`）官方文档证实；`/chat save` JSON 持久化；OTel 事件含逐次 api_response 五桶 token（与 Gemini CLI 同构）；chats 载体具体路径/usageMetadata 待本机样本 |
| A47 | [Qoder](https://qoder.com/)（阿里；前通义灵码，2026-05 品牌升级；docs.qoder.com + npm @qoder-ai/qodercli 1.1.64 解包取证 2026-09-29） | CLI 设备流在 `~/.qoder/`，IDE 为 Electron `%APPDATA%\com.qoder.app.stable*`；CLI 会话格式与 usage 字段未证，需本机取证 |
| A48 | AtomCode（AtomGit 生态，联合华为 InsCode AI IDE；官方核验 e4215f733eeba4cede553e28f9b559e6b3dc34ef（GitHub 镜像同 SHA），2026-09-29） | CLI 形态国产 Agent（GitHub 2100+ star、17.4 万下载，2026-06 报道）；本地存储格式未证，开源可后续源码核验 |

固定源码提交通过 GitHub 公共 API 的 commits 结果再次核对；
文件读取使用 raw.githubusercontent.com，未执行任何上游代码。
滚动页面没有与本机发行版做对应验证，接入时必须补版本清单及样本。
CodeBuddy 页面在浏览工具返回 502，改用 PowerShell 只读获取官方 HTML 后核验正文；
没有把失败页面当作证据。WorkBuddy 和 ZCode 正文也通过该方式核对。
Hermes 固定 commit 通过 commits API 获取，定点只读核对存储文档、schema 与 usage 相关片段。
追加获取 agent/usage_pricing.py 时遇到 HTTP 429；停止该读取，不把外层 PowerShell 退出码 0
当作此文件获取成功，不据响应字段名称推断缓存包含关系；此项留给 M3 的版本样本核验。

## M8 格式依据（核验于 2026-09-29）

在覆盖调研基础上，逐产品核验格式证据；产物锚点
已并入上表 A25–A48 各行；字段级证据全文在各适配器源文件头与
[M8 验证记录](../../validation/desktop-usage/m8-second-batch.md)：

- 官方源码核验（固定提交）：Roo b867ec9（已归档）、Goose a701bb1、Crush 1f3827b、
  jcode 4f6bf8e、gajae 7e54f9c、Continue 5522c6f、AtomCode e4215f7、Zed bd74733、
  Aider 5dc9490、Amazon Q 15cc8f3、Codebuff caec5fc、iFlow 4642808。
- 官方分发物核验：Command Code npm 1.69.0 dist/cli.mjs（仓库无产品源码）、
  Qoder npm 1.1.64（混淆 bundle，仅路径可信）。
- 第三方解析器证据（闭源）：tokscale 1d9a939 的 Amp/Grok/Junie/Kiro/Droid/Xum/
  Antigravity 实现逐行提取。
- 重大结论：Goose 存在 tokscale 未覆盖的逐请求 usage_ledger；Crush token 列是
  上下文快照非用量；Amazon Q/Codebuff 本地无逐次 token 载体（不实施）；
  iFlow 已停服（2026-04-17）；Roo 与 cline 的 tokensIn 包含关系存在血统分歧
  （各按锚点实现，待真实样本复核）。

## 研究边界

远端计费 API/组织数据不在范围内；真实来源仍按逐版本证据核验，缺失样本不推断。
JetBrains 自家 AI Assistant/TRAE 等缺证 IDE 留 F1；Zed/Junie CLI 在 M8。
包体、性能、平台和真实来源结果分别见验证记录，设计目标不作为测量值。

## 扩展覆盖的证据层级

扩展来源见上表 A25–A48；当前能力及阶段划分见
[接入矩阵](adapters.md#扩展覆盖)与 [执行计划 M8](execution.md#m8)。

方法与证据层级：

- 以第三方开源解析器 [tokscale](https://github.com/junhoyeo/tokscale)
  （固定 main `1d9a9395418efc6952944b794097935d7d6fa1e8`）作为路径/字段线索层，
  只读其 clients.rs、scanner.rs 与 sessions/ 解析器源码；其估算、比价与
  聚合策略不沿用，仅取存储路径、字段名与生命周期线索。
- 产品身份逐个经官方站点、官方仓库或多方报道核对：goose 仓库已由
  block/goose 迁移为 aaif-goose/goose；coder/mux 更名 coder/xum；
  Qoder 为通义灵码品牌升级（2026-05）；Grok Build 为 xAI 官方 CLI（新闻佐证）；
  Command Code、jcode、gajae-code、Codebuff、AtomCode 均有官方站点或发布渠道。
- 闭源产品（Amp、Grok Build、Junie CLI、Kiro、Droid、Xum、iFlow CLI、
  Qoder CLI、Antigravity protobuf 布局）的字段证据来自第三方解析器或逆向分析，
  能力与真实验收按接入矩阵逐版本维护；合成回归不认证本机输出。
- 明确排除项：Cursor 逐次用量仅在远端 dashboard（get-filtered-usage-events）；
  TRAE 的 tokscale 路线来自其 usage API（远端）；Warp 本地仅账户级
  requests/spend 缓存。三者均不符合"只统计本机来源"边界，不入 token 统计。
- 既有条目证据升级：Zed 内置（A23 补充 A38）发现 threads.db 本地逐次载体；
  JetBrains（A07）补充 Junie CLI 本地 events.jsonl 证据；两者从 F1 缺证类
  已在 M8 注册；JetBrains AI Assistant IDE 插件与 TRAE 仍在 F1。
- 缺证 IDE 家族（F1 扩充，不探测/不实施）：Cursor IDE、Windsurf（IDE+CLI）、
  京东 JoyCode、智谱 CodeGeeX 插件、百度文心快码 Comate、华为 InsCode/CodeArts Snap。
  每项保留产品身份与"待本地格式证据"状态，不写"不支持"。

这些来源不授权访问登录后台、计费 API 或组织数据；实现与验证按当前合同执行。

<a id="verification-20260929"></a>
<a id="verification"></a>
<a id="readiness-verification"></a>

## 当前验证入口

设计与计划只保留当前要求；实际命令、版本、环境、结果及缺口集中在
[最新验收](../../validation/desktop-usage/current-acceptance.md)。专项格式依据见
[验证目录](../../validation/desktop-usage/)，旧过程由 Git 保留。
文档检查不能代替业务、真实来源或原生平台验收。
