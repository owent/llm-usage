# M8 第二批适配器实施验证记录（2026-09-29）

<a id="m8-second-batch-adapter-implementation-checks"></a>

2026-10-06 Aider 0.86.2、Goose 1.53.0、Continue CLI 1.5.47、jcode 0.91.0 与
gajae-code 0.18.7、AtomCode 5.2.1、Crush 0.97.1、Junie 26.9.22、Xum 0.30.0 与 Roo 3.54.0 已补官方容器/本地模型
非空真实样本、独立核对与回归（Crush 仅成本；Junie 任务失败但已有七次调用；
Xum 默认流缺用量，网关上游请求选项对照单列；Roo 缓存及取消尾调用覆盖缺口保留），
见 [M8 容器样本](m8-container-samples.md)。以下保留当时的当时基于文档和源码的实施记录。

范围：M8 扩展 Agent 覆盖（[调研 A25–A48](../../design/desktop-usage/research.md#agents)、
[扩展矩阵](../../design/desktop-usage/adapters.md#扩展覆盖)）的文档级实施。
18 个新适配器目录（17 个解析 + Qoder 探针）+ 注册 + 设计要求检查；不提交、不推送、不部署。

<a id="local-environment-and-inventory"></a>

## 本机环境与盘点

- Windows 11 x64；Node 22+；cwd 仓库根；全量命令 `npm run verify`。
- 本机盘点（2026-09-29，只读探测 home/APPDATA/LOCALAPPDATA）：M8 产品均未安装；
  仅 `%LOCALAPPDATA%/Zed/threads/threads.db` 存在（threads 表 schema 与官方
  迁移 SQL 一致、**0 行**——schema 核验结果，无真实用量样本）。
- 结论：全部按 M3 未安装产品先例交付**基于文档或源码的实现**（固定源码/官方分发物/
  固定版本的第三方解析器 + 合成测试数据），discover 在本机返回空（Zed 发现空库），
  真实样本出现后升级验证。

<a id="深度调研先于实施逐产品证据核验"></a>

<a id="深度调研先于实施逐产品资料核验"></a>

<a id="product-reference-checks-before-implementation"></a>

## 实施前的产品资料核验

并行调研只读核验以下固定版本（详细字段依据入
[research.md](../../design/desktop-usage/research.md#agents) 与各适配器文件头）：

| 产品 | 核验来源 | 等级 |
| --- | --- | --- |
| Roo Code | RooCodeInc/Roo-Code b867ec9（已归档 2026-05） | official-source |
| Goose | aaif-goose/goose a701bb1 | official-source |
| Crush | charmbracelet/crush 1f3827b | official-source |
| jcode | 1jehuang/jcode 4f6bf8e（master） | official-source |
| gajae-code | Yeachan-Heo/gajae-code 7e54f9c | official-source |
| Command Code | npm command-code@1.69.0 dist/cli.mjs 逐行 | official-distribution |
| Continue | continuedev/continue 5522c6f | official-source |
| AtomCode | atomgit e4215f7（GitHub 镜像同 SHA） | official-source |
| Zed | zed-industries/zed bd74733 + 本机 schema | official-source + local schema |
| Aider | Aider-AI/aider 5dc9490 | official-source |
| Amp/Grok/Junie/Kiro/Droid/Xum/Antigravity | tokscale 固定提交 1d9a939 | third-party-parser / 逆向 |
| Amazon Q | aws/amazon-q-developer-cli 15cc8f3 | official-source（**没有本地 token 记录**） |
| Codebuff | CodebuffAI/codebuff caec5fc | official-source（**本地仅 credits，无 token 字段**） |
| iFlow CLI | iflow-ai/iflow-cli 4642808（**2026-04-17 停服**） | official-docs（没有默认用量记录） |
| Qoder | docs.qoder.com + npm 1.1.64 解包 | path-verified / fields-unverified |

关键调研结论（禁止猜测下的是否实施的判断）：

- **Goose 有逐请求表**：官方迁移 15 的 `usage_ledger`（Unix 秒 + 五桶 + cost_source），
  tokscale 未覆盖（其只见会话级）；旧库按 `sessions.accumulated_*` 聚合读取。
- **Crush token 列不是用量**：官方源码证实 `prompt_tokens/completion_tokens` 是
  最近 step 上下文规模快照（SET 覆盖、摘要后重置）；仅根会话 `cost` 累计
  （子会话回卷 ⇒ `parent_session_id IS NULL` 过滤）⇒ cost-only 实施。
- **Roo tokensIn 含缓存**（官方三处核验依据），与既有 cline 适配器（dcf8c3c 四桶
  互斥）存在字段定义差异：两适配器各按各自固定版本资料实现，分歧在 adapters.md 登记，
  待双方真实样本复核。
- **jcode 缓存字段定义按 provider 原样**：openai input 含 cache（uncached 派生）、
  anthropic 三列互斥、未知并列不派生；快照+journal 合并，崩溃窗口按消息 id
  upsert 保存。
- **gajae usage 已官方归一化互斥桶**（适用 map_pi_family 规则）；五桶齐全才统计
  （官方 stats parser 使用相同规则）；message.timestamp 毫秒与条目 ISO 并存。
- **Command Code inputTokens 含 cache**（官方成本公式证实）；有效路径=末条目回溯
  parentId 链；fork 复制保留 id+timestamp ⇒ 跨文件去重键。
- **Continue usage 仅 CLI 写入**（会话累计真实 API 值）；devdata.sqlite 是
  tokenizer 估算不采纳；GUI 会话无字段。
- **Zed 当时以累计用量统计，并核对 request 桶的覆盖规则**（官方 thread.rs:2893 清零）；
  当时把 TokenUsage 序列化缺省的 0 视为已报告 0；仅 zed.dev provider；imported 跳过。
- **Amazon Q / Codebuff 本地没有逐次 token 记录**（官方源码证实：Q 的 API
  tokenUsage 在类型转换层被丢弃；Codebuff 本地仅 credits）⇒ 不实施 token
  适配器，按边界排除（与 Warp/Cursor 同类）。
- **iFlow 已停服**且唯一用量输出是需启用的 OTel（归 M5 OTLP 要求），无适配器。
- **Qoder 字段仍缺少可核验的资料**（bundle schema 线索不作数）⇒ 探针级 fail-closed 接入。

<a id="eighteen-independent-adapters-and-v30-registries"></a>

## 实施（18 个适配器，全部独立目录 + 版本注册表 + V30）

`desktop/src-tauri/crates/core/src/adapters/`：
zed、aider、junie、xum、droid、amp、grok、roo、goose、crush、jcode、
gajae-code、commandcode、continue（模块 continuedev）、atomcode、kiro、
antigravity、qoder（探针）。全部注册进 `built_in_adapters()`。

要点（与矩阵边界一一对应）：

- 估算路径全部不采纳：Kiro Auto 零计数/字节折算/窗口差额、Goose reasoning 差额、
  Droid 转录分摊、Grok 累计差额/补偿、Continue devdata、Crush token 快照。
- 两种记录格式：Amp ledger（有 timestamp）为主、消息 usage 仅对账不推造时间；
  Kiro CLI turns 与 kiro-cli SQLite 分列（交叉重叠尚未核实，如实标注）。
- 会话级聚合（不虚构逐次）：Zed（累计用量统计及 request 桶对账）、Xum、
  Droid、Continue、AtomCode（round_count 合计作调用数）、Goose 旧库聚合读取。
- 新依赖：`zstd ^0.13`（Zed threads.db zstd blob 有界解压，64 MiB 上限）。

<a id="tests-and-commands"></a>

## 测试与命令

- `cargo test -p llm-usage-core`（cwd desktop/src-tauri）：**566 项全部通过**
  （lib 158 项含各适配器单元测试 + 集成测试全绿；含新增
  `tests/m8_contract.rs` 19 项：17 个解析适配器设计要求检查 + Qoder 探针 fail-closed + V30 目录/注册表结构检查）。
- `tests/adapter_layout_v30.rs` 的 AGENTS 清单扩展覆盖全部 M8 目录。
- `npm run verify`（cwd 仓库根）**全部检查通过**：markdownlint 150+ 文件 0 问题、
  assets/脚本/前端逻辑/svelte-check（0 错误 0 警告）、cargo fmt + clippy
  （-D warnings）通过、Rust 全部测试通过、vite 8.3.1 构建成功。
- `git diff --check` 通过；`git status --short` 无临时产物（build/ 已 ignore）。

<a id="证据等级与剩余缺口"></a>

<a id="verification-scope-and-remaining-gaps"></a>

## 核验范围与剩余缺口

| 核验范围 | 适配器 |
| --- | --- |
| official-source（含分发物）+ 本机 schema | zed |
| official-source / official-distribution | roo、goose、crush、jcode、gajae、commandcode、continue、atomcode、aider |
| third-party-parser（tokscale 1d9a939） | amp、grok、junie、kiro、droid、xum |
| third-party-reverse-engineered（风险最高档） | antigravity（protobuf 布局逆向；1.1.18+ 无时间戳行 fail closed） |
| path-verified / fields-unverified | qoder（探针 fail closed） |

未完成/后置项：

1. **真实数据验收全部后置**：本机 18 个产品目录均无真实用量数据（Zed 空库）；
   真实脱敏测试数据 出现后逐产品升级为已验证并核对边界（对账、覆盖、时区）。
2. Amazon Q、Codebuff、iFlow 不实施 token 适配器（没有本地记录或已停服），
   状态见矩阵；Cursor/Warp/TRAE/Windsurf 维持排除/F1。
3. Kiro 两种记录格式交叉对账、Roo vs Cline tokensIn 字段定义差异、Antigravity 1.1.18+
   时间戳布局：均待真实样本。
4. 初轮只修改核心库与注册，未重跑浏览器回归与 Windows release 构建；
   后续工作区复核已补跑浏览器回归，记录如下。release 构建仍随 M7 验收。

<a id="worktree-review-after-f6ede266590b37c14f424070b048ffd773f3ae9c-2026-09-29"></a>

## f6ede266590b37c14f424070b048ffd773f3ae9c 工作区复核（2026-09-29）

范围：审查该提交后的 74 个已暂存改动，并继续修复审查发现的问题；
工作区改动未提交、未推送。环境：Windows 11 x64，Node.js v24.21.0，
Cargo 1.98.0，仓库根执行 npm/Git，`desktop/src-tauri` 执行定向 Cargo 测试。

已确认并修复的边界：

- Zed 累计桶合计改为 checked 算术后，调用处仍按整数使用，导致核心库
  无法编译；现在溢出时记录诊断并跳过该线程。
- Goose 旧库会话累计会增长，完整页后的游标必须从头复查；
  Crush 累计成本使用 `updated_at` 作为修订号，否则同 ID 的后续成本与
  旧 Final 事件冲突。Zed、Kiro、Crush 超过 50,000 行时按页继续，
  读到末页后复查可变行。
- jcode journal 达到单轮行数上限时按持久偏移续读，末页后重读以应用
  会话级元数据；原实现每轮从头读，尾部永久不可达。同 ID 消息的
  `source_revision` 取快照/日志 `updated_at`，不把消息完成时间误当修订时间；
  同毫秒且内容冲突仍交给导入层标记冲突。
- Xum 版本只从完整 JSON 的顶层字段判定；截断头部按兼容回退。
  Kiro 部分写入的 SQLite 魔数保持 Pending。共用文件头读取处理短读。
- 停用实例的统一扫描的停用检查、逐源计划到期过滤、实际扫描成败记录、
  OTel 错误状态透传、SourceList 周名时区及来源列表字段索引按工作区差异核验。

回归：`tests/m8_review_fixes.rs` 覆盖跨页读取、累计更新、journal 续读与
同消息时间戳的更正、
停用来源、版本依据、坏桶诊断及单行故障；补充 Zed 溢出单测。
`npm run verify`（仓库根，退出码 0）通过文档 lint、资源检查、脚本/前端
测试、Svelte 检查、Rust 格式/Clippy/全量测试及 Web 构建；
`npm run test:browser`（仓库根，退出码 0）通过浏览器交互回归。
默认沙箱启动 Node 子进程报 `spawnSync EPERM`，按受控提权路径重跑
同一命令成功；该错误不作为产品失败的依据。

验收等级仍是文档级 + 合成数据：本轮没有新增 M8 产品的本机真实用量样本。
本机默认 Copilot 数据库路径本轮探测为不存在，不能把此前 36 行真实数据
核对结果当作本轮复测。前述真实样本、Kiro 两种记录格式交叉对账、Roo/Cline
字段定义差异和 Antigravity 1.1.18+ 时间戳布局仍按原缺口保留。
本轮还探测了 M2 默认目录：`%USERPROFILE%/.claude`、`.gemini`、`.qwen`
均不存在；这只说明当前 shell 下默认路径无样本，不能替代环境变量或手工根检查，
M2 真实样本 缺口仍保留。
