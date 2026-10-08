# M2-A：适配器框架与 Codex 适配器

<a id="m2-a-adapter-framework-and-codex-adapter"></a>

<a id="run-information"></a>

## 元信息

| 项目 | 内容 |
| --- | --- |
| 日期 | 2026-09-24 |
| 执行环境 | Windows 11 Pro 26200 x64；Rust 1.98.0；Node v24.21.0 |
| 代码 revision | 工作树未提交改动 |
| 设计依据 | execution.md M2；adapters.md 交付要求与必需样本；architecture.md 增量策略；validation.md V07/V12/V17 |

<a id="commands-and-results"></a>

## 命令与结果

| # | 命令（cwd） | 退出码 | 结果摘要 |
| --- | --- | --- | --- |
| 1 | `npm run verify`（仓库根） | 0 | lint / assets / svelte-check / fmt / clippy -D warnings / cargo test / vite build 全绿 |
| 2 | `cargo test --locked --workspace`（desktop/src-tauri） | 0 | **135 passed / 0 failed**（M1 的 72 + 本阶段新增） |
| 3 | `cargo run -p llm-usage-core --example real_verify_codex -- ~/.codex build/desktop-usage-validation/real-check` | 0 | 本机真实只读核对，允许字段的汇总见下 |

<a id="implemented-changes"></a>

## 实现清单

- `adapters/framework.rs`：SourceAdapter trait（discover→detect→scan→capability）、
  扫描运行管线（generation 重扫、文件未变化则跳过读取、fail closed、诊断输出字段限制）、
  快照对账（per-call 合计 vs 最终快照 + compaction 携带）。
- `adapters/jsonl.rs`：JSONL 读取器（V07 全部边界：半行不前移游标、跨块 UTF-8、
  8 MiB 单行上限受限报错、BOM、坏行隔离无正文、同长替换/截断/改名 generation 重探测）。
- `adapters/codex.rs`：Codex 0.155.0-alpha.16.3 适配器。token_usage_record 逐次（model_call，
  resp:{response_id} 身份，缺 ID 回退 seq:{session}:行号）、token_count 累计快照分格式不混算、
  compaction 携带记录去重与对账、turn_context 模型归属（无模型归属依据时为 unknown）、
  codex_vscode→host_application=vscode、子 Agent 类别映射。能力声明落库 source_instances。
- 重复 final 语义修正（ingest.rs）：同键同内容仅时间不同的重报按相同内容保留（Keep），
  不误标 conflict；真正的同修订内容分歧仍保留 conflict 诊断。

<a id="tests-63-new-cases-all-real-temporary-filessqlite"></a>

## 测试（新增 63 个，全部真实临时文件/SQLite）

- `codex_contract.rs`：M0 三份真实测试数据的完整采集流程的期望（单调用/49 调用/多模型 compaction），
  能力声明结构、发现/实例/落库。
- `codex_gaps_synthetic.rs`：合成样本（目录头标注 synthetic）：重复 final、子 Agent、
  cache write>0、无 usage 的 tool/user 消息、缺 response_id 回退身份；V17 未知版本/格式 fail closed。
- `codex_incremental_v12.rs`：追加增量、半行等待、截断/同长替换/改名重扫、按读取上限分段、
  重复扫描不增量、冲突重复 final 保留现存。
- `jsonl_v07.rs`：V07 逐条边界；`review_regressions.rs`：回归守卫。

<a id="read-only-local-checks-codex-265-rollout-files"></a>

## 本机真实只读核对（~/.codex，265 个 rollout 文件）

| 指标 | 实测 |
| --- | --- |
| 完整扫描文件 | 53（另有 6 degraded：空文件/异常 meta） |
| fail closed | 212 个文件，均为其他版本（0.154.0-alpha.6.2×89、0.155.0-alpha.16×18、0.146/147/149/150/151/153 等 12+ 版本）；符合设计规则，多版本支持需逐版本测试数据 |
| 行/记录/事件 | 23,059 行 → 2,620 个模型调用事件 |
| 快照对账 | matched 48 / mismatch 5 / no_snapshot 0；mismatch 均为"最终快照 > 逐次合计"（差 2.9M–31.5M token），与源端保留截断或跨文件会话一致，差异已存 reconcile_mismatch 诊断，不静默吸收 |
| 重复扫描 | rescan_added=0（未重复入库） |
| 汇总（UTC） | calls=2620；input_total 268,545,932；cache_read 257,653,888（占比 ~95.9%）；output 1,271,994；total 269,817,926；模型 3 种 |

诊断计数：unsupported_version 212、snapshot_out_of_order 76、unexpected_session_meta 6、
reconcile_mismatch 5、unknown_record_type 4、usage_shape_deviation 1。无正文/路径泄露
（限制输出字段的脚本 examples/real_verify_codex.rs，临时库在 gitignored 的 build/desktop-usage-validation/real-check/）。

<a id="completion-after-interruption"></a>

## 中断后收尾（主会话完成）

M2-A 子代理因额度中断，以下由其代码基础上修复后验收：能力声明 cost 断言、
重复 final 保留已有记录（ingest 比较规则）、同长替换测试数据（原测试数据破坏 cli_version 与 fail-closed 冲突，
改为整行交换）、clippy 6 处、fmt、bundle-report tar 加 --force-local（Windows 路径冒号）。

<a id="outstanding-work-at-this-stage"></a>

## 未完成项

| 项 | 状态 | 后续 |
| --- | --- | --- |
| Codex 其他版本（0.146–0.155 系） | 部分完成（[m2d](m2d-layout-versions.md)：0.153.0/0.154.0-alpha.6.1/6.2 已验证；0.139–0.151 无逐次记录格式待专用实现；未收录版本按 latest_fallback 尝试） | 0.139–0.151 核验 token_count/last_token_usage 身份语义后扩展 |
| OTel 遥测接入 | 未实现 | M5 |
| 真实 GUI 数据源页 | 未实现 | M6 |

<a id="证据文件"></a>

<a id="validation-files"></a>

## 验证产物

- `desktop/src-tauri/crates/core/src/adapters/`（framework/jsonl/codex）、`tests/`（codex_*、jsonl_v07、review_regressions、fixtures/codex/）。
- `build/desktop-usage-validation/real-check/`（gitignored）：real-check.sqlite 临时核对库。
