# WSL Agent 产品安装验证与清理（2026-09-30）

范围：在 WSL Debian（用户 owent，Node 20.19.2/npm 10.9.9/Python 3.13.5，
网络可用，无免密 sudo——全部用户级安装）安装 M2/M3/M8 缺真实样本产品中
可无头安装的 CLI，做安装级/真实数据验证后全部卸载并清理。
目的：为"真实 fixture 待本机数据"缺口取得真实证据；不产生模型调用
（无凭据，不登录）。Windows 侧对 WSL 数据经
`\\wsl.localhost\Debian\home\owent`（UNC）只读访问。

## 安装与首跑（全部官方渠道，npm 包名经 registry 身份核验）

| 产品 | 渠道/版本证据 | 首跑产物（真实落盘） |
| --- | --- | --- |
| Claude Code | npm `@anthropic-ai/claude-code`（官方 repo 核对）；`claude -p hi` 未登录退出 | **真实转录** `~/.claude/projects/<cwd-slug>/*.jsonl`（8 行）+ `~/.claude.json` + backups/sessions |
| Gemini CLI | npm `@google/gemini-cli` 0.62.0 | `~/.gemini/projects.json.*.tmp`（首跑写临时文件后未留存主体） |
| Qwen Code | npm `@qwen-code/qwen-code`（bin 自报 0.15.10） | `~/.qwen`（installation_id/debug/skills/output-language.md） |
| OpenCode | npm `opencode-ai` 1.18.33；`opencode run` | **真实 SQLite** `~/.local/share/opencode/opencode.db`（session/message/part 表族 + 7 行 event）+ snapshot/log |
| Command Code | npm `command-code` 1.72.4（bin 自报 0.52.5） | --version/--help 无配置目录 |
| gajae-code | npm `gajae-code` 0.18.1（Yeachan-Heo 官方）+ bun 1.4.2 | `gjc --version` 可运行；无数据目录 |
| OpenClaw | npm `openclaw`（openclaw/openclaw 官方） | **包损坏**：package.json bin 指向不存在的 openclaw.mjs，bin 未链接——安装级证据记录，未运行 |
| goose | 官方 release v1.52.0 二进制（download_cli.sh 网络失败，改 gh api 取 latest release 直装） | `goose session list` 真实响应"无会话"；未建配置目录 |
| crush | npm `@charmland/crush` v0.97.1（charm README 官方渠道） | `crush agent list` 需 TTY 配置，无配置目录 |
| jcode | 官方 `curl jcode.sh/install` v0.89.3 | `~/.jcode`（builds/logs/migrations/telemetry；**无 sessions**——须实际会话） |
| aider | 官方 `aider-install`（uv tool）0.86.2；`aider --yes --message hi` | **真实** `~/.aider/analytics.json`（仅 opt-in 状态 uuid/permanently_disable/asked_opt_in）+ installs.json/caches |

**npm 命名陷阱（适配器安全备注）**：`jcode`（npm）是无关 MVC 框架、
`crush`（npm）是 MediaCrush 残件、`goose-cli` 是数据库迁移工具、
`grok-cli` 是第三方代理、`hermes-agent`（npm）是非官方桥接——
均非目标产品官方分发包，已在 registry 逐一经 repository/homepage 核对排除。

## 现成数据盘点（安装前已存在，与本次无关，清理时保留）

- `~/.local/share/kilo/kilo.db`：kilo 7.6.2 真实库（27 会话/1659 消息，历史测试遗留）；
- `~/.omp/agent/agent.db`：model_usage/usage_history/client_usage 表**全部 0 行**（空骨架）；
- `~/.copilot`：仅 config/ide/logs，无 session-store.db；
- `~/.cline/data/db/*.db`：**Cline CLI 新产品面**（sessions/schedules 表，0 行）——
  与我们已实施的 VS Code 扩展形态（ui_messages JSONL）不同源，矩阵未覆盖，
  记为证据缺口（无用量行，不实施）。

## 真实验证结果（probe_home 只读探针，内存库不落盘）

新增 `crates/core/examples/probe_home.rs`（适配器注册表同步下沉 core，
`adapters::built_in_adapters()`，app 复用）；对 WSL home UNC 只读发现+扫描：

| 适配器 | 结果 |
| --- | --- |
| claude | 发现真实转录；**首轮 fail closed**（首记录 `queue-operation` 不在文档集）→ 据此扩展适配器（见下）→ 复验 complete、8 条、0 事件（`<synthetic>` 占位跳过） |
| kilo | UNC 直读 SQLite staging backup 超时（9p 锁限制）；经 sqlite backup API 一致快照到本地后：**complete、1659 条、1595 事件/27 会话/2 模型，latest_fallback（7.6.2 未收录版本真实回退验证）** |
| opencode | 同上超时；本地快照：**complete、真实 1.18.33 库探测通过、0 事件**（无用量行，不虚构） |
| jcode/goose/crush/aider/gemini/qwen | 无用量载体目录 → 无根发现（符合预期，不虚构） |
| 其余 30+ | 无根发现 |

**SQLite over UNC 发现**：kilo/opencode 适配器的 staging backup
（复制库文件）在 `\\wsl.localhost` 9p 文件系统上超时失败——若用户以手工根
指向 WSL 路径会遇到；错误如实报出（fail closed），未损坏数据。
本地一致性快照路径验证通过。

## Claude Code 2.1.197 真实格式适配（本次新证据驱动的修复）

真实转录固化的格式事实：文件可以 `queue-operation`（enqueue/dequeue）开头；
`attachment`/`last-prompt` 为非用量元数据记录；未登录占位 assistant 为
`model="<synthetic>"` 且 usage 全 0。适配器变更（claude/detect.rs +
transcript_doc1.rs）：三类元数据记录放行跳过（携带 usage 仍 fail closed）；
`<synthetic>` assistant 记 `synthetic_assistant_skipped` 诊断、不产事件
（不把 0 用量占位算作模型调用）。真实脱敏 fixture
`tests/fixtures/claude/real-2.1.197-queue-metadata/`（8 行结构保真，ID 匿名化）

- 合同测试 2 项（放行/0 事件/幂等；usage 载体 fail closed→pending）。
`claude_contract` 4 项全过；WSL 真实文件复验 complete。

## 命令与退出码（关键节点）

| 命令（环境） | 退出码 | 结果 |
| --- | --- | --- |
| `npm install -g`×2 + bun/pip/uv 安装（WSL，用户级） | 0 | 11 产品安装成功（openclaw 包损坏除外） |
| `cargo build --release -p llm-usage-core --example probe_home` | 0 | 探针构建 |
| `probe_home.exe '\\wsl.localhost\...\owent'`（×3 轮） | 0 | 39 适配器 × 真实 home（见上表） |
| `probe_home.exe <本地快照镜像> kilo opencode` | 0 | 1595 事件/27 会话/2 模型 |
| `cargo test -p llm-usage-core --test claude_contract` | 0 | 4 项通过 |
| `npm run verify`（仓库根，推送前） | 0 | 全链通过 |
| `npm run build:desktop`（图标/sample-data 修正后） | 0 | 安装包 2.81 MiB |
| 清理后逐项核验 | — | 全部创建物（目录/二进制/缓存/tmp）不存在；`~/.local/bin` 仅剩既有 npm/npx/omp/pnpm/yarn |

## 清理（验证完成后执行）

npm 卸载 8 包（672 packages removed）+ `rm -rf ~/.local/npm-global ~/.bun`；
`uv tool uninstall aider-chat` + pip 卸载 aider-install/uv；删 goose/jcode 二进制；
删除全部创建的数据目录（.claude/.claude.json/.qwen/.gemini/.config+cache+share
的 opencode/.jcode/.aider/.local/share/uv）；清理 /tmp 产物。
**安装前已存在的目录（.cline/.omp/.copilot/kilo 数据等）一律未动。**
逐项核验无残留。

## 后续发现：CI 暴露的暂存备份竞速（已修复）

本次推送后 CI（Linux）暴露既有缺陷：9 个 SQLite 暂存副本适配器把
backup.step 的 Busy/Locked 无进展重试也计入 done_pages，使"空间上限"与
"超时"两个出口竞速——Linux 上 20.5s 页上限先触发，busy 场景误报
`space cap`（Windows 本地超时先触发故全绿，与 WSL UNC 超时同为暂存
备份路径）。修复（cb28454，2026-09-30）：仅 `StepResult::More` 计入
空间预算，Busy/Locked 只耗时间预算；Windows/WSL 双平台复测一致，
CI 36709419197 全绿。登记见 [m0-ci](m0-ci.md)。

## 未完成/后续

1. 全部产品无凭据未登录：用量事件为零（claude synthetic/opencode 空库），
   真实用量 fixture 仍待有凭据环境产生；
2. goose/crush/command-code/gajae 的数据目录须实际会话才生成，本轮未获载体；
3. Cline CLI（`~/.cline/data/db`）新形态：0 行无证据，未实施，入矩阵缺口；
4. SQLite over UNC 的 staging backup 超时：如后续支持 WSL 手工根，需对 9p
   路径改用流式快照或文档化限制；
5. 安装级证据（版本/目录骨架/npm 命名陷阱）已入本记录，供 adapters 矩阵引用。
