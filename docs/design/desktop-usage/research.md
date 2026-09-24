# 调研依据与本轮验证

核验日期：2026-09-24；工作区：D:/workspace/github/owent/llm-usage。
首次设计轮开始时 HEAD 为 `8080a0534b58aaa49e5fb7e5c5e212b7c9f92b70`，
唯一 `git status --short` 项为未跟踪的 previous-draft/。
本轮读取其源代码和 README，未打开 usage.db 内容、未读取 data.json 的个人统计，未运行 collect.py。

来源以官方正文、维护者固定源码和本地原型为层级，搜索摘要只用于定位。
滚动文档不代表本机安装版本；源码字段存在不等于完整运行路径已验收。
所有 Agent 的本机版本、真实输出和运行兼容性均未验证。

本次完善按用户新决策：Windows 11 x64 首发、GitHub 三平台 CI、可选 WSL 构建；
Harness Agent 明确为 Nous Research Hermes Agent；全部 Agent 尝试本地提取，支持定时任务，
仅统计本机来源。原设计的企业 API/账号报表接入已移出范围，保留相关来源只解释排除原因。
开工准备补充决定：实施时允许提取本机真实 Agent 数据验证；缺证 IDE 后移 F1，当前不实施；
M0 固定平台基线并探测 WSL 的步骤已确认。设计准备完成，技术实现及运行证据仍待 M0–M7。

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
| T08 | [Microsoft WSLg](https://learn.microsoft.com/en-us/windows/wsl/tutorials/gui-apps) | Linux GUI 需 WSL 2，WSLg 不等于完整 Linux 桌面；本机环境未探测 |
| T09 | [Task Scheduler](https://learn.microsoft.com/en-us/windows/win32/taskschd/task-scheduler-start-page)、[身份](https://learn.microsoft.com/en-us/windows/win32/taskschd/security-contexts-for-running-tasks)、[错过时点](https://learn.microsoft.com/en-us/windows/win32/taskschd/tasksettings-startwhenavailable) | 系统触发/运行上下文有官方 API；选择普通当前用户的无窗口提取，注册权限及恢复需 M6 实测 |
| T10 | [Tauri WebDriver](https://v2.tauri.app/develop/tests/webdriver/) | 当前文档有嵌入式跨平台服务；直接 tauri-driver 仍区分 Windows/Linux 与 macOS；测试插件不得进入发行制品 |

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
| A04 | [CodeBuddy monitoring](https://www.codebuddy.ai/docs/cli/monitoring) | OTel http/protobuf、model_stream/model_request 重叠、缺 usage 的情况及内容采集开关；IDE 另验 |
| A05 | [Copilot CLI reference](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-command-reference) | OTLP JSON/protobuf 与 file JSONL、chat span；不依赖私有 SQLite schema 承诺 |
| A06 | [VS Code monitoring](https://code.visualstudio.com/docs/agents/guides/monitoring-agents) | chat 与 invoke_agent 不同层级；可选缓存/推理与 TTFT 字段；只覆盖文档指定 Agent |
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
| A22 | [WorkBuddy 用量](https://www.workbuddy.cn/docs/workbuddy/Usage) | 官方积分/套餐查询已读；未证实逐次 token/cache 的可提取合同 |
| A23 | [Zed telemetry](https://zed.dev/docs/telemetry) | hosted 计量和本地遥测日志有区别；不能据服务端 token 计量推出本地完整字段 |
| A24 | [Hermes 官网](https://hermes-agent.nousresearch.com/)、[存储文档](https://hermes-agent.nousresearch.com/docs/developer-guide/session-storage/)、[固定存储说明](https://github.com/NousResearch/hermes-agent/blob/ef70b3661cbfcf57e583008ad91dd04d8ba46070/website/docs/developer-guide/session-storage.md)、[usage](https://github.com/NousResearch/hermes-agent/blob/ef70b3661cbfcf57e583008ad91dd04d8ba46070/hermes_state_usage.py)、[schema](https://github.com/NousResearch/hermes-agent/blob/ef70b3661cbfcf57e583008ad91dd04d8ba46070/hermes_state_schema.py)、[response usage](https://github.com/NousResearch/hermes-agent/blob/ef70b3661cbfcf57e583008ad91dd04d8ba46070/agent/turn_usage.py) | 用户已明确产品；本地 state.db/profile、模型/task 累计、增量/absolute、辅助不写 sessions、历史回填已核对；逐次日志和 normalize_usage 完整语义仍待验 |

固定源码提交通过 GitHub 公共 API 的 commits 结果再次核对；
文件读取使用 raw.githubusercontent.com，未执行任何上游代码。
滚动页面没有与本机发行版做对应验证，接入时必须补版本清单及样本。
CodeBuddy 页面在浏览工具返回 502，改用 PowerShell 只读获取官方 HTML 后核验正文；
没有把失败页面当作证据。WorkBuddy 和 ZCode 正文也通过该方式核对。
Hermes 固定 commit 通过 commits API 获取，定点只读核对存储文档、schema 与 usage 相关片段。
追加获取 agent/usage_pricing.py 时遇到 HTTP 429；停止该读取，不把外层 PowerShell 退出码 0
当作此文件获取成功，不据响应字段名称推断缓存包含关系；此项留给 M3 的版本样本核验。

## 研究限制与实施阶段核验

- 本轮没有访问任何登录后台、计费 API 或组织数据；这些远端来源已排除在产品范围外。
- 实施阶段提取真实数据验证已获允许；本轮未提取样本或打印真实对话，原型数据库尚未审计。
- Kimi 新 wire、Kimi Work、ZCode、WorkBuddy 留 M4；JetBrains/TRAE、Zed 内置与缺证 IDE 变体后移 F1。
- 未进行包体基准、数据库性能、跨平台测试或真实 LLM 验收；性能数字均是设计目标。
- 平台和 Hermes 身份已由用户明确；WSL 可用性、三平台 CI、系统任务及真实适配器仍未执行。
- 没有新建 Agent 专属配置或新 Skill；现有维护 Skill 保留触发方式，更新命令引用及受影响的维护合同。

<a id="verification"></a>

## 首次设计轮文档验证

环境：Windows、PowerShell 7.6.6、Node.js v24.21.0；已有 markdownlint-cli2 0.23.3。
根 package.json 与 package-lock.json 实际缺失，git ls-files 也未列出；
因此没有运行 npm ci 或 npm run lint:docs，没有安装依赖。
初始普通 exec 启动失败 `CreateProcessAsUserW failed: 5`，经平台允许的权限重试恢复；
该故障是执行基础设施问题，不是业务测试失败。

以下命令 cwd 均为仓库根，仅执行文档检查：

```powershell
node node_modules/markdownlint-cli2/markdownlint-cli2-bin.mjs
node node_modules/markdownlint-cli2/markdownlint-cli2-bin.mjs --no-globs Plan.md README.md AGENTS.md 'docs/design/desktop-usage/*.md' '.agents/skills/ai-maintenance/SKILL.md' '.agents/skills/ai-maintenance/references/maintenance.md' '.agents/skills/ai-maintenance/references/source-index.md'
python -X utf8 C:/Users/owt50/.codex/skills/.system/skill-creator/scripts/quick_validate.py .agents/skills/ai-maintenance
git diff --check
git status --short
```

| 检查 | 退出码/结果 | 说明 |
| --- | --- | --- |
| 全仓 markdownlint | 1；22 个文件，最终剩 6 个问题 | 均在原有 previous-draft/README.md：2 个围栏语言、4 个表格空格；保留原型未修改 |
| 本轮 markdownlint | 0；13 个文件，0 个问题 | 使用 --no-globs 显式给出本轮文件，保留根规则；没有修改全局 ignores 或关闭规则 |
| 本地链接/锚点/空白检查 | 0；13 个文件、70 个本地链接、无错误 | 一次性 Python 只读检查，包含未跟踪文件；非业务测试 |
| Skill quick_validate | 0；Skill is valid | 首次系统默认 GBK 解码失败，显式 -X utf8 后通过；未改变 Skill description 或发现策略 |
| git diff --check | 0 | Git 的 LF→CRLF 提示不是空白错误；新增文件另查内容 |
| 原型输入哈希复核 | 三个基准文件均一致 | store.py、collect.py、collectors/__init__.py 与前文摘要一致；本轮无原型写操作 |

最初调用 markdownlint 的库模块没有产生检查输出；核对 package.json 的 bin 后改用上面的
markdownlint-cli2-bin.mjs，库模块的退出码不计为验证。命令行单独传负 glob 也未排除原型，
最终使用 --no-globs 显式文件清单，不把前一次退出码 1 记成通过。
Skill 仅调整验证命令引用和已有表格格式，静态检查不代表真实模型触发或质量评估；后者未执行。

业务实现、采集、数据库迁移、外部配置改动、真实服务测试、提交/推送/部署均未执行。

## 决策完善轮验证

2026-09-24 根据用户决策修改计划与受影响文档，新增 scheduling.md、platform-ci.md，
同步根 AGENTS.md、README.md 和维护 Skill 的 maintenance.md 引用内容。
没有修改 Skill 名称、描述、触发逻辑或脚本；本轮不重复运行真实模型评估。

cwd 为仓库根，使用已有 Node/Python 文档工具，未安装依赖：

```powershell
node node_modules/markdownlint-cli2/markdownlint-cli2-bin.mjs --no-globs Plan.md README.md AGENTS.md 'docs/design/desktop-usage/*.md' '.agents/skills/ai-maintenance/references/maintenance.md'
git diff --check
git status --short
```

本轮 13 个文件 Markdown 检查退出码 0，0 个问题；一次性 Python UTF-8 只读检查
73 个本地链接/锚点、末尾换行和行尾空白，退出码 0，无错误；三份原型输入 SHA-256 均未变化。
git diff --check 通过，未跟踪设计文档另行读取核对；未改动 previous-draft。
全仓 Markdown 的原型遗留问题仍参见前轮记录，不把本轮定向检查写成全仓通过。

本轮仅公开资料研究和文档维护；未读取个人 Agent 数据，未创建 CI workflow、注册系统任务、
启动 WSL、构建客户端、安装依赖、调用模型或接入远端用量 API。V01–V27 均为未来业务验收计划。

<a id="readiness-verification"></a>

## 开工准备轮检查

2026-09-24 按用户最新决定明确实施阶段的本机数据验证许可，缺证 IDE 后移 F1，
平台方案及 M0 核验时机已确认。增加 implementation-readiness.md，同步根计划、接入矩阵、
执行详情、范围/验收说明、平台文档、根规则和维护 Skill 的参考文档；不修改 Skill 的触发方式。

本轮复核实际仓库，仍未发现根 package.json/package-lock.json、Cargo.toml 或 Rust 工具链声明；
构建/版本锁定留 M0。已有设计/规则修改及未跟踪原型均保留，未执行实现、安装、个人数据采集或 CI。
外部产品技术事实未新增，本轮沿用前轮已核验来源，只调整用户决策与实施分期。

验证 cwd 为仓库根，使用已有工具；以下检查均为文档准备证据：

```powershell
node node_modules/markdownlint-cli2/markdownlint-cli2-bin.mjs --no-globs Plan.md README.md AGENTS.md 'docs/design/desktop-usage/*.md' '.agents/skills/ai-maintenance/references/maintenance.md'
git diff --check
git status --short
```

- Markdown：14 个文件，退出码 0，0 个问题。
- 一次性 Python UTF-8 只读检查：14 个文件、91 个本地链接/锚点，末尾换行及行尾空白无错误，退出码 0。
- 阶段与验收编号：M0–M7/F1 齐全且不重复，V01–V27 共 27 项保留；后移 IDE 不再列入 M4/M5。
- git diff --check：退出码 0；未跟踪文档单独检查，三个原型基准文件 SHA-256 与首次记录一致。
- 原型 README 的既有全仓 lint 问题未修改；本轮定向检查不代表全仓或业务测试通过。

准备结论：原待用户决策项已处理，开工前设计与文档检查完成。实施从 M0 开始；
本轮没有提取个人 Agent 数据、安装依赖、执行构建、创建 CI、注册任务或更改外部配置。
