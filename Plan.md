# Current implementation plan

<a id="桌面用量客户端执行计划"></a>

Version: 0.3.11. Reviewed against the current manifests, registered adapters, application
entry points, tests and design requirements on 2026-11-16. Windows 11 x64 remains the
first desktop target; Linux/macOS CI is retained.

The documentation deployment is complete, as confirmed by the user. Completed delivery
scope and the applicable requirements are consolidated in
[execution requirements](docs/design/desktop-usage/execution.md). This file contains
current work and unresolved limits, without the previous release/deployment narrative.

## Current maintenance

| Item | Latest result |
| --- | --- |
| Dependencies | Latest compatible direct npm/Cargo libraries and pnpm, synchronized locks, Jiff 1.0 API migration. Frozen installs and unified/browser checks pass. TypeScript 6 is retained because Astro/Svelte compiler consumers exclude 7. Two upstream low-severity npm findings have no compatible fix. |
| Offline models | Eight new model references, including Qwen4-Max-Preview and its official alias; nine dated API price rows for seven models. Current display override: Qoder `qmodel_latest` → `Qwen4-Max-Preview`, with raw identities/filter keys/pricing preserved. Preview remains unpriced. New/old database imports, future prices and unchanged historical estimates pass. |
| Model mappings | Versioned local JSON import and opt-in public feed refresh are implemented. Default-off downloads, three-day cache, effective intervals, retained conflicts, transactional stale-worker checks and metadata-only UI refresh preserve raw identity and prices. Core/app/browser and real Windows WebView2/IPC checks pass; see [validation](docs/validation/desktop-usage/model-aliases.md) and [requirements](docs/design/desktop-usage/model-aliases.md). |
| Qoder CN VS Code | Independent 2.6.11 SQLite metadata reader, legacy Lingma discovery, WAL handling and request observations. Full-registry/old-cursor/bad-row/ownership/rollback regressions pass. The real release executable reads two native observations idempotently, with tokens/model-call counts unknown. Windows native GUI/IPC checks pass with isolated synthetic inputs. |
| Documentation | Affected English/Chinese specifications, client guidance and comment references synchronized. Type/build/unit/browser checks pass. Exact commands/results are in [current maintenance validation](docs/validation/desktop-usage/maintenance-current.md). |

## Unimplemented or unverified scope

The project is not implemented for every requested client/interface. These limits do
not prevent the completed desktop/documentation features from being used.

| Scope | Remaining condition |
| --- | --- |
| Qoder standalone IDE / JetBrains | Independent native token readers are unimplemented: inspected distributions/transcripts have not established durable per-call token fields. Implement only after writer-bound metadata and nonempty samples establish ownership and field semantics. CN VS Code metadata support does not verify these interfaces. |
| Qoder CLI | Source-derived 1.1.76 transcript parser is implemented and regression-tested; real-client token acceptance remains unverified. Official-channel redacted zeros remain unknown. |
| Qoder CN VS Code tokens | Installed 2.6.11 chat records do not contain token usage; only native request observations are recoverable. Agent/edit modes and token-bearing formats need independent native checks before extending this reader. |
| Public model mapping feed | The static catalogue is prepared for the next documentation deployment. The two public endpoints require independent deployment/HTTPS acceptance. Project-maintained mappings do not reveal hidden provider routing or certify the model actually served. |
| Other F1 IDE candidates | Deferred where no testable installation/local usage records were found; JetBrains AI Assistant remains F1. Per-product/version limits are maintained in the [adapter matrix](docs/design/desktop-usage/adapters.md). |
| Native update acceptance | A newer public-release upgrade, NSIS updater execution while the installed application is stopped, and Linux/macOS GUI updates retain separate acceptance conditions. Implemented update code and synthetic replacement checks do not complete those cases. |

Additional DPI, complete screen-reader use, host login/logout, OS wake measurements and
macOS desktop hardware acceptance remain outside the active round. Their absence is not
a passing result. Existing native/CI results and their platform limits are indexed in
[current acceptance](docs/validation/desktop-usage/current-acceptance.md).

## Completion boundaries

Collect only local Agent records. Preserve unknowns, conflicts, first observations,
diagnostic history and sealed partitions. Real read-only extraction is already authorized
under the [implementation boundaries](docs/design/desktop-usage/implementation-readiness.md).
Temporary artifacts stay in root `build/`. Version 0.3.11 includes the mapping changes;
the user has authorized its commit, annotated changelog tag and push. The existing tag
workflow verifies all platform packages before creating a draft Release; the main-branch
documentation workflow publishes the public mapping feed. Remote completion is checked
separately from [local feature validation](docs/validation/desktop-usage/model-aliases.md).
