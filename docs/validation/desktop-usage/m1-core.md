# M1: Statistics core and SQLite storage

<a id="m1统计核心与-sqlite-存储"></a>

This records the initial M1 implementation. Later schema v2 and statistics/retention/query
repairs and regressions are in [M0/M1 review](m0-m1-review.md). The 72 tests below are the
initial baseline.

<a id="元信息"></a>

## Run information

| Item | Value |
| --- | --- |
| Date | 2026-09-24 |
| Environment | Windows 11 Pro 26200 x64; Rust 1.98.0; Node v24.21.0 |
| Code revision | Uncommitted working tree; desktop/src-tauri converted to a workspace |
| Design references | execution.md M1; data-contract.md statistical rules; validation.md V01–V06/V09/V14/V16 and fixed mathematical samples; scheduling.md job behavior |

<a id="命令与结果"></a>

## Commands and results

| # | Command (working directory) | Exit | Result |
| --- | --- | --- | --- |
| 1 | npm run verify (repository root) | 0 | lint:md, assets:check, svelte-check, fmt --check, clippy -D warnings, cargo test and vite build pass |
| 2 | cargo test --locked --workspace (desktop/src-tauri) | 0 | **72 passed / 0 failed**, 14 suites; real temporary SQLite files, no mocks |
| 3 | git diff --check (repository root) | 0 | No whitespace errors |

<a id="实现清单"></a>

## Implemented changes

desktop/src-tauri becomes a workspace: app crate and crates/core, named llm-usage-core,
a pure Rust library without Tauri; default-members makes root commands include core.

- domain: all eight TokenUsage fields nullable, unknown stays unknown; six RecordKind
  categories, FieldQuality, Lifecycle and integer minor-unit costs. Reject tokens above
  2^62 and confused second/millisecond timestamps.
- metrics: sum exclusive input buckets; cache_input_ratio returns no value for zero
  denominators/no samples; contradictory values produce diagnostics without adjusting
  them into range; i128 prevents overflow.
- adapters/usage_map: five independently tested mappings: Codex cache is an input subset,
  Kimi four exclusive fields, ZCode's two opposite input definitions, Copilot three added
  categories and Kilo's entirely exclusive buckets.
- calendar: jiff IANA zones; DST 23/25-hour days, cross-year ISO weeks, Sunday labels and
  retention cutoff D−1.
- storage: DDL for 18 specified tables and versioned transactional migrations;
  foreign_keys=ON/WAL/synchronous=FULL/busy_timeout=5s; reject too-new user_version;
  monotonically increasing data_revision.
- identity: namespaced event IDs; source revisions first, otherwise lifecycle order;
  equal-rank conflicts remain, without choosing MAX.
- ingest: one transaction for events, cursors, parser context, daily summaries, diagnostics,
  job progress and revision; six injected failure points.
- aggregates: source intervals, cumulative observations distinguishing resets/regressions,
  sums of mutually exclusive coverage; quotas do not become tokens.
- query: days sum into weeks/months; ratios recomputed; DISTINCT sessions across periods;
  partial-period flags, separate unknown rows included in totals; excluded attribution
  listed by reason.
- jobs: ingest_runs state machine, same-source coalescing, interrupted on restart.
- retention: seal expired days with timezone/field/source versions before removing details;
  no additions to sealed days; storage size includes WAL/backups.

<a id="测试覆盖对照"></a>

## Test coverage

| Requirement | Tests | Result |
| --- | --- | --- |
| 11 fixed mathematical samples | fixed_samples.rs: 11 hard-coded expectations | Passed |
| V01 cache inclusion | mapping_v01.rs: 8 | Passed |
| V02 repeats/corrections/disorder/conflicts | dedup_v02.rs: 6 | Passed |
| V03 call/attempt/message categories | classification_v03.rs: 3 | Passed |
| V04 midnight/leap day/cross-year week/Sunday/DST | calendar_v04.rs: 6 | Passed |
| V05 model switch/same name across providers/unknown | models_v05.rs: 3 | Passed |
| V06 day→week/month, weighted ratios, distinct, partial periods | rollup_v06.rs: 4 | Passed |
| V09 injected-failure recovery | faults_v09.rs: 3; all six points and idempotent restart replay | Passed |
| V14 retention/sealing/storage size | retention_v14.rs: 5 | Passed |
| V16 schema rejection/migration rollback | migration_v16.rs: 5 | Passed |
| Storage/jobs/boundaries | storage_jobs.rs: 14 plus 4 in src | Passed |

<a id="新锁定依赖"></a>

## Newly locked dependencies

| Dependency | Version | License | Purpose |
| --- | --- | --- | --- |
| jiff, including transitive jiff-core/static/tzdb | =0.2.37 | Unlicense OR MIT | IANA calendar/DST |
| portable-atomic family, transitive | 1.15.0 / 0.2.8 | Apache-2.0 OR MIT | jiff dependency |

<a id="失败与未执行项"></a>

## Failures and outstanding work at this stage

| Item | Status | Next step |
| --- | --- | --- |
| Initial performance measurement, M1 deliverable | Not run | At least one million events with V20 scale tests |
| Daily cost aggregation | Storage only | No cross-currency addition; panel belongs to M6 |
| aggregate_generations building→published across transactions | Not implemented | Current recomputation is atomic in one transaction; add if complex rebuilds need it |
| Pre-migration space check/consistent backup | Not implemented | Complete V15/V16 acceptance |
| Actual WSL/container attribution checks | Storage/query rules only | V25 |
| Timers/debounce/system tasks | Tables/job behavior only | M6 |
| import_manifests legacy import logic | State storage only | M2 |

<a id="验证产物"></a>

<a id="证据文件"></a>

## Validation files

- desktop/src-tauri/crates/core/: source and tests; ten integration-test files under tests/.
- desktop/src-tauri/Cargo.lock: jiff and llm-usage-core.
