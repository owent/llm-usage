# Windows Agent 数据提取

<a id="windows-agent-extraction"></a>

环境为 Windows 11 x64、Node 24.22.1、Rust/Cargo 1.99.1，应用源码版本 0.3.7。
用户已授权安装，并向指定智谱、Kimi Coding 入口发送最小测试请求。
本轮未提交、发布或清空现有统计数据库。没有可执行的 macOS 环境，
也未取得用户原始 macOS 诊断文件。

## 安装与请求

官方 npm 的 latest 分别为 Claude Code 2.1.312、DSH 0.2.1-rc.2，
均安装到已忽略的 build/agent-native-20261029/tools，锁文件保留 npm integrity 引用。
两个原生版本命令均通过。检查安装脚本后，只执行了 Claude 原生二进制放置脚本与
DSH spawn-helper 权限脚本，没有批量批准其他被拦截的生命周期脚本。
注册表完整性校验不代表独立验签。本轮没有选择 DSH 的 next 或 alpha 标签。

本地启动脚本内部用 yq 读取凭据，只经进程环境传递。
密钥未写入普通配置、提示词、命令参数或日志。Claude 使用供应商文档指定的 Anthropic 入口，
DSH 使用给定的 OpenAI 兼容 URL。测试工作区、配置和数据目录均隔离。
Claude 三个简单请求禁用工具，另一个允许 Write 的样本产生两次调用的工具循环。
DSH 禁用标题生成和遥测，因此这些样本不验证默认辅助调用覆盖范围。

| 原生样本 | 调用数 | 非缓存输入 | 缓存读取 | 输出 |
| --- | ---: | ---: | ---: | ---: |
| Claude / glm-5.3-flash | 1 | 1,710 | 未知 | 55 |
| Claude / glm-5.3 | 1 | 1,707 | 未知 | 14 |
| Claude / kimi-for-coding | 1 | 1,257 | 512 | 36 |
| Claude / glm-5.3-flash，工具循环 | 2 | 2,845 | 2,432 | 244 |
| DSH / glm-5.3-flash | 1 | 11,190 | 未知 | 69 |
| DSH / glm-5.3 | 1 | 11,187 | 未知 | 34 |
| DSH / kimi-for-coding | 1 | 10,760 | 未知 | 60 |

七次 CLI 启动均退出 0。Claude CLI 用量与去重后的原生 assistant 用量一致，
十条内容块记录对应五次调用。默认零缓存写入与 Claude 完整总量仍未知。
原生 transcript 不证明计费渠道或实付。DSH 三条 settlement 共记录 33,137 个非缓存输入、
163 个输出 token，重建总量为 33,300，不将其当作独立源总量。
本轮没有增加独立 HTTP 响应捕获。

## 解析修正与检查

旧 Claude 解析器在未变化的真实文件上复现：遇到 atis-latch 后记录
undocumented_record_type，导入零次调用。原生 2.1.312 写者和样本将请求 shape、blob、request、
atis、tools-used、cost-state 元数据与 assistant 用量分开。只有这些元数据形状有效时才跳过，
不增加用量。file-history-snapshot 和 progress 有发行包写者依据与合成形状测试，
本轮真实工具样本没有产生这两种类型。

未知类型、形状无效或携带 usage 的元数据仍整文件拒绝。
探测允许形状有效的元数据作为首记录。独立的元数据规则标记使已消费且未变化的文件重新检查一次，
并覆盖有界续读、不完整末行、事务回滚、诊断历史、原观察时间和真实冲突保留。
逐记录版本与默认零规则没有变化；本轮新记录仍保留内部兼容性引用。原生 2.1.312
事件使用 known_version，不产生 latest_fallback 警告；元数据诊断指出实际出错字段。

DSH 仍写入 v4。已安装 pi-ai 1.4.0 的源码独立确认非缓存输入、包含推理的输出、
省略或默认零缓存和计算总量。发行包事件表新增 job/registered、job/settled、
working-directory/change，对应源码定义只包含执行或工作目录元数据。
合成测试接受完整声明形状及可选字段，不虚构调用；形状无效或携带用量的元数据、
未知必需事件均拒绝且不推进检查点。真实任务及工作目录切换执行不属于三个原生简单请求样本的验收范围。

重建后的 Windows release 可执行程序通过应用注册表读取七个原始本地文件，
得到八次调用、七个 active 文件，没有 degraded 文件。完整重复扫描保留事件与 data_revision。
对旧版 Claude 失败导入数据库制作独立 SQLite 备份后，成功恢复一次调用，将文件状态改为 active，
并保留原 undocumented_record_type 诊断。整个过程源文件摘要未变。
无界面可执行程序和 SQLite 结果不代表应用 GUI 或 macOS 验收。

| 检查 | 命令或入口 | 结果 |
| --- | --- | --- |
| 产品检查 | `npm run verify`，最终 Cargo Clippy/测试 | 退出 0；1,160 项 Rust 测试通过，9 项平台忽略；lint、脚本/UI 测试、类型、Clippy 和前端构建通过 |
| Windows release 程序 | `node node_modules/@tauri-apps/cli/tauri.js build --no-bundle`，工作目录 `desktop` | 退出 0；最终解析源码已重建，没有生成安装包或发布 |
| 原生提取、旧库 | 本轮 `native-readback.mjs` | 退出 0；八次调用、七个 active 文件，重复结果相同，旧失败恢复，源摘要未变 |
| Zed 原生提取、旧库 | 本轮 `acceptance.mjs` | 退出 0；一个区间/164 输出/两个 turn 桶；新导入、重复及旧 skip 恢复，源数据库/WAL/逻辑行未变 |
| 无界面回归 | `npm run test:headless` | 退出 0；11 项检查，使用独立合成来源与数据库 |
| 文档 | `npm run check:docs`、`npm run test:docs`、`npm run build:docs` | 退出 0；双语内容、类型、41 项测试及 2,961 个本地发布输出文件通过检查 |
| 脱敏、Git | 本轮公开、私有凭据检查；`git diff --check` | 退出 0；公开变更文件和本地测试文件均没有实际密钥匹配 |

本轮日志保留首次失败。新增回归最初按引用传递 ScanLimits，修正后的测试通过。
脱敏最初漏掉 DSH message.id，正确触发归属拒绝，现已保留其哈希身份。
原未知类型测试改用真正未知的名称，保留负例；file-history-snapshot 已成为已知类型。
文档目录测试的预期数量已随新增验证页更新。沙箱中分别遇到 Windows CredReadW 拒绝访问、
Pagefind 目录访问被拒绝和无界面测试临时目录 EPERM，对应正常用户环境检查均通过。
发行版构建最初遇到 npm 参数转发及工作目录错误，改为在 desktop 目录直接运行 Tauri CLI 后通过。

## Zed 原生线程

用户在本机 Zed 1.26.2、提交 c4273271092b0287f7d181f616bd5171e0a2f70c 中，
使用自定义 glm-5.3 与内置 DeepSeek V4.1 Flash 产生两个 turn。只读访问默认 threads.db
发现一个 zstd DbThread 0.3.0：累计 input_tokens=47,748、output_tokens=164，
两个请求桶，没有缓存字段。最新模型为 deepseek/deepseek-flash，消息没有逐调用模型或用量。
脱敏样本保留计数、关系及 schema，移除正文、工具和原始 ID。本机版本及用户新产生的样本
确定本次来源；序列化 schema 不能识别所有历史记录的客户端版本。

固定源码确认两种映射不同：OpenAI chat 从 prompt_tokens 减去缓存；内置 DeepSeek
保留含缓存的 prompt_tokens，并写入零缓存桶。线程计数跨 Provider、模型切换累计，
DbThread.model 可变。因此不能把原生输入认定为非缓存输入或含缓存总输入。
保留正输出、缓存桶；默认零及完整总量仍未知。只比较请求桶与累计输出，不相加含义不明的
输入、缓存桶，也不把用量归给最新模型。两个桶只计已观察到的 turn，不能确定底层请求数。

旧 release 程序复现 Provider 过滤问题：文件为 active，存在 non_zed_dev_thread_skipped，
却没有聚合。修复按完整原生 0.3.0 结构读取，不依赖自定义 Provider 名称。
其中包含 llm-usage-zhipu；完整原生结构优先于旧的仅统计 Provider 回退。
已有输入、质量差异按普通聚合冲突保留原汇总，不作为解析依据修正放行。
历史 hosted 规则独立保留。其他 Provider 只有结构兼容
读取，没有真实调用认证。独立规则标记重放已消费的旧游标，在事务失败时保留检查点，
且仅在完整有效扫描后标记完成。坏行保留诊断，不掩盖其他有效线程。

Windows 程序从原始来源读出一个原生 session 区间：164 个已报告输出 token、两个已观察
turn 桶，输入、缓存及总量未知。独立的旧失败导入数据库恢复相同聚合，并保留旧 skip 诊断；
重复扫描保留 data_revision。原生区间独立于逐日、逐模型调用统计，本样本不增加逐调用
事件或每日总量。回归覆盖真实发现与路由、已消费旧游标、检查点及后期聚合写入回滚、
实际分页上限续读、坏行、正缓存、未知零、歧义总量与输出求和溢出、输出对账及已有聚合冲突，
也覆盖切回旧自定义或 hosted Provider 名称；原 1.22.0 仅统计回归通过。

首次测试失败暴露出手工根未路由的测试辅助函数、错误的每日汇总预期，以及 parser 版本变化
导致的提前删除检查点。现改为应用路由及原生区间查询，以独立规则标记保留旧检查点。
此前隔离 GUI 尝试因系统目录定位及 API Key 转发失败，随后用户提供了这些成功的原生调用。
本轮使用只读可执行程序、SQLite 验证；GUI 渲染及用户报告的 macOS 问题仍未验证。
一次翻译清单写入返回 Windows UNKNOWN，未改变操作的重试随后成功。

来源：[Claude npm](https://www.npmjs.com/package/@anthropic-ai/claude-code)、
[DSH npm](https://www.npmjs.com/package/@deepseek-ai/dsh)、
[智谱 Claude 配置](https://docs.bigmodel.cn/cn/coding-plan/tool/claude)、
[Kimi Claude 配置](https://www.kimi.com/code/docs/en/third-party-tools/claude-code.html)、
[固定 Zed 数据库写者](https://github.com/zed-industries/zed/blob/c4273271092b0287f7d181f616bd5171e0a2f70c/crates/agent/src/db.rs)、
[固定 Zed 用量映射](https://github.com/zed-industries/zed/blob/c4273271092b0287f7d181f616bd5171e0a2f70c/crates/language_model_core/src/chat_completion.rs)、
[固定 DeepSeek 映射](https://github.com/zed-industries/zed/blob/c4273271092b0287f7d181f616bd5171e0a2f70c/crates/language_models/src/provider/deepseek.rs)、
[固定线程累计写者](https://github.com/zed-industries/zed/blob/c4273271092b0287f7d181f616bd5171e0a2f70c/crates/agent/src/thread.rs)。
