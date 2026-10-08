# M1a: Historical source identity and storage partitions

<a id="m1a历史来源身份与存储分区"></a>

<a id="元信息"></a>

## Run information

| Item | Value |
| --- | --- |
| Date | 2026-09-25 |
| Environment | Windows 11 x64; rustc 1.98.0; Node v24.21.0 / npm 12.0.2 |
| Code revision | Uncommitted working tree (after M2-D plus this change) |
| Design references | [Source identity and exchange rules](../../design/desktop-usage/data-contract.md#provenance); execution.md M1a; validation.md V28 |

<a id="命令与结果"></a>

## Commands and results

| # | Command (working directory) | Exit | Result |
| --- | --- | --- | --- |
| 1 | `cargo test -p llm-usage-core --test migration_v28` (desktop/src-tauri) | 0 | **7 passed / 0 failed** |
| 2 | `cargo test -p llm-usage-core` | 0 | 283 passed / 0 failed (M2-D baseline 276 + V28 7) |
| 3 | `npm run verify` (repository root) | 0 | All checks passed; see the same-day m6-desktop-core.md record |

<a id="实现清单"></a>

## Implemented changes

- **Schema v4 migration** (storage/schema.rs):
  - origin_hosts stores an opaque stable host_id and is_local; origin_host_names retains
    observed names. Renaming keeps the ID; identical names may belong to different hosts.
  - source_instances.origin_host_id defaults to the legacy_unknown namespace and has a host
    index. Sources already assigned to another host are not overwritten. Verified local
    collection may claim legacy_unknown when both the local file and locality are verified.
  - daily_usage is rebuilt by source-instance partition, with instance_id in the primary key.
    Old mixed rows retain their values in legacy_unknown without an invented split. During
    migration, unsealed days are recomputed from existing events by source; sealed rows retain
    their partition and continue to block recomputation.
  - recompute_day supports both layouts: pre-v4 databases use INSERT without partitions,
    preserving the v2 migration path.
- **Host identity API** (storage/mod.rs): ensure_local_host generates `host-<32hex>` with
  randomblob and persists settings.local_origin_host_id. Host names and IPs are not identity.
  observe_hostname records names; register_origin_host registers imported external sources.
- **Collection integration**: RunConfig.origin_host_id reaches upsert_source_instance.
- **Exchange format** (exchange.rs, llm-usage-exchange-1): ExchangeExport includes version,
  batch, optionally redacted host, source registration, events (parse_basis, revisions and
  completeness), sealed day partitions, snapshot/incremental status and explicit deletion
  declarations. build_export is read-only. decide_record_merge reuses ingest's revision,
  lifecycle and content comparisons, keeping imported and local writes consistent.

<a id="v28-用例对应"></a>

## V28 cases

| Case | Result |
| --- | --- |
| Host renaming does not double count: same ID, recorded name history | Passed |
| Same name on different hosts does not cause key collisions: host_id keys | Passed |
| Only verified local collection claims an old instance; another host's assignment is kept | Passed |
| Actual v3 migration: legacy_unknown partitions, preserved sealed values, recomputed unsealed partitions | Passed |
| Queries sum sources; deleting details preserves partition and registered source identity | Passed |
| Export fields include version/batch/source/parse_basis/redaction and survive JSON round trips | Passed |
| Merge cases: skip duplicates, replace by comparison order, add exclusive data, retain conflicts | Passed |

<a id="与合同的偏差说明"></a>

<a id="与设计的差异说明"></a>

## Differences from the design

- A corrected record with the same revision and different content produces Conflict rather
  than replacement, matching ingest: equal revisions compare content only. Replacement needs
  a higher revision. Exchange does not introduce a separate comparison rule.
- When copying a database to a new machine, callers explicitly provide identity and collection
  information. The mapping/confirmation UI for registration conflicts belongs to the later
  Merge feature, which the design schedules separately.

<a id="未完成项"></a>

## Outstanding work at this stage

| Item | Status | Next step |
| --- | --- | --- |
| Complete import/Merge write path | Not implemented, as planned | Schedule with import; decisions and format are ready |
| Multi-host historical exports, grouped by host | Not implemented | build_export currently uses this database's local-host view |

<a id="验证产物"></a>

<a id="证据文件"></a>

## Validation files

- storage/schema.rs (v4), storage/mod.rs, ingest.rs (partitioned recompute_day),
  adapters/framework.rs (origin_host_id), exchange.rs and tests/migration_v28.rs.
