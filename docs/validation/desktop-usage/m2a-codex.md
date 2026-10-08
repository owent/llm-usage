# M2-A: Adapter framework and Codex adapter

<a id="m2-a适配器框架与-codex-适配器"></a>

<a id="元信息"></a>

## Run information

| Item | Value |
| --- | --- |
| Date | 2026-09-24 |
| Environment | Windows 11 Pro 26200 x64; Rust 1.98.0; Node v24.21.0 |
| Code revision | Uncommitted working tree |
| Design references | execution.md M2; adapters.md delivery requirements/samples; architecture.md incremental strategy; validation.md V07/V12/V17 |

<a id="命令与结果"></a>

## Commands and results

| # | Command (working directory) | Exit | Result |
| --- | --- | --- | --- |
| 1 | npm run verify (root) | 0 | lint/assets/svelte-check/fmt/clippy -D warnings/cargo test/vite build pass |
| 2 | cargo test --locked --workspace (desktop/src-tauri) | 0 | **135 passed / 0 failed**, M1's 72 plus new tests |
| 3 | cargo run -p llm-usage-core --example real_verify_codex -- ~/.codex build/desktop-usage-validation/real-check | 0 | Read-only local checks; permitted-field statistics below |

<a id="实现清单"></a>

## Implemented changes

- adapters/framework.rs: SourceAdapter discover→detect→scan→capability; scan flow includes
  generation rescans, skipping unchanged files, rejecting unrecognized data and restricted
  diagnostics. Reconcile per-call totals with final snapshots and carried compaction records.
- adapters/jsonl.rs: all V07 boundaries: no cursor advance for partial lines, UTF-8 across
  chunks, controlled errors above 8 MiB per line, BOM, isolated bad rows without content,
  generation redetection for same-length replacement/truncation/renaming.
- adapters/codex.rs: Codex 0.155.0-alpha.16.3. Per-call token_usage_record becomes model_call
  with resp:{response_id}, falling back to seq:{session}:line. token_count cumulative formats
  remain separate. Deduplicate/reconcile compaction carry records; turn_context determines
  models, otherwise unknown; codex_vscode maps to host_application=vscode; subagent categories
  mapped. source_instances stores capabilities.
- ingest.rs repeated-final fix: same key/content differing only in time is Keep, avoiding
  false conflicts; real same-revision differences still retain conflict diagnostics.

<a id="测试新增-63-个全部真实临时文件sqlite"></a>

## Tests: 63 new cases, all real temporary files/SQLite

- codex_contract.rs: end-to-end expectations for three M0 real samples: one call, 49 calls,
  multimodel compaction; capabilities/discovery/instances/storage.
- codex_gaps_synthetic.rs: clearly labeled synthetic samples: repeated final, subagent,
  positive cache writes, usage-free tool/user messages, missing response_id fallback;
  V17 rejects unknown versions/formats.
- codex_incremental_v12.rs: appends, partial lines, truncation/same-length replacement/rename,
  segmented reads under scan limits, no new records on rescan, retain existing conflicting final.
- jsonl_v07.rs: each V07 boundary; review_regressions.rs: prior-failure regressions.

<a id="本机真实只读核对codex265-个-rollout-文件"></a>

## Read-only local checks: ~/.codex, 265 rollout files

| Metric | Observed result |
| --- | --- |
| Fully scanned files | 53; another 6 degraded for empty files/abnormal metadata |
| Rejected | 212 other-version files: 0.154.0-alpha.6.2×89, 0.155.0-alpha.16×18 and 12+ versions including 0.146/147/149/150/151/153; per-version samples required |
| Lines/records/events | 23,059 lines → 2,620 model-call events |
| Snapshot comparison | matched 48 / mismatch 5 / no_snapshot 0; final snapshot exceeds per-call total by 2.9M–31.5M tokens, consistent with source retention truncation or cross-file sessions; reconcile_mismatch retained, no silent addition |
| Rescan | rescan_added=0; no duplicates |
| UTC totals | calls=2620; input_total 268,545,932; cache_read 257,653,888, about 95.9%; output 1,271,994; total 269,817,926; 3 models |

Diagnostics: unsupported_version 212, snapshot_out_of_order 76, unexpected_session_meta 6,
reconcile_mismatch 5, unknown_record_type 4, usage_shape_deviation 1. No bodies/paths leaked.
examples/real_verify_codex.rs restricts fields; temporary database is in ignored
build/desktop-usage-validation/real-check/.

<a id="中断后收尾主会话完成"></a>

## Completion after interruption

The M2-A subagent stopped at its quota. The main session repaired/validated its implementation:
capability cost assertion, repeated-final Keep logic, same-length replacement sample (original
corrupted cli_version and conflicted with rejection rules; replaced by whole-line swapping),
six Clippy issues, formatting and --force-local for bundle-report tar with Windows drive colons.

<a id="未完成项"></a>

## Outstanding work at this stage

| Item | Status | Next step |
| --- | --- | --- |
| Other Codex 0.146–0.155 versions | Partial; [M2-D](m2d-layout-versions.md) verifies 0.153.0/0.154.0-alpha.6.1/6.2; 0.139–0.151 lack per-call format and need dedicated implementation; unlisted versions use latest_fallback attempts | Verify token_count/last_token_usage identity before extending |
| OTel intake | Not implemented | M5 |
| Actual GUI sources page | Not implemented | M6 |

<a id="验证产物"></a>

<a id="证据文件"></a>

## Validation files

- desktop/src-tauri/crates/core/src/adapters/: framework/jsonl/codex; tests/: codex_*,
  jsonl_v07, review_regressions and fixtures/codex/.
- Ignored build/desktop-usage-validation/real-check/ contains real-check.sqlite.
