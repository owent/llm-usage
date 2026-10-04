# M2-D：适配器目录化迁移、未知版本兼容尝试与 Codex 逐版本 fixture

## 元信息

| 项目 | 内容 |
| --- | --- |
| 日期 | 2026-09-25 |
| 执行环境 | Windows 11 x64；rustc 1.98.0；Node v24.21.0 / npm 12.0.2 |
| 代码 revision | 未提交工作树（M2-B/C 已验收状态 + 本次新增） |
| 依据合同 | [architecture.md 目录合同](../../design/desktop-usage/architecture.md#adapter-layout)、[未知版本兼容合同](../../design/desktop-usage/architecture.md#unknown-version)；[execution.md M2 目录迁移](../../design/desktop-usage/execution.md#m2-layout)；validation.md V17/V30 |
| 执行方式 | codex/framework/存储链路由主会话实现并作为参考；pi/omp/claude/gemini/qwen 五个目录化迁移由并行子代理按参考实现执行，主会话统一集成验证 |

## 命令与结果

| # | 命令（cwd） | 退出码 | 结果摘要 |
| --- | --- | --- | --- |
| 1 | `cargo test --locked -p llm-usage-core`（desktop/src-tauri） | 0 | **276 passed / 0 failed**（M2-B/C 基线 249 + 净增 27：V17 fallback 场景、逐版本 fixture、目录结构检查、注册表单测等） |
| 2 | `cargo test --locked --workspace`（desktop/src-tauri） | 0 | 278 passed（core 276 + app 2） |
| 3 | `cargo clippy --locked -p llm-usage-core --all-targets -- -D warnings` | 0 | 通过（app crate 存量 main.rs 测试模块顺序警告不在 --all-targets 门禁内，为 M0 遗留非本次引入） |
| 4 | `npm run verify`（仓库根） | 0 | lint:md 94 文件 0 问题（修复 ai-maintenance SKILL.md 存量 MD060 表格填充，HEAD 版本同样不通过，与本次改动无关）、assets:check、test:scripts、svelte-check、fmt --check、clippy、cargo test、vite build 518.01 kB / gzip 176.33 kB 全绿 |
| 5 | `cargo run -p llm-usage-core --example real_verify_codex -- ~/.codex build/.../real-check-codex-m2d` | 0 | 见下节 |
| 6 | `cargo run -p llm-usage-core --example real_verify_{pi,omp,claude,gemini,qwen}` | 0 | pi/omp 数值与 m2bc-resumed 基线逐位一致（迁移不改变统计）；claude/gemini/qwen 维持 no_data/not_found |

## 目录化迁移（V30 结构）

- 六个 Agent 全部迁入 `desktop/src-tauri/crates/core/src/adapters/<agent>/`：
  `mod.rs`（稳定入口 + SourceAdapter impl）、`detect.rs`（探测与版本分派）、
  `versions/mod.rs`（版本注册表：`VERIFIED_VERSION_IMPLS` + `LATEST_IMPL_ID` +
  探测/扫描共用的 `select()`）、`versions/<format_id>.rs`（格式实现）。
  根级 `codex.rs`/`claude.rs`/`pi.rs`/`omp.rs`/`gemini.rs`/`qwen.rs` 删除。
- 产品特有映射下沉：codex → `codex/common.rs`、claude → `claude/common.rs`；
  `usage_map.rs` 保留跨 Agent 共享件（`map_pi_family` pi/omp 同口径、
  `map_genai_usage` gemini/qwen 同形）与尚无适配器目录的未来产品映射
  （kimi/zcode/copilot/kilo，M3–M5 实现时下沉）。
- 结构合同由 `tests/adapter_layout_v30.rs` 锁定：目录/入口/注册表存在性、
  根级无单文件实现、产品映射不在根级、共享映射防误删。
- 公开入口/来源身份/游标/解析上下文兼容：各格式实现解析上下文新增
  `#[serde(default)] version_basis`，旧上下文反序列化单测验证不重置游标
  （codex/claude/pi/omp 各一）；`file_identity`/`generation`/游标格式未变。

## 未知版本兼容尝试（V17 新语义）

- `DetectOutcome::Supported` 增加 `basis: VersionBasis`（known_version / latest_fallback）
  与可空 `format_version`；`UnsupportedVersion` 仅保留给已确认的不兼容版本
  （pi v1/v2 与缺失 version：固定源码证实落盘格式不同，不尝试回退）。
- 兼容标记持久化（schema v3 迁移，`parse_basis` 不参与内容哈希）：
  - `usage_events.parse_basis`：逐事件选择依据；
  - `source_files.format_status`：JSON{format, found_version, basis, compat}；
  - 诊断 `latest_fallback`（每文件每代数一条）；文件状态 `active_compat`。
- 回退失败判定（V30）：fallback 扫描读到记录、零事件且带结构诊断 ⇒
  `incompatible`：不提交事件/游标/聚合，下轮可重新尝试；
  部分可用时保留可校验部分并随诊断返回覆盖缺口。
- schema < 3 的测试冻结旧库按旧列集写入（`parse_basis` 不持久化，等价历史行为）；
  正常打开总是先迁移。
- 测试改写：原“未知版本一律拒绝”用例改为——未收录版本回退成功带标记
  （codex/pi/omp）、版本缺失但 Agent 可识别回退（codex）、仅新增可选字段容忍
  （codex）、结构破坏判不兼容保留旧结果（codex）、部分可用（codex）、
  pi 已确认不兼容版本仍拒绝。

## Codex 逐版本 fixture（M2-A 遗留项）

提取工具 `build/desktop-usage-validation/tools/extract-codex-versions.mjs`（gitignored）：
每版本选最小含 usage 文件、白名单脱敏（数字保留、字符串 REDACTED、ID→anon-N、
cwd→`<PATH>`）、jq 口径独立核算期望值；泄漏核查无 UUID/路径/正文。

| 版本 | 代表文件 | 调用 | Σtotal | 快照对账 | 结论 |
| --- | --- | --- | --- | --- | --- |
| 0.153.0 | 276 行 | 25 | 934,367 | matched | 已验证（rollout_v1），fixture 入库 |
| 0.154.0-alpha.6.1 | 37 行 | 3 | 69,740 | matched | 已验证（rollout_v1），fixture 入库 |
| 0.154.0-alpha.6.2 | 36 行 | 5 | 115,757 | matched | 已验证（rollout_v1），fixture 入库 |
| 0.155.0-alpha.16.3 | （M2-A 3 会话） | 49 等 | — | — | 已验证（M2-A） |

**关键实测发现：0.139–0.151 系列本机全部文件无 `token_usage_record`**
（仅 `event_msg/token_count` 累计快照，如 0.142.5 一个 74 次 token_count 的会话仅
1 条逐次记录都没有）——与 rollout_v1 的逐次载体不同。这些版本按 LatestFallback
尝试后判 incompatible（可见诊断、不伪造数据、游标不推进），
专用旧版实现（需先核验 `last_token_usage` 逐次回声的稳定身份语义）留作后续任务，
未登记为已验证。测试 `codex_versions_v17.rs::legacy_carrier_versions_stay_fallback_*`
锁定该行为。

## 本机真实只读核对（real_verify_codex，2026-09-25）

| 指标 | 本次（M2-D 后） | M2-A 基线（2026-09-24） |
| --- | --- | --- |
| 完整扫描文件 | 61（53 个 0.155/0.154/0.153 已验证 + 8 个新登记版本文件） | 53（仅 0.155.0-alpha.16.3） |
| 判 incompatible（0.139–0.151 旧载体） | 238（原为 212 个 unsupported_version fail closed） | 212 |
| 事件 | 2,578 | 2,620 |
| 重复扫描 | rescan_added=0（幂等） | 同 |
| 汇总（UTC） | calls=2,578；input_total 295,984,414；cache_read 275,255,936；output 1,502,259；total 297,486,673 | calls=2,620；total 269,817,926 |

事件数低于基线的原因：源端保留变化——本机 0.155.0-alpha.16.3 文件现存 4 个
（M2-A 时 53 个完整扫描来自更多 0.155 文件，Codex 源端自动清理了旧 rollout）；
新增登记版本补入部分事件。这是源数据集变化，非解析回归（回归测试与
pi/omp 逐位一致的迁移前后数值另证）。

## 未完成项（显式遗留）

| 项 | 状态 | 后续 |
| --- | --- | --- |
| Codex 0.139–0.151 专用旧版解析器（token_count/last_token_usage 载体） | 未实施 | 需先核验逐次回声的稳定记录身份与调用边界语义；核验后按目录合同在 codex/versions/ 新增实现并逐版本 fixture |
| claude/gemini/qwen 真实 fixture | 本机无数据 | 本机出现数据后按脱敏流程补取复验 |
| fallback 文件在解析器升级后自动重扫 | 未实施 | 已记各适配器 limitations；需显式重扫或后续按 parser_version 变化触发 |
| M6 数据源页展示兼容状态 | 未实现 | M6 |

<a id="证据文件"></a>

## 验证产物

- 适配器目录：`adapters/{codex,claude,pi,omp,gemini,qwen}/`；框架：
  `adapters/framework.rs`；存储：`storage/schema.rs`（v3）、`ingest.rs`。
- 测试：`tests/adapter_layout_v30.rs`、`tests/codex_versions_v17.rs`、
  各适配器 `*_gaps_synthetic.rs`（V17 新语义改写）。
- fixtures：`tests/fixtures/codex/rollout-v{0.153.0,0.154.0-alpha.6.1,0.154.0-alpha.6.2}.*`。
- `build/desktop-usage-validation/`（gitignored）：codex-versions/（提取产物与
  manifest）、real-check-*-m2d/、tools/extract-codex-versions.mjs。
