# M8 第二批适配器实施验证记录（2026-09-29）

范围：M8 扩展 Agent 覆盖（[调研 A25–A48](../../design/desktop-usage/research.md#agents)、
[扩展矩阵](../../design/desktop-usage/adapters.md#扩展覆盖)）的文档级实施。
18 个新适配器目录（17 个解析 + Qoder 探针）+ 注册 + 合同测试；不提交、不推送、不部署。

## 本机环境与盘点

- Windows 11 x64；Node 22+；cwd 仓库根；全量命令 `npm run verify`。
- 本机盘点（2026-09-29，只读探测 home/APPDATA/LOCALAPPDATA）：M8 产品均未安装；
  仅 `%LOCALAPPDATA%/Zed/threads/threads.db` 存在（threads 表 schema 与官方
  迁移 SQL 一致、**0 行**——schema 级证据，无真实用量样本）。
- 结论：全部按 M3 未安装产品先例交付**文档级证据实现**（固定源码/官方分发物/
  第三方解析器锚点 + 合成 fixture），discover 在本机返回空（Zed 发现空库），
  真实样本出现后升级验证。

## 深度调研（先于实施，逐产品证据核验）

并行调研只读核验以下锚点（详细字段证据入
[research.md](../../design/desktop-usage/research.md#agents) 与各适配器文件头）：

| 产品 | 证据锚点 | 等级 |
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
| Amazon Q | aws/amazon-q-developer-cli 15cc8f3 | official-source（**无本地 token 载体**） |
| Codebuff | CodebuffAI/codebuff caec5fc | official-source（**本地仅 credits，无 token 字段**） |
| iFlow CLI | iflow-ai/iflow-cli 4642808（**2026-04-17 停服**） | official-docs（无默认用量载体） |
| Qoder | docs.qoder.com + npm 1.1.64 解包 | path-verified / fields-unverified |

关键调研结论（禁止猜测下的准入判定）：

- **Goose 有逐请求表**：官方迁移 15 的 `usage_ledger`（Unix 秒 + 五桶 + cost_source），
  tokscale 未覆盖（其只见会话级）；旧库按 `sessions.accumulated_*` 聚合兜底。
- **Crush token 列不是用量**：官方源码证实 `prompt_tokens/completion_tokens` 是
  最近 step 上下文规模快照（SET 覆盖、摘要后重置）；仅根会话 `cost` 累计
  （子会话回卷 ⇒ `parent_session_id IS NULL` 过滤）⇒ cost-only 实施。
- **Roo tokensIn 含缓存**（官方三重证据），与既有 cline 适配器（dcf8c3c 四桶
  互斥）存在血统分歧：两适配器各按自己的锚点实现，分歧在 adapters.md 登记，
  待双方真实样本复核。
- **jcode 缓存口径按 provider 原样**：openai input 含 cache（uncached 派生）、
  anthropic 三列互斥、未知并列不派生；快照+journal 合并，崩溃窗口按消息 id
  upsert 兜底。
- **gajae usage 已官方归一化互斥桶**（map_pi_family 口径适用）；五桶齐全才统计
  （官方 stats parser 同口径）；message.timestamp 毫秒与条目 ISO 并存。
- **Command Code inputTokens 含 cache**（官方成本公式证实）；有效路径=末条目回溯
  parentId 链；fork 复制保留 id+timestamp ⇒ 跨文件去重键。
- **Continue usage 仅 CLI 写入**（会话累计真实 API 值）；devdata.sqlite 是
  tokenizer 估算不采纳；GUI 会话无字段。
- **Zed cumulative 权威、request 桶覆盖语义**（官方 thread.rs:2893 清零）；
  TokenUsage 0 值序列化缺省=已报告 0；仅 zed.dev provider；imported 跳过。
- **Amazon Q / Codebuff 本地无逐次 token 载体**（官方源码证实：Q 的 API
  tokenUsage 在类型转换层被丢弃；Codebuff 本地仅 credits）⇒ 不实施 token
  适配器，按边界排除（与 Warp/Cursor 同类）。
- **iFlow 已停服**且唯一用量载体是需启用的 OTel（归 M5 OTLP 合同），无适配器。
- **Qoder 字段仍缺证**（bundle schema 线索不作数）⇒ 探针级 fail-closed 接入。

## 实施（18 个适配器，全部独立目录 + 版本注册表 + V30）

`desktop/src-tauri/crates/core/src/adapters/`：
zed、aider、junie、xum、droid、amp、grok、roo、goose、crush、jcode、
gajae-code、commandcode、continue（模块 continuedev）、atomcode、kiro、
antigravity、qoder（探针）。全部注册进 `built_in_adapters()`。

要点（与矩阵边界一一对应）：

- 估算路径全部不采纳：Kiro Auto 零计数/字节折算/窗口差额、Goose reasoning 差额、
  Droid 转录分摊、Grok 累计差额/补偿、Continue devdata、Crush token 快照。
- 双载体：Amp ledger（有 timestamp）为主、消息 usage 仅对账不推造时间；
  Kiro CLI turns 与 kiro-cli SQLite 分列（交叉重叠无证据，如实标注）。
- 会话级聚合（不虚构逐次）：Zed（cumulative 权威 + request 桶对账）、Xum、
  Droid、Continue、AtomCode（round_count 合计作调用数）、Goose 旧库兜底。
- 新依赖：`zstd ^0.13`（Zed threads.db zstd blob 有界解压，64 MiB 上限）。

## 测试与命令

- `cargo test -p llm-usage-core`（cwd desktop/src-tauri）：**566 项全部通过**
  （lib 158 项含各适配器单元测试 + 集成测试全绿；含新增
  `tests/m8_contract.rs` 19 项：17 个解析适配器合同 + Qoder 探针 fail-closed + V30 目录/注册表结构检查）。
- `tests/adapter_layout_v30.rs` 的 AGENTS 清单扩展覆盖全部 M8 目录。
- `npm run verify`（cwd 仓库根）**全链通过**：markdownlint 150+ 文件 0 问题、
  assets/脚本/前端逻辑/svelte-check（0 错误 0 警告）、cargo fmt + clippy
  （-D warnings）通过、Rust 全部测试通过、vite 8.3.1 构建成功。
- `git diff --check` 通过；`git status --short` 无临时产物（build/ 已 ignore）。

## 证据等级与剩余缺口

| 证据等级 | 适配器 |
| --- | --- |
| official-source（含分发物）+ 本机 schema | zed |
| official-source / official-distribution | roo、goose、crush、jcode、gajae、commandcode、continue、atomcode、aider |
| third-party-parser（tokscale 1d9a939） | amp、grok、junie、kiro、droid、xum |
| third-party-reverse-engineered（风险最高档） | antigravity（protobuf 布局逆向；1.1.18+ 无时间戳行 fail closed） |
| path-verified / fields-unverified | qoder（探针 fail closed） |

未完成/后置项：

1. **真实数据验收全部后置**：本机 18 个产品目录均无真实用量数据（Zed 空库）；
   真实脱敏 fixture 出现后逐产品升级为已验证并核对边界（对账、覆盖、时区）。
2. Amazon Q、Codebuff、iFlow 不实施 token 适配器（本地无载体/停服），
   状态见矩阵；Cursor/Warp/TRAE/Windsurf 维持排除/F1。
3. Kiro 双载体交叉对账、Roo vs Cline tokensIn 口径分歧、Antigravity 1.1.18+
   时间戳布局：均待真实样本。
4. 浏览器回归与 Windows release 构建未随本轮重跑（本轮改动仅核心库与注册，
   前端界面无变化；随下轮界面相关改动一并执行）。
