# Parser-upgrade conflict correction: validation

<a id="解析器升级冲突修复验收"></a>

Date: 2026-10-03. Windows 11 x64, Node 24.21.0, Rust 1.98, application 0.2.1.
This page retains the latest results; temporary scripts, backups and logs are under root build/conflict-fix/.

<a id="根因与当前行为"></a>

## Cause and current behavior

All 1,414 conflicts on the local trend page were on 2026-09-27 in Asia/Shanghai:
1,380 Codex and 34 Kilo Code. Read-only comparison of 13 Codex files and one Kilo
database found identical token fields. Complete old summaries differed only in parser
version; no other content differences or read errors.

Comparison covers every event field. Only a parser-basis change permits updating parser,
summary and compatibility metadata while retaining original time, usage, source revision,
identity and diagnostic history. Remove that record's false conflict and recalculate
affected unarchived daily/hourly summaries in the same transaction. Actual content
changes still follow revision/lifecycle conflict rules. Later metadata updates cannot
clear a real same-batch conflict; matching same-version repeats cannot clear real conflicts.
Archived history is unchanged. Codex rollout-4 and Kilo message-tokens-3 invoke the
existing version-upgrade mechanism to replay old cursors/positions, without clearing the
database or changing schema. See [data rules](../../design/desktop-usage/data-contract.md).

<a id="实际检查"></a>

## Actual checks

| Check | Result | Scope |
| --- | --- | --- |
| Pre-fix Codex health/dedup/Kilo incremental regressions | Exit 0; 17 passed | Existing baseline |
| parser_metadata_upgrade | Exit 0; eight passed | Old summaries including observation time, version false conflicts, retained time, all-field changes, both conflict orders, out-of-order data, rollback, consumed Codex cursors and Kilo positions |
| codex_incremental_v12 / dedup_v02 / dashboard_repair | Exit 0; 28 passed | Existing repeated final records, real conflicts, VS total upgrades and dashboard statistics |
| Consistent local-backup validation | Exit 0; 1414→0 | 13 Codex files, one Kilo database; 3,903 events and all non-conflict summary metrics unchanged; original source bytes unchanged; reread adds nothing |
| Actual statistics-database correction and independent read-only check | Exit 0; zero detail/daily-summary conflicts | Same single-writer lock as application; consistent pre-fix backup; all 3,903 events and 1,424 old update_conflict diagnostics retained, 1,414 metadata-correction diagnostics added; data revision 388→390 |
| npm run verify | Exit 0; Rust 834, frontend 20, scripts three passed | Markdown, assets, types, fmt, workspace Clippy -D warnings, specification/integration tests, frontend build; five explicit/environment-dependent tests remain ignored by default |
| npm run build:desktop | Exit 0; NSIS 3.60 MiB | Windows release and installer containing fix; not installed |
| npm run test:headless | Exit 0; 11 passed | New executable, isolated synthetic sources, final three events/75 tokens |
| npm run test:desktop -- --runs 1 | Exit 0; nine passed | New native WebView2/IPC with isolated nonempty synthetic data; two startups checked function only, without repeating the 20-run performance benchmark |
| npm run lint:md / local-reference check / git diff --check | Exit 0; 175 Markdown files, 22 changed documents/170 local references, no errors | Includes new files; temporary scripts/backups stay in root build/ outside Git |
| Read-only task-process/system-task check | Exit 0; no leftovers | Matches only isolated acceptance directories and this task's tools |

The consistent pre-correction backup is build/conflict-fix/real-1791037580446/before.sqlite,
retained locally outside Git. Flags were not bulk-cleared directly: all 1,414 records
passed complete-old-summary comparison and the application's ingest transaction
recalculated summaries. Original Agent data remained read-only. Continue collection
with this batch's new executable; old executables lack the metadata-upgrade conflict logic.

This batch made no commit, push, installation or remote CI run. Cross-platform and
complete GUI scale acceptance remain tracked in Plan.md.
