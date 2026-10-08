# M2-D: Adapter directories, unknown-version reads and per-version Codex samples

<a id="m2-d适配器目录化迁移未知版本兼容尝试与-codex-逐版本测试样本"></a>

<a id="m2-d适配器目录化迁移未知版本兼容尝试与-codex-逐版本-fixture"></a>

<a id="元信息"></a>

## Run information

| Item | Value |
| --- | --- |
| Date | 2026-09-25 |
| Environment | Windows 11 x64; rustc 1.98.0; Node v24.21.0 / npm 12.0.2 |
| Code revision | Uncommitted tree: accepted M2-B/C plus this work |
| Design references | [Adapter layout](../../design/desktop-usage/architecture.md#adapter-layout), [unknown versions](../../design/desktop-usage/architecture.md#unknown-version), [M2 migration](../../design/desktop-usage/execution.md#m2-layout), validation.md V17/V30 |
| Execution | Main session implemented Codex/framework/storage as the reference; five parallel subagents migrated Pi/OMP/Claude/Gemini/Qwen; main session integrated and validated |

<a id="命令与结果"></a>

## Commands and results

| # | Command/directory | Exit | Result |
| --- | --- | --- | --- |
| 1 | cargo test --locked -p llm-usage-core, desktop/src-tauri | 0 | 276 passed/0 failed: M2-B/C baseline 249 plus net 27, covering V17 fallback, version samples, directories, registries |
| 2 | cargo test --locked --workspace, desktop/src-tauri | 0 | 278 passed: core 276, app 2 |
| 3 | cargo clippy --locked -p llm-usage-core --all-targets -- -D warnings | 0 | Passed; existing app main.rs test-module ordering warning outside this core check, inherited from M0 |
| 4 | npm run verify, repository root | 0 | Markdown 94 files/0 issues after fixing existing SKILL.md MD060 table padding, also failing at HEAD; assets/scripts/Svelte/fmt/Clippy/Rust/build pass; Vite 518.01 kB, gzip 176.33 kB |
| 5 | cargo run -p llm-usage-core --example real_verify_codex against ~/.codex, output build/.../real-check-codex-m2d | 0 | Native results below |
| 6 | real_verify examples for Pi/OMP/Claude/Gemini/Qwen | 0 | Pi/OMP exactly match m2bc-resumed values; Claude/Gemini/Qwen remain no_data/not_found |

<a id="目录化迁移v30-结构"></a>

## Directory migration: V30 structure

- Six agents moved into desktop/src-tauri/crates/core/src/adapters/agent/: mod.rs stable
  entry and SourceAdapter implementation; detect.rs detection/version selection;
  versions/mod.rs registry with VERIFIED_VERSION_IMPLS, LATEST_IMPL_ID and shared select();
  versions/format_id.rs format implementation. Removed root codex.rs/claude.rs/pi.rs/
  omp.rs/gemini.rs/qwen.rs.
- Product mappings moved to codex/common.rs and claude/common.rs. usage_map.rs retains
  cross-agent map_pi_family for Pi/OMP and map_genai_usage for Gemini/Qwen, plus future
  Kimi/ZCode/Copilot/Kilo mappings awaiting M3–M5 adapters.
- tests/adapter_layout_v30.rs checks directories/entries/registries, absence of root
  single-file implementations and product mappings, and preservation of shared mappings.
- Public entry/source identity/cursors remain compatible. Format parse contexts add
  version_basis with serde(default); Codex/Claude/Pi/OMP each test old-context deserialization
  without cursor reset. file_identity/generation/cursor formats unchanged.

<a id="未知版本兼容尝试v17-新语义"></a>

## Unknown-version reads: New V17 behavior

- DetectOutcome::Supported adds basis: VersionBasis, known_version/latest_fallback,
  and nullable format_version. UnsupportedVersion remains only for confirmed incompatible
  versions: fixed source establishes a different saved layout for Pi v1/v2/missing version,
  so these do not attempt fallback.
- Schema v3 persists compatibility information; parse_basis is excluded from content hashes:
  usage_events.parse_basis per event; source_files.format_status JSON with format/
  found_version/basis/compat; latest_fallback diagnostic once per file/generation;
  active_compat file status.
- V30 fallback failure: records read, zero events and structural diagnostics means incompatible.
  No event/cursor/aggregate commit; next run may retry. Partially usable files retain validated
  records and diagnostics describing incomplete coverage.
- Tests for schema<3 freeze the old column set without persisting parse_basis, matching
  historical behavior. Normal database opening always migrates first.
- Revised tests replace blanket unknown-version rejection with marked successful fallback
  for Codex/Pi/OMP; identifiable Codex without version; added optional fields; damaged structure
  rejected while retaining old results; partial Codex reads; confirmed incompatible Pi rejected.

<a id="codex-逐版本-fixturem2-a-遗留项"></a>

<a id="codex-逐版本测试样本m2-a-遗留项"></a>

## Per-version Codex samples: Remaining M2-A work

Ignored extraction tool build/desktop-usage-validation/tools/extract-codex-versions.mjs
chooses the smallest usage-containing file per version. Redaction preserves numbers,
replaces strings with REDACTED, IDs with anon-N and cwd with &lt;PATH&gt;. jq independently
calculates expected values. Leak checks find no UUIDs/private paths/message text.

| Version | Representative file | Calls | Total sum | Snapshot comparison | Result |
| --- | --- | --- | --- | --- | --- |
| 0.153.0 | 276 lines | 25 | 934,367 | matched | rollout_v1 verified; sample added to the repository |
| 0.154.0-alpha.6.1 | 37 lines | 3 | 69,740 | matched | rollout_v1 verified; sample added to the repository |
| 0.154.0-alpha.6.2 | 36 lines | 5 | 115,757 | matched | rollout_v1 verified; sample added to the repository |
| 0.155.0-alpha.16.3 | M2-A three sessions | 49 and others | — | — | Verified in M2-A |

**Native finding: all inspected local 0.139–0.151 files lack token_usage_record.** They
contain event_msg/token_count cumulative snapshots: for example, a 0.142.5 session has
74 token_count records and no individual usage records. Unlike rollout_v1, these formats
remain LatestFallback and then incompatible, with visible diagnostics, no invented data
and no cursor advancement. A dedicated legacy parser requires verification of stable
last_token_usage record identity and call boundaries first; these versions were not registered
as verified. codex_versions_v17.rs tests named legacy_carrier_versions_stay_fallback_* fix this behavior.

<a id="本机真实只读核对real_verify_codex2026-09-25"></a>

## Native read-only Codex check, 2026-09-25

| Metric | After M2-D | M2-A baseline, 2026-09-24 |
| --- | --- | --- |
| Complete files | 61: 53 verified 0.155/0.154/0.153 files plus 8 newly registered version files | 53, only 0.155.0-alpha.16.3 |
| Incompatible legacy 0.139–0.151 files | 238; previously 212 unsupported_version rejections | 212 |
| Events | 2,578 | 2,620 |
| Repeat scan | rescan_added=0, no duplicates | Same |
| UTC totals | calls 2,578; input_total 295,984,414; cache_read 275,255,936; output 1,502,259; total 297,486,673 | calls 2,620; total 269,817,926 |

Lower event counts reflect source retention changes. Only four 0.155.0-alpha.16.3 files
remained locally; M2-A's 53 complete scans included more files from that version before
Codex cleaned old rollouts. Newly registered versions add some events. Regression tests
and exact Pi/OMP before/after results separately verify that migration preserves parsing.

<a id="未完成项显式遗留"></a>

## Outstanding work at this stage

| Item | Status | Next step |
| --- | --- | --- |
| Dedicated Codex 0.139–0.151 token_count/last_token_usage parser | Not implemented | Verify stable individual-record identity/call boundaries, then add codex/versions implementation and per-version samples |
| Native Claude/Gemini/Qwen samples | No local data | Obtain/redact/recheck when available |
| Automatic fallback-file replay after parser upgrade | Not implemented; adapter limitations record this | Explicit rescan, or later trigger on parser_version changes |
| M6 source-page compatibility status | Not implemented | M6 |

<a id="验证产物"></a>

<a id="证据文件"></a>

## Validation files

- adapters/{codex,claude,pi,omp,gemini,qwen}/, adapters/framework.rs,
  storage/schema.rs v3 and ingest.rs.
- tests/adapter_layout_v30.rs, tests/codex_versions_v17.rs and adapter *_gaps_synthetic.rs
  updated for V17 behavior.
- tests/fixtures/codex/rollout-v{0.153.0,0.154.0-alpha.6.1,0.154.0-alpha.6.2}.*.
- Ignored build/desktop-usage-validation/: codex-versions extracted files/manifest,
  real-check-*-m2d and tools/extract-codex-versions.mjs.
