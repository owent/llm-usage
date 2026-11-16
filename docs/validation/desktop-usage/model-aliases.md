# Model mapping validation

Reviewed: 2026-11-16. Feature checks used the working tree based on 0.3.10. Release
preparation synchronizes all application version sources/locks to 0.3.11; the user has
authorized commit, an annotated changelog tag and push. The executable digest and tests
below precede that version-only bump; tagged CI verifies 0.3.11 packages separately.
Environment: Windows 11 x64, Node 26.13.0, Rust 1.99.1 and installed
Edge/WebView2. Temporary logs, isolated databases and probes are under
`build/model-aliases-20261116/`. The [mapping specification](../../design/desktop-usage/model-aliases.md)
defines source, interval, conflict and privacy requirements.

## Implemented behavior

- A versioned metadata setting retains candidates, exact scopes, effective intervals and SHA-256
  content identities. Imports and successful raw cache updates share one SQLite transaction.
  No database schema change or data reset is required.
- Local JSON import and default-off online refresh are independent of pricing. The default cache
  lifetime is three days, configurable from 1 to 365. Downloads use only the two specified HTTPS
  URLs, with no redirects, credentials or local usage. Failures retain the successful cache.
- Complete-state tokens and monotonic settings timestamps reject stale downloads, including
  manual imports, other writers and off/on settings changes during download.
- Backend labels use exact Agent/provider scopes and the retained usage query snapshot. Mixed
  labels, gaps, expired newer mappings and conflicts retain raw identities. Details prefer an
  already persisted native canonical model. Raw keys, counts, unknowns and pricing remain separate.
- Settings support imports, refresh, TTL and status in ten languages. Metadata revisions update
  tables, filters and chart labels without changing usage revision or filter/grouping values.
  The Qoder fallback remains the user-authorized `qmodel_latest` → `Qwen4-Max-Preview` override.

## Checks

Commands ran from the repository root unless a working directory is stated.

| Command/check | Result and scope |
| --- | --- |
| `cargo test --locked --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core --test model_alias_catalog` | 10 passed: bounded validation, Agent/provider/suffix scope, temporal conflicts/expiry, idempotence, rollback, stale/cross-process writers, read-only cache invalidation, accelerated/hourly/archived queries, native canonical priority and the public seed's explicit override basis. |
| `cargo test --locked --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-desktop model_alias_refresh` | Four passed: old settings/defaults, TTL, fixed-source fallback, retained first errors/cache and monotonic settings rejection. HTTP responses are injected; this is not public deployment acceptance. |
| `cargo clippy --manifest-path desktop/src-tauri/Cargo.toml --workspace --all-targets --locked -- -D warnings` | Exit 0. |
| `cargo test --manifest-path desktop/src-tauri/Cargo.toml --locked` | 1,181 passed, 10 ignored, zero failed with normal Windows credential-store access and isolated temporary paths. Existing ignored checks remain separate. |
| `npm run test:ui` / `npm run check` | 34 passed; Svelte reports zero errors/warnings. Server-provided raw conflict labels take precedence over bundled fallback labels. |
| `npm run test:browser` | Exit 0, installed Edge with synthetic IPC: default-off settings, TTL draft/save validation, import errors/success, failed download, metadata-only polling, raw filter/series keys and live detail labels. Existing interaction assertions remain enabled. |
| `node node_modules/@tauri-apps/cli/tauri.js build --no-bundle` from `desktop/` | Exit 0; frontend and Windows release executable built. SHA-256: `fea71f04ecc6c480e687f9817db7126de43f65a0abffd2f3f90de346c48b137a`. No installer/package acceptance is inferred. |
| `node build/model-aliases-20261116/native-check.mjs` | Exit 0, real WebView2/Tauri IPC and independent data/source/temp roots. Verified default-off state, native collection, JSON imports/idempotence, metadata-only UI refresh, raw chart/detail identities, unchanged complete usage rows/revision/totals, conflict rejection, disabled network refresh and restart persistence. Uses synthetic Codex records; it does not certify a new Qoder-native format. |
| `npm run lint:md`, `npm run check:docs`, `npm run test:docs`, `npm run build:docs` | Exit 0: 483 Markdown files, 215 reviewed repository pairs/21 guide pairs, Astro zero errors/warnings, 41 documentation tests, 1,427 built pages and checked publication links/markers. The generated mapping feed matches the 396-byte source exactly. |

## Failures and corrections

The first restricted full Rust run failed in the existing receiver test because `CredReadW`
returned `0x80070005`, producing `credential_store_unavailable` before the expected occupied-port
branch. The same final source passes with normal Windows credential-store access. Both logs
are retained as `rust-full.log` and `rust-full-native.log`; assertions were preserved.

The browser mapping test found that an update before the first metadata status poll could leave
an old label visible. Polling now compares the actual loaded summary revision with backend status.
Another first-run assertion counted cost recomputations performed by earlier tests; the mapping
check now compares against its entry count and confirms that it adds none. The initial logs
remain alongside the passing third browser run.

## Remaining limits

The static public catalogue is prepared at `docs/site/public/models/aliases.json`; its two live
endpoints require the corresponding documentation/repository publication and independent HTTPS
acceptance. The authorized main-branch push triggers documentation publication; actual
HTTPS and deployment completion remain separate checks. There is no claimed official Qoder key-to-model API.
Maintainers must check sources and update that feed. A display name cannot identify hidden
routing, provider fallback or historical serving models. No new Agent-native reader, account
API, additional platform GUI or package lifecycle acceptance is established by these checks.
