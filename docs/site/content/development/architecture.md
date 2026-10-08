---
title: Architecture and data flow
description: Module boundaries, the single writer, IPC and saved validation records.
sidebar:
  order: 2
---

The desktop application uses Tauri 2, a Rust backend, SQLite and a Svelte/TypeScript frontend.
ECharts loads the needed components and renders SVG. Node is a build dependency, and the
historical Python prototype is not a runtime sidecar.

## Module map

| Location | Responsibility |
| --- | --- |
| `desktop/src-tauri/crates/core/src/adapters/` | Product-specific discovery, format/version detection, bounded parsing and field mapping. |
| `desktop/src-tauri/crates/core/` | Shared domain rules, storage, migration, deduplication, queries, archives and pricing. |
| `desktop/src-tauri/src/` | Tauri IPC, app state, scheduling, process ownership, system tasks, telemetry and credential integration. |
| `desktop/src/` | Five-page Svelte UI, typed API wrappers, local catalogs, charts and interaction logic. |
| `desktop/tests/` | Browser, executable, native, scale, installation and credential harnesses. |
| `docs/design/desktop-usage/` | Authoritative application specifications and verified source research. |
| `docs/validation/desktop-usage/` | Commands, environments, first failures and validation scope. |

## Durable collection

Discovery determines product, physical file ownership and local attribution. Adapters read
bounded records and produce normalized events or independent cumulative snapshots.
A single SQLite writer commits observations, revisions, diagnostics, fingerprints,
generations, processing positions and eligible aggregates in the same transaction.

Two source-instance readers can run concurrently. A failed or interrupted source does
not block other valid sources or advance its unconfirmed cursor/deadline. Restart recovery
and repeated reads are idempotent. Parser corrections compare complete old summaries,
retain real conflicts and reevaluate unchanged consumed files when the field rules require it.

## Frontend and IPC

The UI receives typed DTOs with quality counts, scopes, status and data revisions. Large
token values cross IPC as decimal strings, preserving values beyond JavaScript's safe
integer range. Charts can scale display values while tables/exports preserve exact amounts.

The frontend does not read arbitrary files, execute SQL, access agent credentials or spawn
shells. Backend filters and pagination are bounded. Optional HTTP reception shares the
same validated ingest boundary and remains authenticated and local.

Read the [architecture specification](/reference/design/architecture/),
[data rules](/reference/design/data-contract/) and
[query acceleration specification](/reference/design/query-acceleration/) before changing these boundaries.
