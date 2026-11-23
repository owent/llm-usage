# 当前维护验证

核对日期：2026-11-17。维护验收源码版本为 0.3.9，发布准备版本为 0.3.10。
环境为 Windows x64、Node 26.13.0、
npm 12.0.2、pnpm 12.21.0 和 Rust 1.99.1。命令从仓库根目录运行；临时脚本、日志、
公开目录原始数据及隔离数据库位于 `build/maintenance-20261116/`。
用户随后授权提升版本、提交、附 changelog 的 annotated `v0.3.10` tag、推送及发布工作流。
全部 manifest/lock 版本已同步；现有工作流在平台检查后生成草稿。下文的本地可执行程序及完整
维护检查发生在仅改版本号之前，tag CI 单独验证 0.3.10。发布准备产物位于 `build/release-0310/`。

## 计划核对

核心存储、采集、桌面查询/设置/调度、费用、交换、更新、多语言及文档均已有实现和限定范围的检查。
用户确认文档部署完成。[Plan.md](../../../../../Plan.md) 现仅保留当前工作及未满足条件，
[执行要求](../../design/desktop-usage/execution.md) 汇总已交付范围。独立 Qoder IDE/JetBrains
token 读取器及其他 F1 候选仍未实现或未核验。真实公开版本/安装器及其他平台 GUI 更新验收
与代码完成分别记录。

<a id="custom-provider-official-price-fallback"></a>

## 自定义 Provider 的官方价格回退

2026-11-17 在 Windows x64、Node.js 26.13.1 和 Rust 1.99.1 环境核对。临时探针、公开目录数据
和译文输入位于 `build/pricing-custom-provider-20261117/`。最小回归使用
`zhipu-ai-(coding-plan)`、`glm-5.3-flash`、最新内置 Z.ai USD 价目及一个较低优先级的冲突
CNY 快照。旧候选顺序在应用快照优先级之前跨快照比较币种，返回 `channel_unknown`。
Provider ID 原本就允许自定义，本次不增加 Provider 名称别名。

回退选择现在先限定用户配置的 region/channel 或官方 global 范围，再按快照优先级选择，
随后在所选快照内优先 API 渠道。较低优先级的冲突不再遮挡已选价目；该快照内部的币种或渠道
冲突仍返回 `channel_unknown`。精确型号、官方供应商限制、token 覆盖范围和上下文档位规则不变。
裸模型名及带 Provider 前缀的模型名都覆盖发生时和当前价估算。计价规则标记升级为
`official-reference-6`，只修正仍保留明细的未封存日期一次，不改写封存历史，也不需要清库。

2026-11-17 重新读取[官方 Z.ai 价格页](https://docs.z.ai/guides/overview/pricing)，
GLM-5.3-Flash 每百万 token 的输入、缓存输入、输出价格分别为 USD 0.15、0.03、0.50。
models.dev 公开响应为 HTTP 200、6,689,815 字节，SHA-256 为
`01221db67a47999df0d93c2e7d85ad70172bc6732815751da164ce875231c88e`，同时包含该模型的
Z.ai 和智谱条目；回归验证较高优先级的内置官方条目优先。对应用数据库的只读检查仅限该模型的
价格行、计价默认值、规则标记及聚合费用/事件字段，未读取会话内容、来源路径或凭据。
数据库中没有本次报告的准确自定义 Provider，因此新验收用例采用合成 token 和真实价格行结构，
不声称已完成真实事件 GUI 或账单验收。

| 检查 | 结果 |
| --- | --- |
| 修复前的自定义 Provider 回归 | 退出码 1：`custom_provider_uses_the_preferred_same_model_official_snapshot` 因自定义 Provider 未获得官方同型号价格而失败；修改后通过。 |
| 七组价格/看板测试 | 退出码 0：57 项通过，包括同快照歧义、精确价格优先、旧规则修复及封存日保护。 |
| 隔离 SQLite 自定义 Provider 查询 | 退出码 0：事件提交、发生时日成本重算、当前参考汇总及模型行小计均选择 USD 和内置 Z.ai 条目。首次测试代码编译时，闭包遮蔽了事件辅助函数；重命名闭包后修复该错误并通过。 |
| `npm run verify` | 首次在 120 秒工具超时处以退出码 143 终止于全量 Rust 测试。将超时调整为 300 秒后完整重跑退出码 0；加入 SQLite 用例后的最终重跑也退出码 0：1,192 项 Rust 测试通过，十项既有原生/平台测试保持显式忽略；20 项脚本、34 项前端测试及 Svelte、rustfmt、Clippy 和生产前端构建通过。 |
| `npm run check:docs` / `test:docs` / `build:docs` | 退出码 0：215 对仓库文档、21 对指南、41 项文档测试、构建 1,427 页并检查 3,018 个输出文件。四个变更 Rust 文件的注释参考均已复核。 |

### 发布 0.3.13 的准备

用户已授权提升版本、提交、附 changelog 的 annotated tag、推送及发布工作流。发布准备产物位于
`build/release-0313/`。候选版本包含自定义 Provider 计价修复、有界 Codex 超长压缩记录流式读取
及汇总参考价格通用提示。根/桌面 npm 清单与锁文件、Tauri 配置、两个 Rust 包版本和 Cargo.lock
均为 0.3.13；共享 pnpm 锁文件没有需要修改的工作区版本字段。

打包脚本的 `releaseVersion()` 返回 0.3.13。最终 `npm run verify` 退出码 0：1,192 项 Rust 测试通过，
十项既有原生/平台测试保持显式忽略；20 项脚本、34 项前端测试及 Markdown、资源、Svelte、rustfmt、
Clippy 和生产前端构建通过。`npm run test:browser` 也退出码 0。完整的 annotated `v0.3.13`
消息作为 Release 正文。现有 tag 工作流先核验该准确源码和六个平台/架构包，再创建 Draft Release；
触发工作流本身不代表远端打包、附件核验或公开发布已经完成。

<a id="copilot-composite-and-omp-1822"></a>

## Copilot 复合型号计价与 oh-my-pi 18.22.0

2026-11-17 在 Windows x64、Node.js 26.13.1 和 Rust 1.99.1 环境核对。只读探针和隔离扫描数据库
位于 `build/pricing-followup-20261117/`。报告的 124 条未计价事件在 `assistant_usage_events`
schema 9 中分别为 `provider_id=github-copilot`、`model_raw=ZhiPu AI (Coding Plan)/glm-5.3-flash`。
原生 `model` 列保存文档规定的 provider-name/model-id 选择形式，`copilot_usage_model` 为 NULL。
旧参考解析因自定义前缀与来源渠道不同而保留完整字符串，产生 `channel_unknown`。

现在仅 Copilot CLI 在解析官方价格前去掉一个前导命名空间，剩余模型 ID 仍须精确匹配。原始
provider/model 及其他来源的未知命名空间不变。计价规则标记为 `official-reference-7`，只修正
仍保留明细的未封存日期一次。合成回归覆盖两种估算模式及 SQLite 事件提交、重算和汇总路径。

[oh-my-pi 官方 session 规范](https://github.com/can1357/oh-my-pi/blob/main/docs/session.md)将
`model_usage` 定义为会话外的模型调用，包含 purpose/provider/model/Usage/stopReason。解析器
`omp-session-2` 将其作为辅助模型调用导入，旧游标重放无冲突。无效 usage 保留调用和未知 token，
并保持需核对状态；`reset_boundary`、`mode_change` 是非用量元数据。`changes.jsonl` 仅在文件名与
`call`/`change` 审计形状同时匹配时排除，不只凭后缀跳过。

对 oh-my-pi 18.22.0 的完整只读原生扫描得到 216 个 active 文件，无未识别文件或诊断，来源健康
为 `ok`，共导入 835 条 `model_usage`。独立逐条原生记录重算完全一致：输入总量 9,974,920、
未缓存输入 933,304、缓存读取 9,041,616、缓存写入 0、输出 35,154、总 token 10,010,074。
三个 `changes.jsonl` 仅含文件变更审计字段，没有 model、usage 或 token 字段。无效 usage 和
旧游标重放回归与成功原生结果分别保留。

Codex 不需要修改产品代码。第 1492 行是有效的 17,260,078 字节 `compacted`，六项用量齐全。
当前文件为 `active_compat`、`has_record_errors=false`，并已越过该行继续读取。发布构建连续完成
九个有界窗口，导入 256 次调用和 53,711,098 token，与独立计算一致。最终来源健康为 `ok`；
此前的提示来自后续尚未完成的扫描，不是未恢复的记录。历史诊断没有删除。

| 检查 | 结果 |
| --- | --- |
| 修复前回归 | Copilot 复合型号计价及两个 oh-my-pi `model_usage` 用例在修改前失败；修改后均通过，包括无效 usage 分支。 |
| 定向测试 | 退出码 0：75 项，覆盖 oh-my-pi 接口/增量行为、Codex 超限恢复、价格归档/v29、看板修复和型号参考。 |
| `npm run verify` | 退出码 0：1,195 项 Rust 测试通过，十项既有原生/平台测试保持显式忽略；20 项脚本、34 项前端测试及 Markdown、资源、Svelte、rustfmt、Clippy 和生产前端构建通过。 |
| 文档与交互检查 | 退出码 0：215 对仓库文档、21 对指南、41 项文档测试、构建 1,427 页并检查 3,018 个输出文件。浏览器交互检查通过；九个变更 Rust 源文件的注释参考均已复核。 |

这些检查使用隔离数据库和一份 Codex 暂存副本执行原生扫描，不构成原生 GUI 或真实供应商
账单验收。原始 Agent 来源和正在使用的应用数据库均未修改。

## 依赖

直接依赖版本与 npm、crates.io 注册表逐项核对。npm 和 Cargo 更新同时刷新兼容范围内的
间接依赖，两个 npm 锁文件与 pnpm 工作区锁文件保持同步。新增版本包括 Starlight 0.43.1、
Astro 7.4.12、markdownlint-cli2 0.24.1、Playwright 1.65.1、svelte-check 4.7.8、
Vite 8.3.8、rusqlite 0.43.0、Jiff 1.0.0、rfd 0.18.0、notify-rust 4.19.0、
flate2 1.1.11、toml_edit 0.25.19、ureq 3.5.2 和 zstd 0.14.4，其他直接依赖已是当前版本。
TypeScript 保留 6.0.3：`@astrojs/check` 和 `svelte-check` 都只接受 TypeScript 5 或 6，
不接受注册表最新的 7.0.2。未关闭 peer 检查。rfd 将应用 MSRV 提高至 1.92，核心保留 1.90；
固定工具链与 CI 仍为 1.99.1。Jiff 调用改用官方重命名后的
`ZonedDateTime`/`to_zoned_datetime` API，日历/DST/调度仍需通过回归。

根 npm audit 无告警。桌面 npm 报告当前 Svelte Vite 插件通过 `deepmerge` 引入的两条
低风险告警，注册表元数据尚无兼容修复。保留[上游公告](https://github.com/advisories/GHSA-gcqx-w48m-w2h4)
作为依赖限制，不用 overrides 或 peer 例外隐藏告警。

## 离线模型

内置目录收录八个独立模型，包含 `qwen4-max-preview` 和文档确认的
`qwen4.0-max-preview` 别名。Preview 不填入价格，独立于 `qwen4-max`。
[官方 Token Plan 说明](https://docs.qwencloud.com/token-plan/personal/token-plan-personal-overview)
及[产品更新](https://docs.qwencloud.com/token-plan/token-plan-product-updates)不能将 Credits
转换为货币。Qoder 的 `qmodel_latest` 仍是未解析的路由标识。

新快照包含七个模型的九条价格：GPT-6.1 Terra 短/长上下文、Claude Fable 5.2、Gemini 4
Carbon 当前/十二月价格、Gemini 4 Hydrogen，以及原始 CNY 计价的 Qwen4-Max、
Qwen3.8-Max/Flash。依据为官方
[OpenAI](https://developers.openai.com/api/docs/pricing)、
[Terra 上下文价格](https://developers.openai.com/api/docs/models/gpt-6.1-terra)、
[Anthropic](https://platform.claude.com/docs/en/about-claude/pricing)、
[Google](https://aistudio.google.com/docs/pricing) 和
[Qwen](https://platform.qianwenai.com/docs/developer-guides/getting-started/model-prices)
价格页。不支持或未知的缓存 TTL 不计价。保留旧快照及历史估算，独立回归覆盖新/旧数据库导入、
当前参考、未来价格区间及无价 Preview 拒绝计价。

公开 models.dev 仅用于发现候选，不复制不兼容 TTL 或套餐价格。下载目录有 324 个供应商，
大小 6,655,210 字节，SHA-256 为
`d94a595fb045db95d8be9705958cb97b0c0a2c51dfa6dd6ac8a74e23a49349b8`。

## Qoder CN

已安装 VS Code 插件为 `Alibaba-Cloud.tongyi-lingma` 2.6.11，显示名为 Qoder CN。
原生数据库位于 `~/.lingma/vscode/sharedClientCache/cache/db/local.db`；旧发现规则只覆盖
CLI 项目。原生表含一个格式版本为 2 的 chat 会话、两条 `FREE_INPUT` chat 记录和四条消息，
每条请求对应一个已保存 assistant。请求元数据与受查 schema 均无 token 用量。
`extra.modelConfig.key` 为 `qmodel_latest`，不能识别实际服务模型或计费供应商。
插件请求构造确认该标识复制自所选模型配置。会话格式版本不能标识历史插件版本。

独立读取器通过只读 SQLite/WAL 连接仅读元数据，导入两条用量观察记录，token、金额及底层
模型调用次数均未知；不读取问题、回答、上下文、工具或账号。完整注册表重复读取不增加记录。
合成测试覆盖仅 WAL 更新、默认/手工发现、父目录/任意文件名、CLI/数据库同目录共存、坏行分页、旧游标重读、错误归属
空实例恢复、诊断保留及事务回滚。CLI 转录回归单独保留。这些 chat 样本不验证 Agent/edit
模式、含 token 的持久化、独立 IDE 或 JetBrains。

<a id="preview-and-approval-review-pricing"></a>

## Preview 与自动审批计价

修复在 0.3.11 上完成验证，准备以 0.3.12 发布。公开源码摘录、隔离测试数据库和命令日志均放在
`build/pricing-preview-review-20261116/`。当前参考在原型号无适用价目时，尝试去掉末尾
`-preview` 后的精确型号。已有价格不完整或有歧义时保留原结果。已配置手工价仍不标成官方价，
保留币种、区间及档位。Kimi K2.8 优先尝试去后缀价目，再使用既有授权的 K2.7 Code 例外。
Qoder 路由的显示覆盖仍与价格独立；缺少 token 数据时不能计算费用。

本机 Codex 0.169.0-alpha.11.1 的模型缓存有隐藏的 `codex-auto-review` 条目，说明为自动审批。
白名单目录元数据没有实际服务模型身份或单价。[官方供应商实现](https://github.com/openai/codex/blob/98e1584a082234ae716a3cc69d4c6f2d3631cf2d/codex-rs/model-provider/src/provider.rs)
提交 `98e1584a082234ae716a3cc69d4c6f2d3631cf2d` 为 API Key 自动审批选择 `gpt-5.6-luna`，
ChatGPT 则保留 `codex-auto-review`。因此，仅明确 OpenAI 的用量使用标明 Luna 的当前参考。
这不能证明该路由的实际或历史服务模型。[官方费率说明](https://help.openai.com/en/articles/20001415-chatgpt-rate-card-enterprise-token-based-pricing)
指出当前通过 ChatGPT 登录的自动审批安全检查免费；本地模型名不能识别认证方式或历史政策。

新的不可变 Luna 快照采用[官方 API 模型价格](https://developers.openai.com/api/docs/models/gpt-5.6-luna)：
每百万输入/缓存输入/输出 token 为 0.20/0.02/1.20 USD，采用已公布的 >272K 输入/输出倍率。
未核验的长上下文缓存和写入 TTL 价格保留未知。旧数据库启动只导入一次，不改写发生时费用。
缩小价格簿的缓存键加入供应商，避免不相关供应商的不透明路由结果妨碍 OpenAI 参考计价。

首次定向测试失败，是因为测试预期未计价模型没有结果行。存储层会保留未知币种行及未计价数量，
修正后的断言检查计价事件数为零、无替代型号，保留原报告行为。随后五组定向测试共 27 项通过。
后续检查还发现旧测试固定了快照数量：新增一个快照及两行后，共八个快照、66 行。
原快照相等及历史金额断言均保留。

| 检查 | 结果 |
| --- | --- |
| `npm run verify` | 退出码 0：1,189 项 Rust 测试通过，十项原生/平台用例显式忽略；20 项脚本、34 项前端测试及 Svelte/fmt/clippy、前端构建通过。 |
| 最终官方替代价格限制 | 全量检查后，将非官方已配置价目限制为末尾 Preview 回退；K2.7 Code 和 Luna 对应型号仍只取官方价。`npm run fmt:check`、`npm run clippy`、核心库及全部六组价格测试和 `model_reference` 通过：331 项测试，退出码 0。 |
| 新增回归范围 | 八项测试覆盖 Preview 边界/优先级、手工/官方价格限制、Codex 供应商隔离、上下文/缓存未知、旧数据库、回滚、选中小时和日/周/月保留。 |
| 文档检查 | 退出码 0：215 对仓库文档、21 对指南、41 项文档测试、构建 1,427 页并检查 3,018 个输出文件。 |

这些功能检查采用本地合成用量及公开文档/源码元数据，不能验证真实供应商账单或新的原生 GUI。
用户已授权 0.3.12 版本提升、提交、带 changelog 的注释 tag、推送及公开 Release。
发布准备日志放在 `build/release-0312/`；tag CI 在公开发布前单独核验准确的 0.3.12 源码及安装包。
npm/Rust 清单与锁文件已同步；pnpm 锁文件不含包版本字段，依赖图保持不变。
提交前，0.3.12 再次通过 `npm run verify`（1,189 项 Rust、20 项脚本、34 项前端测试；
十项原生/平台用例显式忽略）、发行版本一致性及文档检查/测试/构建。
这些检查不能替代 tag 对应的各平台安装包作业。

## 汇总参考价格提示与超长 Codex 压缩记录

跨型号参考价格参与计价时，指标卡、费用面板汇总和模型表格合计只显示“部分模型采用参考价格”，
不列出替代型号，也不拼接各模型的原因。模型行和单价详情仍保留原始型号、准确的参考型号及原因。
浏览器回归在汇总中提供两个替代型号，分别检查三类汇总区域，并单独检查模型明细。

当前应用数据库副本中，一个 Codex 文件停在第 1,492 行并显示需核对。原生版本
0.167.0-alpha.4.1 的结构可读，`has_record_errors` 为 false。这条完整的 `compacted`
记录为 17,260,078 字节，超过 8 MiB，其中 `latest_token_usage_record` 的六项用量字段有效。
读取器现在在既有 64 MiB 上限内流式读取，丢弃替换历史，只保留时间戳和最新用量记录。
其他用量、未知、损坏、不完整记录及位置数组仍保留原游标行为。未完成的停留游标无需提升解析器版本。

真实文件副本在隔离的新数据库中经九个有界窗口读完，最终为 `active_compat`，
游标和文件大小均为 169,775,000。共导入 256 次调用、53,711,098 总 token，
与独立逐条原生记录计算完全一致。原始 Agent 数据和正在使用的应用数据库均未修改。
该结果只验证真实文件和解析器的读取路径，未对已安装的 GUI 或旧数据库改写进行验收。

[Issue #9](https://github.com/owent/llm-usage/issues/9) 正确指出了首版实现仍把
`latest_token_usage_record` 构造为通用 JSON 值的问题。稠密的未知字段可能使内存远超输入大小，
生成精简记录时又会复制这些内容。携带记录现改用仅接受对象的窄结构，只保留三个 ID 和六项数值，
流式忽略其他记录级及 `usage` 字段，并直接序列化为文本。回归在两层放置稠密数组和字符串，
修复前失败；修复后保留有效用量，且精简行小于 1 KiB。位置数组形式的携带记录和 `usage` 仍被拒绝。

| 检查 | 结果 |
| --- | --- |
| `cargo test --manifest-path desktop/src-tauri/Cargo.toml --locked --test codex_oversized` | 退出码 0：七项测试，包括超长压缩记录后的边界用量及后续调用。 |
| Codex 读取器单元测试和 `codex_health_replay` | 退出码 0：三项读取器测试、五项健康/重放测试。 |
| `cargo clippy --manifest-path desktop/src-tauri/Cargo.toml --locked -p llm-usage-core --lib --test codex_oversized -- -D warnings` | 退出码 0。 |
| `npm run check` 和 `desktop/tests/i18n.test.mjs` | 退出码 0：Svelte 类型及十种语言的消息/占位符检查。 |
| `npm run test:browser` | 退出码 0：Edge 模拟 IPC 检查趋势区、指标卡和表格合计的通用提示及准确的模型明细，且窄屏布局无溢出。 |

一次性数据库/文件副本及原生探针曾位于 `build/codex-review-20261116/`，验证后已删除。

## 验证

| 命令 | 结果 |
| --- | --- |
| `npm ci` 和 `npm --prefix desktop ci` | 退出码 0；两份 npm 锁文件冻结安装通过。 |
| `pnpm install --frozen-lockfile --strict-peer-dependencies` | 使用任务独立 store，退出码 0；保留供应链规则和构建批准。 |
| Qoder 回归覆盖 | 退出码 0：四项 CN 回归及十六项 CLI 转录测试。 |
| 设置 `QODER_CN_NATIVE_DB` 后显式执行 `cn_read_only_local_acceptance` | 退出码 0：两条原生观察，重复读取不变，token/金额未知。 |
| 十一月/Kimi 价格定向测试 | 退出码 0：七项测试。 |
| `npm run verify` | 退出码 0：1,167 项 Rust 测试、十项显式忽略的原生/平台用例、20 项脚本和 32 项前端测试；Svelte/fmt/clippy 及前端构建通过。CN 原生用例已在上方单独运行。 |
| `npm run test:browser` | 退出码 0；Edge 模拟 IPC 交互检查。 |
| `npm run check:docs` / `test:docs` / `build:docs` / `test:docs:browser` | 退出码 0：213 对仓库文档、21 对指南、41 项单元测试、31 项浏览器检查；1,417 页及 2,997 个输出文件。 |
| Windows release/可执行程序检查 | 退出码 0：Tauri release 构建、十一项无界面检查、三项真实 CN 采集检查，以及二十次启动下的 27 项原生 WebView2/IPC 检查。CN 导入两条观察，token/金额未知，重复不增且无其他来源。GUI 测试使用隔离的合成 Codex/OpenCode/Qoder CLI 输入。 |

最终可执行文件为 `desktop/src-tauri/target/release/LLMUsage.exe`，SHA-256 为
`424aa933a355a6f87e7e1370061060d6cafa2f95b06061d87b625e76d9c3c9e5`。
本机二十次启动首屏 P95 为 1,622.98 ms；本次未重复规模/资源验收，也不代表其他平台 GUI 检查。

首次检查发现 Jiff API 重命名、CN 坏记录首窗口缺少原生结构推进标记，以及旧测试固定快照数
断言过期，已分别修正 API、推进标记及快照注册检查。新增参考文档还需要更新侧栏固定目录数断言。
pnpm 旧的间接依赖选择将 astro2tsx 0.1.3 与 emnapi 2 beta 配合使用，运行时报错；
兼容范围内递归更新后选择上游 astro2tsx 0.1.4，该版本显式声明 emnapi runtime 1.11.3，
无需 override 或绕过 peer 检查。沙箱中的 npm、SQLite SHM 和 Pagefind 输出访问失败
在授权文件访问条件下重新执行，不代表产品失败。本地结果不验证该源码版本的远端 CI、发布、
安装器生命周期或 Linux/macOS GUI 验收。

最初直接 Cargo release 的程序通过无界面采集，但缺少 Tauri 内嵌前端的构建上下文，
原生 GUI 测试未找到应用页面。修正后从 `desktop` 使用已安装 Tauri CLI：
`node node_modules/@tauri-apps/cli/tauri.js build --no-bundle`。npm 12 无法通过嵌套根脚本
转发额外的 `--no-bundle` 参数。该命令只构建，不安装。

随后 GUI 检查中的十月合成记录被默认七天明细保留规则正确拒绝，产生
`expired_by_retention`，没有导入。GUI 测试现将 OpenCode/Qoder 的合成副本整体移到近期历史
窗口，保留全部 token/身份/版本字段及相对时间，查询使用同一窗口。产品保留规则与原始测试
文件均未改变。

## Antigravity、依赖与模型清单

2026-11-23，Windows 上的 Antigravity 2.34.0 在两个本机会话数据库中记录了 Gemini 4 Neon
调用。旧读取器要求用量出现在 protobuf 顶层，并随生成记录写入时间戳，因此错过新版的嵌套
用量和规划器完成记录。旧读取器也跳过了生成序号零。[适配器规范](../../design/desktop-usage/adapters.md#antigravity-desktop-generation-format)
记录了调用识别字段及旧处理位置的恢复方法。

只读探针在内存中读取 SQLite 已提交的数据页和 WAL 帧。验证数据库保留模型 ID、用量字段和
统一偏移后的时间戳，并替换响应标识。Windows x64 上用 Rust 1.100.0 构建的调试可执行文件
对隔离应用数据库运行了两次 `--scan-once`。两次均退出 0，51 条调用的模型、完成时间、
未缓存输入、缓存读取、总输出和推理用量逐条吻合，重复扫描后仍为 51 条。独立无界面测试套件
通过 11 项检查。这些结果验证本机采集和持久化，原生 GUI 和其他 Antigravity 客户端版本需要独立验证。

从 npm 和 crates.io 核对直接依赖的当前版本。两份 npm 安装和 pnpm 冻结锁文件检查均退出 0。
npm 和 Cargo 同时解析兼容的传递依赖更新。当前 Astro 和 Svelte 检查器的 TypeScript
对等依赖上限为 6.x，因此保留 6.0.3。仓库和 GitHub 工作流使用 Rust 1.100.0。Linux
Secret Service 6.0.0 分别启用 Tokio 和 Rust 加密功能。应用凭据模块使用这些功能通过
`x86_64-unknown-linux-gnu` 目标的编译检查。Linux D-Bus 往返未验证。

离线清单新增 GLM-5.4、Gemini 4 Neon、GPT-6.2 Astra 和 Sol、MAI Code 2 和 Flash、
Hy4 Flash。[Coding Plan 清单](https://models.dev/providers/zai-coding-plan/)收录 GLM-5.4，
其按量价在 [Z.ai 官方价目表](https://docs.z.ai/guides/overview/pricing)中仍未知。
2026-11-23 快照新增 [Google](https://aistudio.google.com/docs/pricing#gemini-4-neon)、
[OpenAI](https://developers.openai.com/api/docs/pricing)和
[腾讯云](https://cloud.tencent.com/document/product/1823/130055)的官方标准 API 价。
Hy4 Flash 优惠于北京时间 11 月 30 日中午结束。按日生效的快照将切换所在的 UTC 日期留空，
12 月 1 日起采用标准价。

`npm run check:docs`、`npm run test:docs` 和 `npm run build:docs` 均退出 0。
标准浏览器测试脚本在当前沙箱内无法启动 Edge。用本机 Chrome 157 执行同一组 Playwright 断言通过。

`npm run verify` 的 Markdown、资源、脚本、前端测试、Svelte、格式和 Clippy 检查通过。
沙箱内 Rust 测试通过 1,201 项，按条件忽略十项。一项 Windows 接收器生命周期测试在
只读凭据可用性探测时被拒绝。该项在沙箱外单独重跑通过。Rust 文档测试和前端生产构建
另行执行，均退出 0。
