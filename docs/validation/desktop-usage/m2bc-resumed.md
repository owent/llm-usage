# M2-B/C 恢复完成：pi / oh-my-pi / Claude / Gemini / Qwen 适配器

本文件是 [m2bc-suspended.md](m2bc-suspended.md) 的恢复验收记录。恢复范围 =
中断记录差距表：四源合同/缺口/增量集成测试与 fixtures、oh-my-pi 独立适配器定案、
real_verify examples ×5、本机真实只读核对。

## 元信息

| 项目 | 内容 |
| --- | --- |
| 日期 | 2026-09-25（恢复执行；中断于 2026-09-24） |
| 执行环境 | Windows 11 x64；rustc 1.98.0；Node v24.21.0 / npm 12.0.2 |
| 代码 revision | 未提交工作树（M2-A 已验收状态 + 本次恢复新增） |
| 依据合同 | [adapters.md](../../design/desktop-usage/adapters.md)；[validation.md](../../design/desktop-usage/validation.md) V07/V12/V17；[execution.md](../../design/desktop-usage/execution.md) M2 |

## 命令与结果

| # | 命令（cwd） | 退出码 | 结果摘要 |
| --- | --- | --- | --- |
| 1 | `cargo test --locked -p llm-usage-core`（desktop/src-tauri） | 0 | **249 passed / 0 failed**（中断时基线 143 + 本次净增 106：五源集成测试 103 + omp 内联单测 3） |
| 2 | `cargo clippy --locked -p llm-usage-core --all-targets -- -D warnings` | 0 | 比项目门禁（不带 --all-targets）更严的检查范围也通过；顺手修了 4 处存量/新增测试 lint（见「修复记录」） |
| 3 | `npm run verify`（仓库根） | 0 | lint:md 89 文件 0 问题、assets:check、test:scripts、svelte-check、fmt --check、clippy、cargo test（core 249 + app 2 = **251**）、vite build 全绿 |
| 4 | `cargo run -p llm-usage-core --example real_verify_pi -- ~/.pi/agent build/desktop-usage-validation/real-check-pi` | 0 | 本机真实只读核对，白名单聚合见下 |
| 5 | `cargo run -p llm-usage-core --example real_verify_omp -- ~/.omp/agent build/desktop-usage-validation/real-check-omp` | 0 | 同上 |
| 6 | real_verify_claude / real_verify_gemini / real_verify_qwen | 0 | 均 discovered 0 根（no_data / not_found 核验结果见下） |

## 冷启动简报问题②定案：oh-my-pi 需要独立适配器（已完成）

新建 `adapters/omp.rs`（931 行，3 内联单测），不复用 pi.rs 整体而共享其
pub(crate) 解析件（parse_usage/map_cost/事件构造等）。与 pi 的真实差异
（本机 58 会话文件实读核验）：

- 首行为 `type:"title"`（v=1），session 头在其后：detect 有界读前 4 行定位
  （首行非 title/session ⇒ UnknownFormat；只有 title 无 session ⇒ Pending）。
- assistant 自带 `duration`/`ttft` 浮点毫秒（omp 特有），入库四舍五入。
- **model_change 落盘为组合字段 `model`="provider/model"**（3 个真实脱敏
  fixture 一致），非 pi 的 `modelId`/`provider` 分字段；omp.rs 以组合字段为准、
  分字段作后备（omp 本机未观测分字段形状）。此修正是恢复期间由真实 fixture
  核对发现的实现偏差。
- 子 Agent 按路径形状判定：文件名非 `<ts>_<uuid>.jsonl` 时，父会话取最近的
  `<ts>_<uuid>` 祖先目录名（首个下划线后部分）；嵌套子 Agent 隔代不归名。

## 实现与测试清单

| 源 | 真实 fixture | 合成 fixture 目录 | contract | gaps | incremental |
| --- | --- | --- | --- | --- | --- |
| pi | session-error-zero-usage（1 调用全 0，error） | 7 | 2 | 8 | 8 |
| oh-my-pi | session-glm-reasoning（reasoningTokens=54）、session-k3-cache-abort（7 调用含 aborted）、subagent-community-research（5 调用 sub_agent） | 9 | 4 | 12 | 8 |
| claude | 无（本机 no_data） | 8 | 2 | 11 | 8 |
| gemini | 无（本机 not_found） | 8 | 3 | 11 | 5 |
| qwen | 无（本机 not_found） | 8 | 2 | 11 | 8 |

- 所有真实 fixture 经脱敏提取（`build/desktop-usage-validation/tools/extract-pi-omp.mjs`，
  gitignored）：数字/布尔/null 保留、字符串默认 REDACTED、枚举键白名单、
  ID→anon-N 稳定映射；泄漏核查全量去重后只剩枚举值。每个 fixture 目录配
  `_expectations.md` 手工核算（jq 逐条验算），测试期望与之互核。
- 期望值口径：`input_total` 为派生 input+cacheRead+cacheWrite（usage_map.rs
  map_pi_family）；`total_tokens` 派生 = input_total+output；`source_total`
  直报 totalTokens。恢复期间修正两处期望文档把派生值写成原始 input 合计的错误。
- omp gaps 覆盖：四类辅助 usage 载体、无 usage 的 assistant（计调用不补零）、
  fork 继承去重、嵌套子 Agent、error/aborted、cost>0 映射 estimated（0 不映射）、
  detect 首行闸口四态、未知版本/未知格式 fail closed、未知条目类型单诊断。

## 本机真实只读核对（2026-09-25，白名单聚合）

### pi（~/.pi/agent，pi 0.87.1）

| 指标 | 实测 |
| --- | --- |
| 完整扫描文件 | 2（diagnostics=0） |
| 行/记录/事件 | 130 行 → 37 个模型调用事件 |
| 重复扫描 | rescan_added=0（幂等） |
| 汇总（UTC） | calls=37；input_total 2,805,788；cache_read 2,592,768；output 58,631；total 2,864,419 |

说明：中断记录写「pi sessions 为空（no_data）」是 2026-09-24 上午的旧核验结果；
当日 16:37 起本机开始产生 pi 会话（首个脱敏 fixture 即取自该会话），
本次核对时已有 2 个文件。不变量核验：input_total+output = total（2,805,788+58,631=2,864,419）成立。

### oh-my-pi（~/.omp/agent，omp 18.2.7）

| 指标 | 实测 |
| --- | --- |
| 完整扫描文件 | 59（17 主会话 + 42 子 Agent 文件；diagnostics=0） |
| 行/记录/事件 | 28,004 行 → 8,767 个模型调用事件 |
| 类别计数 | primary=6,334、sub_agent=2,433（auxiliary=0，与本机四类辅助载体均无 usage 的实读一致） |
| 重复扫描 | rescan_added=0（幂等） |
| 汇总（UTC） | calls=8,767；input_total 1,021,931,700；cache_read 1,005,572,583（占比 ~98.4%）；output 4,859,995；total 1,026,791,695 |

不变量核验：input_total+output = total（1,021,931,700+4,859,995=1,026,791,695）成立。

### Claude Code / Gemini CLI / Qwen Code

| 源 | 本机核验结果（2026-09-25 复探） | 结论 |
| --- | --- | --- |
| claude | `~/.claude` 存在（backups/ide/sessions/skills），无 `projects/` 目录；real_verify_claude discovered_roots=0 | no_data |
| gemini | `~/.gemini` 不存在；real_verify_gemini 0 根 | not_found |
| qwen | `~/.qwen` 不存在；real_verify_qwen 0 根 | not_found |

三源适配器与测试基于文档级/固定版本源码依据（文件头逐条标注），真实 fixture
待本机出现数据后按 m0-agent-fixtures 脱敏流程补取。

## fork 已知偏差（pi/omp 同一定案）

capability 原描述称 fork 复制条目「四元组逐字相同 ⇒ 同键同内容 Keep」。实测：
复制条目在 fork 文件中携带 **fork 会话身份**（session_id/parent_session_id 取
本文件头），与源文件已存事件同键不同内容 ⇒ 仲裁为 conflict 并保留先扫者。
净效果正确（不双计、源会话归属保留、distinct 会话数不虚增），但行为是
conflict 而非 Keep。pi.rs/omp.rs 的 capability dedup.fork_copies 文本与
`synthetic-fork-inherited/_expectations.md` 均已按实际行为改写。

## 修复记录（恢复期间）

| 处 | 问题 | 修法 |
| --- | --- | --- |
| tests/qwen_incremental_v12.rs | 4 处写文件缺结尾换行 ⇒ 末行按半行不消费，7/8 失败 | 统一以 `\n` 结尾；8/8 通过 |
| omp.rs model_change | 按 pi 分字段形状解析，真实 omp 是组合字段 | 以 `model`="provider/model" 为准、分字段后备；capability 与文件头同步 |
| omp synthetic-detect-supported | 期望把派生 input_total 误写为 110（=totalTokens） | 改 100（100+0+0），测试与 _expectations.md 同步 |
| omp k3/subagent _expectations.md | 「期望入库」把派生 input_total 写成原始 input 合计 | 改 202,560 / 78,310 并注明派生口径 |
| pi 两个 fixture _expectations.md | markdownlint MD056/MD033 | 补表格缺列、尖括号入码 span |
| models_v05/retention_v14/claude_contract/qwen_incremental | clippy --all-targets 下 4 处 lint（数字分组、needless borrow、type_complexity、useless vec!） | 逐处小修，不影响语义 |

## 未完成项（显式遗留，不属于本次恢复范围）

| 项 | 状态 | 后续 |
| --- | --- | --- |
| Codex 旧版本逐版本 fixture | **部分完成**（2026-09-25，[m2d](m2d-layout-versions.md)：0.153.0/0.154.0-alpha.6.1/6.2 已验证；0.139–0.151 实测无逐次载体，待专用实现核验） | 按 m2d 记录后续条件执行 |
| 适配器目录化迁移与未知版本兼容尝试（execution.md#m2-layout） | **已完成**（2026-09-25，[m2d](m2d-layout-versions.md)） | — |
| OMP_PROFILE 命名 profile 目录规则 | 未核验 | 仅支持默认 ~/.omp，已记 omp.rs limitations |
| claude/gemini/qwen 真实 fixture | 本机无数据 | 本机出现数据后按脱敏流程补取复验 |
| OTel 遥测 | 未实现 | M5 |

<a id="证据文件"></a>

## 验证产物

- 适配器：`adapters/{pi,omp,claude,gemini,qwen,usage_map}.rs`；测试：`tests/{pi,omp,claude,gemini,qwen}_*.rs`、`tests/common/mod.rs`、`tests/fixtures/{pi,omp,claude,gemini,qwen}/`。
- examples：`real_verify_{codex,pi,omp,claude,gemini,qwen}.rs`。
- `build/desktop-usage-validation/real-check-{pi,omp,claude,gemini,qwen}/`（gitignored）：临时核对库；提取工具 `tools/extract-pi-omp.mjs`（gitignored）。
