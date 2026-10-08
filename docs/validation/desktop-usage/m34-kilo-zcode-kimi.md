# M3/M4, partial: Kilo Code, ZCode, Kimi Code and Kimi Work adapters

<a id="m3m4部分kilo-codezcodekimi-codekimi-work-适配器"></a>

M3's first adapter, Kilo, and M4's three products, ZCode/Kimi Code/Kimi Work, are implemented.
The user directed remaining locally absent M3 tools, Cline/OpenCode/MiMo/Zoo/DSH/OpenClaw/Hermes,
to be implemented from documents/source with native acceptance deferred.

<a id="元信息"></a>

## Run information

| Item | Value |
| --- | --- |
| Date | 2026-09-25 |
| Environment | Windows 11 x64; rustc 1.98.0; Node v24.21.0 |
| Code revision | Uncommitted working tree after M6, plus these changes |
| Design references | adapters.md A11/A12/A13/A21; architecture.md#database source SQLite rules; V07/V12/V17/V30 |
| Execution | Three parallel subagents implemented four adapters using samples extracted before the earlier interrupted session; main session integrated scanner registration, Clippy/format fixes and full checks |

<a id="命令与结果主会话集成后"></a>

## Commands and results after integration

| # | Command (directory) | Exit | Result |
| --- | --- | --- | --- |
| 1 | cargo test -p llm-usage-core (desktop/src-tauri) | 0 | **373 passed / 0 failed**, M6 baseline 285 plus net 88 |
| 2 | cargo clippy --workspace / cargo fmt --check | 0 | Core clean; app main.rs test-module-order warning was an existing M0 issue |
| 3 | npm run verify (root) | 0 | 49 test binaries pass; lint clean; Vite build unchanged |
| 4 | APPDATA=temporary cargo run -p llm-usage-m0 -- --headless | 0 | **All seven instances succeeded, 27,311 events**; rescan adds only 3 from active ZCode file |

<a id="适配器结论详细核验结果在各-real_verify-输出与-fixture-期望文档"></a>

<a id="适配器结论详细证据在各-real_verify-输出与-fixture-期望文档"></a>

<a id="适配器核验结果详见-real_verify-输出及测试数据期望文档"></a>

## Adapter findings: real_verify outputs and sample expectations contain details

<a id="kilo-code-clim3a11"></a>

### Kilo Code CLI: M3, A11

- Inspected ~/.local/share/kilo/kilo.db: active 519 MB database plus 30 MB WAL, 25 tables.
  message.data.tokens exists on all 13,342 assistant rows, five exclusive buckets. Session
  token columns compare totals only: 275/276 matched; semantics vary by version, never
  mapped as events. session.cost actually contains model JSON and is ignored. New
  session_message core tables have zero rows and are only recorded as present.
- Read-only SQLite; Busy/lock/CANTOPEN may use a staged Online Backup, bounded to 2s/2GiB
  and removed afterward. Native direct reads did not require staging; synthetic tests cover Busy.
- 7.4.8/7.4.9 samples verified. Observed but unregistered 7.3.42–7.7.12 remain latest_fallback.
- Native check: 255 sessions, 13,342 calls, primary 11,745 plus sub_agent 1,597. Total
  **1,645,598,767** matches independent Python sums field by field. Repeats add nothing;
  254/255 snapshots match, one unsynchronized source snapshot remains a visible diagnostic.

<a id="zcodem4a21"></a>

### ZCode: M4, A21

- Separate input definitions: primary AI SDK camelCase five fields, inputTokens includes
  cache reads; Anthropic snake_case comparison input_tokens excludes them. When both views
  exist, check in+cr+cw==inputTokens and out==outputTokens per row; mismatches get dual_caliber_mismatch.
- Version comes from request.headers["x-zcode-app-version"]; 3.14.3 verified.
- Read-only db.sqlite comparison: active 418/438 match, with in-flight/cancelled mismatches
  visible; static M0 sample 16/16.
- Native check: three files/eight events, stable repeated reads. ~/.zcode/v2 and desktop
  %APPDATA%/zcode storage not connected pending inspection.

<a id="kimi-codem4a12与-kimi-workm4a13"></a>

### Kimi Code: M4, A12; Kimi Work: M4, A13

- Shared adapters/kimi_wire.rs handles metadata/usage.record/echo deduplication/subagent
  comparison/epoch-ms checks; independent directories/registries for protocols 1.5 and 1.4.
- Work uses `conv-*` or `ctitle-*` under daimon, lacks usage.record.agentId and uses bare model
  IDs. Two same-ms cross-file pairs from parallel swarms require session:agent in event keys;
  this fixed first-version conflicts that lost two events.
- All observed times are epoch milliseconds. No second-based sample; out-of-range values
  get diagnostics/skipping, without guessed ×1000 conversion.
- Native values match independent jq sums by field and stay stable on rescan:
  - Code: 13 files, 965 calls, primary 586 + auxiliary 5 + sub_agent 374; total **116,428,813**.
    Ten files match, two echo_subset interrupted steps lack echoes and remain visible; one
    subagent.completed snapshot also equals child totals.
  - Work: 69 files, 1,337 calls, 833+8+496; total **125,225,933**; all 69 comparisons match.

<a id="集成后全源汇总headless-真实采集"></a>

## Integrated source totals: actual headless collection

| Agent | Calls | total_tokens |
| --- | --- | --- |
| kilo-code | 13,342 | 1,645,598,767 |
| oh-my-pi | 8,767 | 1,026,791,695 |
| codex | 2,597 | 299,747,341 |
| kimi-work | 1,337 | 125,225,933 |
| kimi-code | 965 | 116,428,813 |
| zcode | 266 plus 3 from active file | 100,651,285 |
| pi | 37 | 2,864,419 |

Claude/Gemini/Qwen had no local data; existing not_found/no_data status retained.

<a id="未完成项"></a>

## Outstanding work at this stage

| Item | Status | Next step |
| --- | --- | --- |
| Kilo new session_message tables, zero rows | Not collected | Inspect dedicated implementation when Kilo changes |
| ZCode v2/desktop sessions | Not collected | Inspect |
| Kimi usage.record lacks stable ID; provider/per-call latency omitted | unavailable | Connect after upstream correlation keys exist |
| Work positive cache writes/subagent.completed | No native sample | Synthetic paths covered; obtain native sample later |
| Work official default layout/environment | No documentation found | Add manual roots after installation moves |
| Remaining M3 Cline/OpenCode/MiMo/Zoo/DSH/OpenClaw/Hermes | Not implemented | User-directed document/source implementation, native acceptance later |

<a id="验证产物"></a>

<a id="证据文件"></a>

## Validation files

- adapters/{kilo,zcode,kimi-code,kimi-work}/ and adapters/kimi_wire.rs; product mappings
  in Kilo/ZCode common.rs and kimi_wire.rs, removed from usage_map.rs.
- `tests/{kilo,zcode,kimi_code,kimi_work}_{contract,gaps_synthetic,incremental_v12}.rs`;
  `tests/fixtures/{kilo,zcode,kimi-code,kimi-work}/`: real redacted and synthetic samples,
  each with `_expectations.md`.
- real_verify_{kilo,zcode,kimi_code,kimi_work}.rs; ignored
  build/desktop-usage-validation/tools/extract-kimi.mjs.
- src/scanner.rs registers ten adapters.
