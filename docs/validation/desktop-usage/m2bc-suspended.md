# M2-B/C 中断记录：pi/oh-my-pi/Claude/Gemini/Qwen 适配器（半成品保留，未验收）

> **已于 2026-09-25 恢复并完成验收**，见 [m2bc-resumed.md](m2bc-resumed.md)。
> 本文保留为中断经过与冷启动简报的历史记录；下方「与 M2 完成条件的差距」
> 状态列已按恢复结果更新。

本文件不是通过记录。M2-B（pi + oh-my-pi）与 M2-C（Claude Code/Gemini CLI/Qwen Code）
两个并行实现任务由用户决定于 2026-09-24 中断，工作树半成品经核验编译与测试全绿后保留，
待用户恢复后继续。恢复前不得以「M2 适配器已完成」对外表述。

## 元信息

| 项目 | 内容 |
| --- | --- |
| 日期 | 2026-09-24 |
| 执行环境 | Windows 11 x64；rustc 1.98.0；Node v24.21.0 / npm 12.0.2 |
| 代码 revision | 未提交工作树：M2-A 已验收状态（另一会话已 `git add`）+ 本次中断留下的未跟踪/未暂存改动 |
| 依据合同 | [adapters.md](../../design/desktop-usage/adapters.md) 接入矩阵；[validation.md](../../design/desktop-usage/validation.md) V07/V12/V17 |

## 中断经过与恢复方式

| 任务 | 范围 | 运行时长 | 中断原因 | 恢复方式 |
| --- | --- | --- | --- | --- |
| M2-B（后台子代理 agent-16） | pi + oh-my-pi 适配器 | 约 57 分钟 | 用户要求暂停并落地文档 | 本机：`Agent(resume="agent-16", "继续")`；换机后失效，用下方冷启动简报 |
| M2-C（后台子代理 agent-17） | Claude/Gemini/Qwen 适配器 | 约 57 分钟 | 同上 | 本机：`Agent(resume="agent-17", "继续")`；换机后失效，用下方冷启动简报 |

## 换机恢复方案（2026-09-24 增补）

### 换机后会失效的东西

- **子代理会话**：agent-16/17 的上下文只存在本机 `~/.kimi-code/sessions/`，不随仓库转移。
  换机后由「冷启动简报」（本节末尾）替代，新机器上的 Agent 读简报冷启动即可。
- **未提交的工作树**：M0 以来的全部成果（含本记录）目前只在本机工作树/暂存区，
  `git log` 最新仍是 `9394264 P1`。不转移则全部丢失。
- **`build/desktop-usage-validation/`**：真实核对的原始产物被 gitignore，不随 git 转移；
  其结论已固化在 m2a-codex.md / m0-agent-fixtures.md（会随 git 转移）。需要原始产物时手动复制该目录。
- **本机真实数据探测结论**：claude/gemini/qwen 本机 not_found、pi sessions 为空（no_data）
  是**这台机器**的证据。新机器可能装有这些 Agent、存在真实数据——既是新证据机会，
  也意味着恢复后必须按新机器实际重新探测，不得沿用旧机结论。

### 第一步：把工作树固化并转移（三选一）

| 方案 | 命令（旧机执行） | 新机恢复 | 适用 |
| --- | --- | --- | --- |
| A. 提交并推送（推荐） | `git add -A && git commit -m "M0-M2 阶段性成果与 M2-B/C 中断半成品" && git push origin main`（LFS 对象随 push 自动上传） | `git clone git@github.com:owent/llm-usage.git && cd llm-usage && git lfs pull` | 有 GitHub 访问权；历史可追溯，最稳 |
| B. git bundle 单文件 | 先按 A 提交，再 `git bundle create llm-usage.bundle main` | `git clone llm-usage.bundle` | 不想推送远端；**注意 bundle 不含 LFS 对象**，需另复制 `.git/lfs/` 目录，否则 svg/ico 等是指针 |
| C. 整目录文件同步 | 无（用网盘/U盘/rsync 复制整个 `llm-usage/` 含 `.git/`，`node_modules` 与 `target/` 可排除） | 直接进目录工作 | 最省事，工作树与暂存区逐字节保留；但无提交历史增量，丢失恢复点 |

无论哪个方案，新机首次进入后重建依赖并验证基线：

```bash
npm ci                 # 根目录；desktop/ 依赖由根 scripts 级联（如缺则再 npm --prefix desktop ci）
npm run verify         # 基线：core 143 + app 2 = 145 passed，lint/clippy/fmt/svelte-check 全绿
```

### 第二步：新机环境前提

| 依赖 | 要求 | 旧机实测 |
| --- | --- | --- |
| OS | Windows 11 x64（首发验收平台）；WSL2 可做 Linux 构建冒烟 | Windows 11 x64 |
| Node.js | 22+（AGENTS.md 合同） | v24.21.0 / npm 12.0.2 |
| Rust | stable（edition 2021，无 toolchain 文件锁定） | 1.98.0 |
| git-lfs | 必需（`.gitattributes` 对 svg/png/ico 等启用 LFS） | 已启用 |
| WebView2 | GUI 冒烟（`npm run dev:desktop`）需要 | 通过 |

### 第三步：冷启动简报（换机后给新 Agent，替代 agent resume）

> 你在 `llm-usage` 仓库继续 M2-B/C。先读：`Plan.md`（状态表）、
> `docs/validation/desktop-usage/m2bc-suspended.md`（本文件，含半成品清单与差距表）、
> `docs/validation/desktop-usage/m2a-codex.md`（已验收的适配器长什么样，照此标准）、
> `docs/design/desktop-usage/adapters.md` 与 `data-contract.md`（口径合同）。
> 工作树已有 pi/claude/gemini/qwen 四个适配器半成品与共享 usage_map（文件清单见上节），
> 编译测试全绿但只有内联单测。你的任务：①对照 codex 的 contract/gaps/incremental
> 测试组补齐四源集成测试（`tests/common/mod.rs` 辅助函数已预留，fixture 目录待建）；
> ②确认 oh-my-pi 是否需要独立适配器文件；③按本机实际安装重新探测真实数据
> （旧机 claude/gemini/qwen not_found、pi sessions 为空——那是旧机证据，新机重查），
> 有真实样本则按 m0-agent-fixtures 脱敏流程提取核验，没有则记录 not_found/no_data；
> ④补 real_verify example 与验收记录。规则：测试不 mock、未知不补零、
> 未知版本 fail closed、不改 previous-draft/、未经授权不 commit/push。
> 完成后 `npm run verify` 必须全绿（基线 145 passed，只会增不会减）。

## 半成品清单（全部保留在工作树，未提交）

| 文件 | 状态 | 内容与证据等级 |
| --- | --- | --- |
| `desktop/src-tauri/crates/core/src/adapters/pi.rs` | 未跟踪，985 行，3 单测 | pi session JSONL v3；固定源码证据 pi-mono b4559750（联网只读核对）；本机 pi 0.87.1 sessions 为空，真实核对状态 no_data |
| `desktop/src-tauri/crates/core/src/adapters/claude.rs` | 未跟踪，680 行，3 单测 | 仅文档级证据（A01），文件头自标「待真实样本」；本机 not_found |
| `desktop/src-tauri/crates/core/src/adapters/gemini.rs` | 未跟踪，658 行，2 单测 | 同上量级，未核实真实样本 |
| `desktop/src-tauri/crates/core/src/adapters/qwen.rs` | 未跟踪，705 行，2 单测 | 同上量级，未核实真实样本 |
| `desktop/src-tauri/crates/core/src/adapters/usage_map.rs` | 未暂存改动 +195 行 | `PiFamilyUsage`/`map_pi_family`（pi-mono b4559750 与 oh-my-pi 62bc57b 固定源码口径）、`ClaudeTranscriptUsage` 等共享映射 |
| `desktop/src-tauri/crates/core/src/adapters/mod.rs` | 未暂存改动 +5 行 | 注册四个新模块 |
| `desktop/src-tauri/crates/core/tests/common/mod.rs` | 未暂存改动 +114 行 | M2-C 测试辅助（claude/qwen/gemini 临时目录与 fixture 路径构造）；**对应 `tests/fixtures/{claude,qwen,gemini}/` 目录与集成测试尚未创建**，辅助函数当前无调用方 |

oh-my-pi 适配器未见独立文件（仅共享 `map_pi_family` 口径）；M2-B 的 oh-my-pi 部分是否
完成取样未核实，恢复后先向 agent-16 确认。

## 命令与结果（中断后核验）

| # | 命令（cwd） | 退出码 | 结果摘要 |
| --- | --- | --- | --- |
| 1 | `cargo test --locked -p llm-usage-core`（desktop/src-tauri） | 0 | **143 passed / 0 failed**（含四个新适配器内联单测 10 个：pi 3、claude 3、gemini 2、qwen 2） |
| 2 | `npm run verify`（仓库根） | 0 | 全链路绿：lint:md 57 文件 0 问题（本次在 `.markdownlint-cli2.jsonc` 的 ignores 增补 `.venv/**`，该目录已被 `.gitignore` 忽略，为本地 Python 环境非项目文档）、assets:check、test:scripts、svelte-check、cargo fmt --check、clippy -D warnings、cargo test --locked（core 143 + app 2 = 145，对比 M2-A 记录 135）、vite build 518.01 kB / gzip 176.33 kB |

## 与 M2 完成条件的差距（恢复后待办）

> 2026-09-25 恢复完成，各项状态如下；验收细节见 [m2bc-resumed.md](m2bc-resumed.md)。

| 项 | 状态 | 原因 | 后续条件 |
| --- | --- | --- | --- |
| 合同级集成测试（对照 Codex 的 contract/gaps/incremental 测试组） | **已完成**（2026-09-25） | 五源 15 个集成测试文件 103 用例全绿；oh-my-pi 定案为独立适配器 omp.rs | — |
| 合成/真实 fixture | **已完成**（2026-09-25） | pi 真实 1 + 合成 7；omp 真实 3 + 合成 9；claude/gemini/qwen 合成各 8（本机无真实数据，no_data/not_found 证据在恢复记录） | 三源真实样本待本机出现数据后按 m0 脱敏流程补取 |
| 本机真实只读核对（对照 `real_verify_codex` example） | **已完成**（2026-09-25） | examples ×6 齐备；pi/omp 真实核对通过（幂等、不变量成立）；claude/gemini/qwen 记录 no_data/not_found | — |
| 重复扫描不增量、主/辅助/子 Agent 覆盖可见（M2 完成条件） | **已验收**（2026-09-25） | V12 增量套件五源全绿；omp 真实核对类别计数 primary/sub_agent 分列 | — |
| Codex 旧版本 fail closed 逐版本 fixture | 未执行（M2-A 遗留） | 同 M2-A 记录 | 按 m2a-codex.md 后续条件执行 |
| 适配器目录化迁移与未知版本兼容尝试（execution.md#m2-layout） | 未执行 | M2 完成条件，不在中断记录待办内 | 另行排期 |

## 证据文件

- 适配器内联证据：四个 `adapters/*.rs` 文件头注释（固定源码 commit 与文档出处逐条在列）。
- 本记录命令输出以工作树复跑为准；无新增脱敏产物写入 `build/desktop-usage-validation/`。
