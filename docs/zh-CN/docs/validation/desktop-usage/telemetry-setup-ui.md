# 全年活动图、Copilot 图表与遥测配置入口

<a id="annual-activity-copilot-charts-and-telemetry-configuration"></a>

2026-10-01，Windows 11 x64。本轮保留此前 Copilot 审查及其他未提交修改；
没有提交、推送、修改真实 Agent 配置或发送模型调用。
配置要求见 [Copilot OTel](../../design/desktop-usage/copilot-otel.md)。

<a id="changes-and-scope"></a>

## 修复与范围

- 热力图原来复用最近 30 天查询；现在以统计时区的当前年份查询 Jan 1–Dec 31，
  可切换年份，闰年 366 格。未来、真实零调用、无按日历史及部分覆盖分别保留。
  星期分布独立查询统计筛选范围，避免年份切换或跨年范围使其漏掉日期；星期名使用 UTC 日历。
- Copilot 输入存在，但缓存/未缓存拆分未知，原图只画拆分而隐藏输入；现增加已报告输入曲线。
  总量未知时显示已知输入/输出分量和提示，仍不推导完整 total。
  趋势及总览今日悬浮保留未知总量、已知分量与实测零值；零值保留为实测零，不显示为无数据。
- 启动后使用 blocking worker 异步检查，前端共享结果；总览提示缺失配置，设置常规页
  提供完整列表、重查、预览、合并应用及当前进程内撤销。
- VS Code 默认用户/profile、Insiders、Agent Host、Gemini、Qwen、Claude、Codex、
  CodeBuddy 采用各自配置；CLI Copilot 创建进程级 launcher，不改自动管理的状态文件。
  检测 PATH 文件与已有用户目录不执行 CLI，常规安装清单用于验证 VS Code 配置键。
- JSONC 精确区间编辑，TOML 保留格式；有界读取、重复键/过深嵌套/链接/只读/并发检查。
  Codex exporter 按叶字段合并，普通表/内联表的既有 headers 与其他选项保留；冲突形状不改写。
  备份原始字节，临时文件替换；profile 同步排除项合并默认用户文件并有条件回滚。
  撤销跳过用户后改的值；旧输出目标、企业策略、启动环境与 Sync 显式冲突不被强改。
- 本机接收器即时绑定后才写 OTLP 设置；失败撤回本次启用。修正 OTLP AnyValue
  字段编号及旧测试编码，protobuf 成功响应为空 protobuf；增加 logs 只接收明确允许的属性。
  logs/supplemental traces 独立存放，防止加入已有原生统计。

<a id="证据"></a>

<a id="核验依据"></a>

<a id="source-references"></a>

## 核验资料

共同字段：verified_at=2026-10-01，owner=仓库维护者；相关配置任务、客户端更新或
格式/策略变化时复核。rolling 文档证明配置方式，不等于安装版本真实导出验收。

| ID / scope | source_url / source_version | method / status / impact |
| --- | --- | --- |
| T01 Copilot file 与 Agent Host | [既有固定版本源码依据](../../design/desktop-usage/copilot-otel.md#依据与本轮验证)，dc546cc3c9979a19adafccd439889d7b64298def | 源码与本机 manifest 只读核验；扩展 0.68.0 声明、app 1.140.0 基线；新导出未启用 |
| T02 Sync 作用域 | [userDataSync.ts](https://github.com/microsoft/vscode/blob/dc546cc3c9979a19adafccd439889d7b64298def/src/vs/platform/userDataSync/common/userDataSync.ts) | 固定源码确认 ignoredSettings 是 APPLICATION，默认空列表；profile 必须另改默认用户文件 |
| T03 Gemini outfile/home | [官方遥测](https://geminicli.com/docs/cli/telemetry/)、[Storage](https://github.com/google-gemini/gemini-cli/blob/main/packages/core/src/config/storage.ts)、[paths](https://github.com/google-gemini/gemini-cli/blob/main/packages/core/src/utils/paths.ts)，rolling | 官方文档/源码；HOME 替换后附加 .gemini；仅合成写入验证 |
| T04 Qwen 配置与内容开关 | [官方设置](https://github.com/QwenLM/qwen-code/blob/main/docs/users/configuration/settings.md)、[遥测](https://github.com/QwenLM/qwen-code/blob/main/docs/developers/development/telemetry.md)，rolling | 官方正文；QWEN_HOME 与 runtime 不同，敏感日志/span 开关关闭；仅合成验证 |
| T05 Claude logs/env | [官方监控](https://code.claude.com/docs/en/monitoring-usage)，rolling | 官方正文确认 logs 独立 HTTP/JSON endpoint 与内容开关；不配置 beta traces 或不支持的 metrics |
| T06 Codex TOML/exporter | [官方示例](https://developers.openai.com/codex/config-sample/)，rolling | 官方正文确认 HTTP/JSON logs exporter 与 log_user_prompt；使用 openai-docs Skill 核验 |
| T07 CodeBuddy env/traces | [官方用户设置](https://www.codebuddy.ai/docs/cli/settings)、[官方监控](https://www.codebuddy.ai/docs/zh/cli/monitoring)，rolling | settings 正文经公开 HTML 核对 env/session 语义；仅 HTTP/protobuf traces；未声称本机新版导出已验收 |
| T08 CLI 用户状态 | [官方配置目录](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-config-dir-reference)，rolling | config.json 为自动管理状态、settings.json 为用户设置；未证实用户 telemetry 键，故仅创建 launcher |
| T09 OTLP 编码 | [common.proto](https://github.com/open-telemetry/opentelemetry-proto/blob/main/opentelemetry/proto/common/v1/common.proto)、[logs.proto](https://github.com/open-telemetry/opentelemetry-proto/blob/main/opentelemetry/proto/logs/v1/logs.proto)，rolling | 官方 proto 核对字段及独立标准样本；旧整数/布尔/双精度编号错误已修复 |

公开资料核验结果保存在忽略目录 `build/telemetry-setup-evidence/`。
固定 Sync 源码 SHA256：`bdff135f2943761075df3e238a6bc190c1dd5289e033585acbfebfa3835bf329`。

<a id="local-read-only-checks"></a>

## 本机只读检查

本节记录前一轮检查，当时用户目录仍可提供配置候选；下方 UI 调整后的检查收紧安装依据。
显式执行 `cargo test --manifest-path desktop/src-tauri/Cargo.toml --locked -p llm-usage-desktop local_configuration_audit -- --ignored --nocapture`，退出码 0。
最终探测识别 VS Code 默认用户及一个 profile、各自 Agent Host、Codex、Copilot CLI、
CodeBuddy 的缺失用户层输出。只输出客户端 ID、状态和原因，不输出配置正文或凭据。
该检查只读，不以本机存在用户目录证明 CLI 的当前版本，也不保证另一个启动环境无覆盖。
配置候选检查通过不等于输出文件或 token 记录已产生。

<a id="checks-and-missing-coverage"></a>

## 验证与缺口

本节为前一轮验收基线；后续批量入口的验收见下一节。
环境：Node.js 24.21.0、Rust/Cargo 1.98.1、Edge 154.0.4258.48，Windows 11 x64。
所有命令从仓库根执行，以下退出码均为 0。

| 命令 | 结果 |
| --- | --- |
| `npm run verify` | Markdown 163 文件、资源 67 派生/82 文件、脚本测试 3、UI 测试 10；svelte-check 0 错误/警告；fmt、clippy `-D warnings` 通过；Rust 752 通过/1 默认忽略；前端 745 模块构建通过 |
| `npm run test:browser` | 真实 Edge、9 个截图、159 次合成 IPC；pageerror 为空，全部交互断言通过 |
| `cargo test ... local_configuration_audit -- --ignored --nocapture` | 默认忽略的本机只读审计显式运行通过，7 个配置候选 |
| `python build/copilot-otel-setup/check-links.py` | 6 份受影响文档的 98 个相对链接通过；不宣称外部链接或章节定位的在线验收 |
| `git diff --check` | 通过；临时产物仅在已忽略的 `build/` 下 |

完整日志在 `build/telemetry-setup-evidence/{verify,browser,local-audit}.log`，
截图和交互结果在 `build/browser-smoke/`；均不进入版本库。
浏览器使用真实 Edge、Vite 和 IPC 合成响应，不连接真实 Agent。
已验证全年/闰年/未来格、已知输入但缓存未知、未知 total 与零输出悬浮、异步检查不写配置、
总览/设置入口、预览/应用/撤销、十语言、主题和窄屏，以及原有筛选/用户/分页回归。
Rust 配置测试全部使用仓库 `build/` 中合成文件；包含 profile 双文件失败回滚、
Codex 内联表/普通表保留和撤销、来源发行者核对、深度上限及标准 OTLP 编码。

未验收：macOS/Linux 桌面、portable/custom user-data 自动发现、真实用户配置写入及
重载后的新 OTel 导出、按新导出自动入统计、跨格式统计来源选择、trace SQLite 与持久化撤销。
新导出目前用于补充核验，不能解释为已补齐历史或已纳入总 token。

<a id="overview-simplification-and-batch-configuration-later-2026-10-01-stage"></a>

## 总览简化与批量配置（2026-10-01 后续）

总览仅显示简要提示与“一键开启全部”“查看详情”；详情直接选择设置中独立的
本机遥测面板并定位焦点。该面板保留单项预览/合并应用/撤销，新增批量按钮。
已配置目标、管理策略及其他阻止自动配置的项目跳过；每项现场预览后立即应用，
避免多个 profile 共享设置文件造成预先生成的计划过期。单项失败继续后续项目，
报告成功、失败和手工项；操作状态与撤销入口在切换页面后保留。

安装检查只认可 PATH 中的 CLI 启动文件或经过发行者核对的 IDE 安装清单；
独立 Copilot 扩展还要求对应 IDE 的启动文件。残留用户目录、globalStorage、profile
与缺少对应 IDE 安装依据的扩展缓存不会单独使 Agent 显示。没有配置文件的已安装 CLI 仍显示。
CLI 非 PATH 安装、portable/custom user-data 与桌面平台的既有验收缺口继续保留。

新增测试：共享设置写入顺序、失败继续与错误脱敏、无可配置项/检查失败零写入；
安装残留排除与独立扩展的宿主依据；总览只有两个按钮、不列路径/键/Agent，
详情导航与焦点、执行中切页、部分失败重试、跨页撤销和三语言窄屏。
沿用上述 Windows/Node/Rust/Edge 环境，以下命令从仓库根执行，最终退出码均为 0。

| 命令 | 最终结果 |
| --- | --- |
| `npm run verify` | Markdown 163 文件；资源 67 派生/82 文件；脚本 3、UI 13、Rust 754 通过/1 默认忽略；svelte-check 0 错误/警告；fmt/clippy 通过；前端 746 模块构建通过 |
| `npm run test:browser` | 两次独立执行通过，每次 192 次合成 IPC；25 类检查、11 张截图，pageerror 为空；既有统计/用户/分页断言继续通过 |
| `cargo test ... local_configuration_audit -- --ignored --nocapture` | 1 个显式只读测试通过，识别 VS Code 默认用户/profile 各自扩展和 Agent Host，以及 Codex，共 5 项 |
| `python build/copilot-otel-setup/check-links.py` / `git diff --check` | 6 份文档、98 个相对链接及补丁空白检查通过 |

本机 PATH 未找到 Copilot CLI、CodeBuddy 等 CLI 启动文件；它们的用户目录不再单独
进入配置候选。这不证明非标准路径的程序已卸载，也不删除既有统计。
日志保存在 `build/telemetry-batch-ui/{verify,browser,browser-recheck,local-audit}.log`，
页面截图和合成交互结果仍在 `build/browser-smoke/`；已人工核对总览与窄屏详情截图。

执行中切页的回归采用可显式释放的异步 mock，避免短定时器在导航完成前已经结束。
调试时另出现一次初始卡片加载 30 秒超时，未获得当次页面错误；增加失败 JSON/截图
采集后，两次独立运行同一组断言通过，启动超时原因未确定。未放宽业务断言。
未修改真实 Agent 配置，未执行 Agent；既有真实写入/新导出/跨平台验收缺口继续保留。
