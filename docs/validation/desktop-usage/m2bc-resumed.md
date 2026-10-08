# M2-B/C recovery: pi, oh-my-pi, Claude, Gemini and Qwen adapters

<a id="m2-bc-恢复完成pi--oh-my-pi--claude--gemini--qwen-适配器"></a>

This records recovery acceptance for [the interruption](m2bc-suspended.md). Scope follows its
gap table: four sources' integration requirements/missing-field/incremental tests and test data,
an independent oh-my-pi adapter decision, five real_verify examples and local read-only native checks.

<a id="元信息"></a>

## Metadata

| Item | Value |
| --- | --- |
| Date | 2026-09-25 recovery; interrupted 2026-09-24 |
| Environment | Windows 11 x64; rustc 1.98.0; Node v24.21.0 / npm 12.0.2 |
| Revision | Uncommitted worktree: accepted M2-A changes plus recovery additions |
| Requirements | [Adapters](../../design/desktop-usage/adapters.md); [validation](../../design/desktop-usage/validation.md) V07/V12/V17; [execution plan](../../design/desktop-usage/execution.md) M2 |

<a id="命令与结果"></a>

## Commands and results

| # | Command and cwd | Exit | Results |
| --- | --- | --- | --- |
| 1 | cargo test --locked -p llm-usage-core; desktop/src-tauri | 0 | 249 passed/zero failed: interruption baseline 143 plus 106, comprising 103 integration tests across five sources and three omp inline tests |
| 2 | cargo clippy --locked -p llm-usage-core --all-targets -- -D warnings | 0 | Passed the broader all-targets check; four existing/new test lint issues were fixed as listed below |
| 3 | npm run verify; repository root | 0 | Markdown lint: 89 files/zero issues; assets/scripts/Svelte/fmt/clippy passed; core 249/app 2 = 251 Rust tests; Vite build passed |
| 4 | cargo run -p llm-usage-core --example real_verify_pi -- ~/.pi/agent build/desktop-usage-validation/real-check-pi | 0 | Local read-only native check; permitted aggregates below |
| 5 | cargo run -p llm-usage-core --example real_verify_omp -- ~/.omp/agent build/desktop-usage-validation/real-check-omp | 0 | Same check for omp |
| 6 | real_verify_claude / real_verify_gemini / real_verify_qwen | 0 | Each discovered zero roots; no_data/not_found results below |

<a id="恢复简报问题结论oh-my-pi-需要独立适配器已完成"></a>

<a id="冷启动简报问题定案oh-my-pi-需要独立适配器已完成"></a>

## Independent oh-my-pi adapter decision

Created adapters/omp.rs with 931 lines/three inline tests. It shares pub(crate) pi parsing helpers
(parse_usage/map_cost/event construction), rather than the entire adapter. Native reads of
58 local session files established these differences:

- First line is type:"title", v=1; the session header follows. Detection reads at most four
  lines: a first line other than title/session is UnknownFormat; title without session is Pending.
- Assistants carry omp-specific floating-point duration/ttft milliseconds, rounded for storage.
- model_change writes a combined model="provider/model" field, consistent in three redacted
  native samples. omp uses this field first and separate modelId/provider fields as fallback;
  the separate-field shape was not observed locally. Native samples exposed this implementation
  error during recovery.
- For sub-agents, filenames other than `<ts>_<uuid>.jsonl` use the nearest `<ts>_<uuid>` ancestor
  directory as parent, taking the portion after its first underscore. Nested sub-agents do not
  inherit a more distant ancestor's identity.

<a id="实现与测试清单"></a>

## Implementation and tests

| Source | Native samples | Synthetic sample count | contract | gaps | incremental |
| --- | --- | --- | --- | --- | --- |
| pi | session-error-zero-usage: one error call, all zero | 7 | 2 | 8 | 8 |
| oh-my-pi | session-glm-reasoning: reasoningTokens=54; session-k3-cache-abort: seven calls including aborted; subagent-community-research: five sub_agent calls | 9 | 4 | 12 | 8 |
| Claude | None; local no_data | 8 | 2 | 11 | 8 |
| Gemini | None; local not_found | 8 | 3 | 11 | 5 |
| Qwen | None; local not_found | 8 | 2 | 11 | 8 |

- Ignored build/desktop-usage-validation/tools/extract-pi-omp.mjs extracts/redacts native samples.
  Numbers/booleans/null remain; strings default to REDACTED, enum keys are allowlisted, and
  IDs map stably to anon-N. Deduplicated leakage checks left only enum values. Each sample
  directory has manually calculated _expectations.md, independently checked record-by-record
  with jq and compared against test expectations.
- input_total is derived input+cacheRead+cacheWrite through usage_map.rs map_pi_family;
  total_tokens is derived input_total+output; source_total preserves reported totalTokens.
  Recovery fixed two expectation documents that confused derived input_total with raw input sums.
- omp gaps tests cover four auxiliary usage formats, assistants without usage (count calls,
  leave tokens unknown), copied fork entries, nested sub-agents, error/aborted, cost>0 as
  estimated (zero omitted), four first-line detection outcomes, rejection of unknown versions/
  formats and one diagnostic for an unknown entry type. These describe this historical stage.

<a id="本机真实只读核对2026-09-25白名单聚合"></a>

<a id="本机真实只读核对2026-09-25允许保存的汇总字段"></a>

## Local read-only native checks: 2026-09-25 permitted aggregates

<a id="pipiagentpi-0871"></a>

### pi: ~/.pi/agent, pi 0.87.1

| Metric | Result |
| --- | --- |
| Completely scanned files | Two; diagnostics=0 |
| Lines/records/events | 130 lines produced 37 model-call events |
| Repeat scan | rescan_added=0 |
| UTC totals | calls=37; input_total 2,805,788; cache_read 2,592,768; output 58,631; total 2,864,419 |

The interruption's empty pi sessions/no_data result was measured on the morning of 2026-09-24.
Local sessions began at 16:37 that day; the first redacted sample came from that session.
Two files existed at recovery. The invariant holds: 2,805,788+58,631=2,864,419.

<a id="oh-my-piompagentomp-1827"></a>

### oh-my-pi: ~/.omp/agent, omp 18.2.7

| Metric | Result |
| --- | --- |
| Completely scanned files | 59: 17 primary/42 sub-agent files; diagnostics=0 |
| Lines/records/events | 28,004 lines produced 8,767 model-call events |
| Categories | primary=6,334; sub_agent=2,433; auxiliary=0, consistent with native reads finding no usage in the four auxiliary formats |
| Repeat scan | rescan_added=0 |
| UTC totals | calls=8,767; input_total 1,021,931,700; cache_read 1,005,572,583, about 98.4%; output 4,859,995; total 1,026,791,695 |

The invariant holds: 1,021,931,700+4,859,995=1,026,791,695.

<a id="claude-code--gemini-cli--qwen-code"></a>

### Claude Code, Gemini CLI and Qwen Code

| Source | Local discovery repeated 2026-09-25 | Result |
| --- | --- | --- |
| Claude | ~/.claude exists with backups/ide/sessions/skills, but no projects/; real_verify_claude discovered_roots=0 | no_data |
| Gemini | ~/.gemini absent; real_verify_gemini found zero roots | not_found |
| Qwen | ~/.qwen absent; real_verify_qwen found zero roots | not_found |

These three adapters/tests rely on documentation/fixed-source references listed in their
headers. Extract native samples under m0-agent-fixtures redaction rules when local data appears.

<a id="fork-的实际处理结果piomp-一致"></a>

<a id="fork-已知偏差piomp-同一定案"></a>

## Observed fork behavior: pi and omp

The original capability text claimed identical copied four-field keys/content would select Keep.
In native checks, copied entries carry the fork file's session_id/parent_session_id from its
header. They therefore share an existing key but differ in content, producing conflict and
retaining the first scanned record. Usage is not counted twice, source-session ownership is
retained and distinct-session counts do not increase; the result is conflict rather than Keep.
pi.rs/omp.rs dedup.fork_copies descriptions and synthetic-fork-inherited/_expectations.md now
describe the observed behavior.

<a id="修复记录恢复期间"></a>

## Recovery fixes

| Location | Problem | Correction |
| --- | --- | --- |
| tests/qwen_incremental_v12.rs | Four writes lacked trailing newline; incomplete last lines were not consumed, causing seven of eight tests to fail | Added \n endings; eight of eight passed |
| omp.rs model_change | Parsed pi's separate fields; native omp uses a combined field | Prefer model="provider/model", retain separate-field fallback; updated capability/header |
| omp synthetic-detect-supported | Derived input_total expectation was incorrectly 110, equal to totalTokens | Changed to 100, from 100+0+0; synchronized test/_expectations.md |
| omp k3/subagent _expectations.md | Stored input_total described as raw-input sum | Corrected to 202,560/78,310 and explained derivation |
| Two pi sample _expectations.md files | markdownlint MD056/MD033 | Added missing table columns and code spans around angle brackets |
| models_v05/retention_v14/claude_contract/qwen_incremental | Four all-targets Clippy issues: numeric grouping, needless borrow, type_complexity, useless vec! | Small local changes preserving semantics |

<a id="未完成项显式遗留不属于本次恢复范围"></a>

## Remaining work outside this recovery

| Item | Status | Follow-up |
| --- | --- | --- |
| Version-specific legacy Codex samples | Partial, 2026-09-25; [M2-D](m2d-layout-versions.md): 0.153.0/0.154.0-alpha.6.1/6.2 verified; 0.139–0.151 lack per-call records, requiring a separately verified implementation | Follow M2-D conditions |
| Adapter directories/unknown-version compatibility, execution.md#m2-layout | Complete, 2026-09-25; [M2-D](m2d-layout-versions.md) | — |
| OMP_PROFILE named-profile paths | Unverified | Default ~/.omp only; limitation recorded in omp.rs |
| Claude/Gemini/Qwen native samples | No local data | Extract/redact/recheck when data appears |
| OTel telemetry | Not implemented at this stage | M5 |

<a id="验证产物"></a>

<a id="证据文件"></a>

## Verification artifacts

- Adapters: adapters/{pi,omp,claude,gemini,qwen,usage_map}.rs; tests:
  tests/{pi,omp,claude,gemini,qwen}_*.rs, tests/common/mod.rs and tests/fixtures/{pi,omp,claude,gemini,qwen}/.
- Examples: real_verify_{codex,pi,omp,claude,gemini,qwen}.rs.
- Ignored build/desktop-usage-validation/real-check-{pi,omp,claude,gemini,qwen}/ contains temporary
  verification databases; tools/extract-pi-omp.mjs is the ignored extraction tool.
