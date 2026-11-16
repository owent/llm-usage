# 当前维护验证

核对日期：2026-11-16。维护验收源码版本为 0.3.9，发布准备版本为 0.3.10。
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
