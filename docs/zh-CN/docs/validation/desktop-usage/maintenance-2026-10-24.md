# 依赖、价格与数据源维护

日期：2026-10-24。版本 0.3.4。本次审查覆盖 `c2940401..f2df0a6e` 中
2026-10-12 至 2026-10-23 的 10 个源码提交，共 165 个文件。
本记录所述改动均未提交；未执行提交、推送、部署或发布。未安装或启动 Agent，
未发起模型请求。真实本机 Agent 数据仅以只读方式提取允许的脱敏计数、版本、
token 合计和诊断类别。临时来源与验证数据库均位于仓库已忽略的 `build/` 下。

## 环境与依赖

本机环境为 Windows 11 x64、Node 24.21.0、Rust 1.99.1、pnpm 12.15.0、
Tauri CLI 2.12.4 和 WebView2 155.0.4283.62。两个 workspace manifest 的
Rust 最低版本均为 1.90；rusqlite 0.41.0 自身要求 Rust 1.89。

| 组件 | 选定版本与约束 |
| --- | --- |
| Astro / Starlight / Markdownlint | 7.4.2 / 0.42.7 / 0.24.0 |
| Tauri API 和 CLI / crate / build crate | 2.12.4 / 2.12.5 / 2.7.4 |
| Svelte / Vite / Playwright | 5.57.2 / 8.3.5 / 1.64.1 |
| rusqlite / bundled libsqlite3-sys | 0.41.0 / 0.39.0 |
| pnpm / 仓库 Rust 工具链 | 12.15.0 / 1.99.1 |
| TypeScript | 保留 6.0.3，满足编译器 API 与 peer 兼容要求 |

两个 npm manifest 与全部 npm/pnpm 锁文件均已同步。根目录和 desktop 的
`npm outdated` 仅显示 TypeScript 7.0.2。最新的 `@astrojs/check` 0.9.12 和
`svelte-check` 4.7.7 均接受 TypeScript `^5 || ^6`，没有绕过 peer 检查。
`cargo update --dry-run` 已无可兼容更新。五个较旧的间接包受上游约束保留：
brotli 要求 `~6.0`，crypto-common 要求 `generic-array =0.14.7`，
proc-macro-crate 2.0.2 固定 toml_datetime 0.6.3 和 toml_edit 0.20.2，
因此 toml 保持 0.8.2。WASM 相关锁文件也已更新为 sqlite-wasm-rs 0.6.2
和 rsqlite-vfs 0.2.0。

首次 npm 审计使用已配置的腾讯镜像，审计接口返回 404。npm 官方端点随后报告
根目录和 desktop 均无漏洞。首次 pnpm 审计发现九个来自旧间接解析的安全公告。
按既有版本范围更新后，undici 8.12.3、zod 4.6.7、fast-uri 3.1.9 和
postcss-selector-parser 7.1.7 被选中，未新增 override。最终 pnpm 审计同样为零。
`pnpm install --frozen-lockfile --strict-peer-dependencies` 通过。KaTeX 0.19.1
同时是根目录直接依赖和 override；测试覆盖普通渲染及拒绝继承的 `trust` 选项。

## 离线价格

`desktop/src-tauri/crates/core/prices/seed-2026-10-24.json` 新增 25 行官方标准
参考价格，五个快照现共 51 行。种子记录 OpenAI、Anthropic、Gemini、Kimi、
bigmodel 和 z.ai 的官方价格页 URL 及 2026-10-24 核验日期。27 项价格回归测试通过。

OpenAI 的 30 分钟／12 小时缓存写入 TTL 不符合现有 5 分钟／1 小时字段。
八条新 OpenAI 行的两种写入价格均为 NULL，该分量不计价；注释不作为可执行费率。
GLM-5.1 的输出价在文件中为 240000/280000，单位是每百万 token 0.0001 CNY，
即每百万 token ¥24/¥28，并非 ¥2.40/¥2.80。未虚构 Kimi K2.8 的精确价格，
保留已授权的 K2.7 Code 替代规则。估算仍需显式启用，保留原币种、渠道、日期和未知分量。

## 代码修复与回归

- **Codex 继承归属与冲突。** 完整旧摘要和按继承快照归一的原生完整摘要区分
  继承记录与原生响应，即使二者都没有源修订。矛盾或无法匹配的原生候选不得
  在重放时跳过，也不能清除真实冲突。候选不完整的历史冲突保留 `legacy_unknown`。
  Codex 解析器引用在重放时只能保持或升级，不能回退。测试覆盖多份继承快照、
  矛盾原生响应、候选顺序、回滚及使用当前解析器的归属修正。
- **OpenCode v2 分页与异常 ID。** 保留非空 ID 的原始类型，不由 SQLite 隐式转换为
  文本；NULL ID 产生诊断，但不阻塞其他有效行。保存的整轮完成状态跨有界页面保留
  非有限 ID 的排除状态，包括首页仅有这种 ID 的部分页。`max_lines=1` 的回归能继续
  读取后续两条有效记录，不再反复重启首页。缺少新状态的旧上下文保持原有首轮行为。
- **Qoder 探测。** 指纹探测在写锁外执行，保留 4,096 项 LRU。每个路径的探针代次
  仅在探针执行期间存在。其它文件的工作不会导致重复探针或覆盖结果；同文件较新的
  不稳定探针会阻止旧稳定结果写入缓存。取消和未完成行尾不会成为负缓存依据。
  10 项探测测试和 16 项转录集成测试覆盖并发、替换、部分写入、注册表路由和历史。
- **Kimi wire 状态记录。** 已核验的 Kimi Code 2.3.1 协议 1.5 清单仅在该产品读取器中
  忽略持久化/控制状态。`goal.update.tokensUsed` 及其它累计字段不产生调用。
  其它协议版本和 Kimi Work 1.4 保留未知类型诊断及各自版本依据。测试覆盖新增的
  21 种状态类型及其跨产品边界。
- **Copilot CLI 验证程序。** 空来源表让 `SUM(...)` 保持 `None`，不再抛出
  `InvalidColumnType`。结果明确为 `CHECK`，不视为真实格式验收通过，也不虚构零。
  `Option` 合计保留未知用量。
- **rusqlite 0.41 迁移。** 本地源处理器按新 `progress_handler` 签名使用可变连接。
  共享存储保留现有接口，使用作用域内 `InterruptHandle` 工作线程。取消期间持续中断，
  让执行线程的检查可见取消状态，并在连接复用前唤醒和等待线程退出。Drop 恢复外层
  控制状态，并保留暂停分类。测试强制在后续长语句开始前取消，并覆盖新的允许缓存项。
- **测试工具与兼容检查。** portable 测试处理 Windows `D:` 盘根目录；原生检查恢复
  所选语言，并在 Details 两页中查找记录。Rust 1.90 的 lint 修改保持原 Option 条件，
  备份测试使用新的可变处理器签名。下文的完整标准验证取代中间失败结果。

## 本机真实数据

注册表包含 41 个适配器，均有注册表驱动的结构与合成检查。有界发现找到 11 个具有
本机根目录的适配器；其中九个产生非空用量，Copilot CLI 和 Zed 的记录为空。
这不证明全部 41 个适配器的所有原生格式，或这九个来源的全部接口都已通过验收。

| 来源 | 只读提取与重复扫描 |
| --- | --- |
| Codex | 2,016 次调用；无诊断或冲突；重读新增 0 |
| Pi | 165 次调用；无诊断；重读新增 0 |
| oh-my-pi | 8,769 次调用；无诊断；重读新增 0 |
| Kilo | 13,606 次调用；保留一个累计对账差异和一个 latest-fallback 引用；健康为 `ok`；重读新增/更新 0 |
| ZCode | 6,782 次调用；127 个轮次对账中 102 个匹配、25 个差异；轮级总量仅用于对账；重读新增 0 |
| Kimi Code | 固定副本含 42 个文件、2,769 次调用；按文件保留协议依据；无诊断；重读新增 0 |
| Kimi Work | 69 个协议 1.4 文件、1,337 次调用；无诊断；重读新增 0 |
| CodeBuddy | 2 个事件，无诊断，重读数量稳定 |
| VS Code Copilot Chat | 1,604 次调用和 72 条观测；重读新增 0。72 条均有 `turn_input_incomplete`，整轮输入和总量完整性受限，健康仍为 `ok` |

其余 30 个注册适配器在本次检查中没有本机候选根目录。Copilot CLI 的
`assistant_usage_events` 和 `sessions` 表为空，Zed 的 threads 表为空；这些属于空会话，
不是成功的原生格式验收。当前没有可用 Qoder 客户端，Qoder 仍只有来源构造的合成检查。
其它适配器已有的合成和历史记录不等于本轮真实客户端验收。

九个真实来源实例的健康均为 `ok`。其余诊断仅限已知的 `latest_fallback`、
`reconcile_mismatch`、`turn_input_incomplete` 和 `unmapped_query_source` 类别。
未改写任何原始 Agent 文件或实时统计数据库；仅使用新建的隔离验证数据库。

## 验证命令与结果

| 命令或检查 | 已核验结果与范围 |
| --- | --- |
| `npm run verify` | 退出码 0：1,123 项 Rust 测试通过，9 项因平台或需显式授权的原生条件忽略；20 项脚本测试和 32 项前端测试通过；Markdown/资源、Svelte 诊断、rustfmt、Clippy 和前端构建通过 |
| `npm run build:desktop` | 使用 rusqlite 0.41.0 和最终取消状态处理，构建 Windows x64 release 可执行文件及 NSIS 安装包 |
| `npm run test:headless` | 使用隔离合成来源的 11 项真实可执行文件/SQLite 检查通过 |
| `npm run test:receiver` | 8 项原生 IPC/HTTP/凭据检查通过；`credentials_remaining=0` |
| `npm run test:update:check` | 未打包/公开版本及缺少身份的两类检查通过；实际状态 `up_to_date`，未选择资产或下载 |
| `npm run test:update:windows` | 原生合成 portable 替换与恢复检查通过；不据此声称存在更新的公开版本 |
| `npm run test:desktop -- --runs 1` | 27 项真实 WebView2/IPC 检查通过：五个主界面、九个设置区、21 事件/20 调用/1 观测、Details 20+1 行；重读和重启无新增；`errors=[]`、`outbound=[]`；首屏 P95 810.9346 ms |
| `npm run test:browser` | 完整模拟 IPC 浏览器检查通过：更新/设置/费用布局、分组/小时 token、遥测、十种语言、主题、过滤和分页 |
| `npm run test:import` | 100 万个合成事件/15,000,000 token 到达 GUI；可见耗时 648,808.4962 ms；650.0523515 秒内采样 1,036 次；进程树峰值 355.55078125 MiB，低于 512 MiB 上限；重读无新增；`errors=[]`、`outbound=[]` |
| `npm run test:install:windows -- --previous-installer build/install-lifecycle/windows/previous/LLMUsage_0.3.3_x64-setup.exe --previous-version 0.3.3` | 12 项真实 NSIS 生命周期检查通过：安装、升级、回滚、卸载/重装及拒绝清理；保留用量和设置；无测试安装、启动项、计划任务或进程残留 |
| `pnpm install --frozen-lockfile --strict-peer-dependencies` | 两个 workspace importer 与共享锁文件通过检查，包括限定构建批准和 peer 检查 |

上述 282 项核心库测试包含在 1,123 项中，不重复累加。注册表驱动的测试覆盖全部
41 个适配器。真实来源验收仅限上述本机非空数据，不证明缺失来源的格式。

## 失败记录与范围限制

首个单行 OpenCode 非有限 ID 用例会重复同一页；保存的整轮完成状态已修复。
新增的 Codex 用例起初会清除真实冲突并回退解析器；完整摘要比较和单调重放现在
同时保留冲突与当前版本。rusqlite 升级最初要求可变处理器，随后暴露 19 个由
MSRV 启用的 `Option` 简化及四个示例/测试问题。这些均已修正，未降低 lint 级别，
也未回退依赖。

首次原生暂停检查改变了 `last_finished_ms`。独立源码审查随后发现单次中断可能在
SQL 语句之间失效、缓存 true 导致误分类，以及嵌套作用域遮蔽。最终实现先发布取消
再中断，作用域内持续重试，恢复控制前停止并等待线程，且保留每个外层作用域的标志。
第三轮审查确认这些顺序；最终 27 项原生验收通过。一次 NSIS 尝试与原生 GUI 并发，
被进程预检拒绝；随后串行执行通过 12 项生命周期检查。

并非全部 41 个适配器都已完成真实数据验收。本轮有九个非空来源、两个空记录和
30 个无本机候选项。Qoder 没有原生客户端检查。未新增 Linux/macOS 桌面、安装或
凭据存储验收，也没有可更新的公开版本。这些属于未验证范围，不代表失败或支持。
离线价格是参考信息，不能证明实际计费渠道或账单。

单独的 Windows 通知测试从原生历史读到了测试通知；`Setting` 返回元素未找到
（0x80070490），不声称看到了横幅或发生点击。阻塞 OS 调用仍不能保证立即中断，
测试也无法穷举所有并发调度。原始 Kimi 副本和真实验证数据库已在脱敏检查后删除。
接收器凭据及自有 OS 任务已清理；改动未提交、推送或发布。

## Qoder 缓存锁修复，2026-10-28

[Issue #5](https://github.com/owent/llm-usage/issues/5) 在提交 `221c226ed` 上成立。
同一文件的较新探针未完成且没有缓存结果时，被取代的旧探针会持有进程级探测缓存锁
再次读取源文件。200 ms 截止时间在两次读取之间检查，阻塞文件 I/O 没有严格的
200 ms 上限，因此其他文件也可能等待该次读取。

重试现在先释放互斥锁，再读取文件，并在读取后检查是否有较新探针发布的缓存结果。
重试不会发布自身已被取代的结果。新增三项同步回归测试，覆盖重试期间探测其他文件、
同文件较新结果发布，以及读取失败后的恢复，同时检查缓存内容和活跃探针清理。
三项测试在原实现上均因仍持有缓存锁而失败；修复后全部 13 项 Qoder 探测测试通过。
测试不依靠耗时阈值判断锁是否被持有。

测试使用 `build/` 下独立的源码构造文件，不增加真实 Qoder 客户端或 GUI 验收。
本次修复日志位于 `build/issue-5/`。

Windows x64 / Rust 1.99.1 验证通过 1,138 项 Rust 测试，9 项按条件忽略；
20 项脚本、32 项前端和 41 项文档测试通过。Markdown/资源、Svelte、rustfmt、
Clippy、前端生产构建及双语文档检查均通过。首次 `npm run verify` 在未改动的
Copilot 临时目录重命名测试处遇到 Windows 沙箱错误 5；使用正常用户权限重跑完整的
`npm run test:rust`，以及随后执行 `npm run build:web`，均以 0 退出，未修改断言。
