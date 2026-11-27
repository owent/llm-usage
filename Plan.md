# Current implementation plan

<a id="桌面用量客户端执行计划"></a>

Version: 0.5.4. Reviewed against the current manifests, registered adapters, application
entry points, tests and design requirements on 2026-11-17. Windows 11 x64 remains the
first desktop target; Linux/macOS CI is retained.

The documentation deployment is complete, as confirmed by the user. Completed delivery
scope and the applicable requirements are consolidated in
[execution requirements](docs/design/desktop-usage/execution.md). This file contains
current work and unresolved limits, without the previous release/deployment narrative.

## Current maintenance

| Item | Latest result |
| --- | --- |
| Dependencies | Latest compatible direct npm/Cargo libraries and pnpm, synchronized locks, Jiff 1.0 API migration. Frozen installs and unified/browser checks pass. TypeScript 6 is retained because Astro/Svelte compiler consumers exclude 7. Two upstream low-severity npm findings have no compatible fix. |
| Offline models and pricing | Qwen4-Max-Preview retains its identity and may use a labeled Qwen4-Max current reference when its own price is absent. Terminal Preview fallback and the OpenAI `codex-auto-review` → GPT-5.6 Luna API counterpart are implemented with a separate Luna snapshot. Custom provider names use exact-model official references after snapshot-priority selection; lower-priority currency conflicts no longer suppress the selected tariff. Copilot CLI provider-name/model-id strings lose one prefix for this source only before exact matching. Display mappings remain separate from pricing. Provider/channel/date/tier safeguards and saved historical estimates are retained; see [pricing requirements](docs/design/desktop-usage/pricing.md) and [current validation](docs/validation/desktop-usage/maintenance-current.md#copilot-composite-and-omp-1822). |
| oh-my-pi | 18.22.0 `model_usage` imports as auxiliary model calls with independent provider/model/Usage/stopReason, including failed calls with unknown usage. `changes.jsonl` is excluded only when its filename and `call`/`change` audit shape match. All 216 real files are active, 835 auxiliary records match independent token recomputation, and old cursors replay without conflicts. Codex oversized recovery completes nine bounded scans with 256 calls/53,711,098 tokens and healthy sources. See [validation](docs/validation/desktop-usage/maintenance-current.md#copilot-composite-and-omp-1822). |
| Model mappings | Versioned local JSON import and opt-in public feed refresh are implemented. Default-off downloads, three-day cache, effective intervals, retained conflicts, transactional stale-worker checks and metadata-only UI refresh preserve raw identity and prices. Core/app/browser and real Windows WebView2/IPC checks pass; see [validation](docs/validation/desktop-usage/model-aliases.md) and [requirements](docs/design/desktop-usage/model-aliases.md). |
| Qoder CN VS Code | Independent 2.6.11 SQLite metadata reader, legacy Lingma discovery, WAL handling and request observations. Full-registry/old-cursor/bad-row/ownership/rollback regressions pass. The real release executable reads two native observations idempotently, with tokens/model-call counts unknown. Windows native GUI/IPC checks pass with isolated synthetic inputs. |
| Zed | Reconciled OpenAI usage from the 1.30.2 local log appears in Overview, Trends and Details. Verified usage and matched calls go into the period containing the interval end. Settings can replace recoverable usage for a local source using recent days or exact hour boundaries. See [Zed validation](docs/validation/desktop-usage/agent-native-20261029.md#zed-1302-model-usage). |
| Documentation | Affected English/Chinese specifications, client guidance and comment references synchronized. Type/build/unit/browser checks pass. Exact commands/results are in [current maintenance validation](docs/validation/desktop-usage/maintenance-current.md). |

## Unimplemented or unverified scope

The project is not implemented for every requested client/interface. These limits do
not prevent the completed desktop/documentation features from being used.

| Scope | Remaining condition |
| --- | --- |
| Qoder standalone IDE / JetBrains | Independent native token readers are unimplemented: inspected distributions/transcripts have not established durable per-call token fields. Implement only after writer-bound metadata and nonempty samples establish ownership and field semantics. CN VS Code metadata support does not verify these interfaces. |
| Qoder CLI | Source-derived 1.1.76 transcript parser is implemented and regression-tested; real-client token acceptance remains unverified. Official-channel redacted zeros remain unknown. |
| Qoder CN VS Code tokens | Installed 2.6.11 chat records do not contain token usage; only native request observations are recoverable. Agent/edit modes and token-bearing formats need independent native checks before extending this reader. |
| Opaque model routes | Display mappings and the approval-review API price counterpart do not certify the model actually served. Qoder metadata still lacks token fields; model names and public prices cannot recover missing usage. |
| Other F1 IDE candidates | Deferred where no testable installation/local usage records were found; JetBrains AI Assistant remains F1. Per-product/version limits are maintained in the [adapter matrix](docs/design/desktop-usage/adapters.md). |
| Native update acceptance | A newer public-release upgrade, NSIS updater execution while the installed application is stopped, and Linux/macOS GUI updates retain separate acceptance conditions. Implemented update code and synthetic replacement checks do not complete those cases. |

Additional DPI, complete screen-reader use, host login/logout, OS wake measurements and
macOS desktop hardware acceptance remain outside the active round. Their absence is not
a passing result. Existing native/CI results and their platform limits are indexed in
[current acceptance](docs/validation/desktop-usage/current-acceptance.md).

<a id="desktop-plugin-delivery"></a>

## Desktop plugin delivery

Implementation started on 2026-11-20 from desktop 0.3.13 at `4c016d68b643`.
The [plugin design](docs/design/desktop-usage/plugins.md) owns the public behavior. Keep these milestones independently reviewable.

The first P1/P2 slice implements an offline unsigned model-name package in Settings. Installation stores immutable content in SQLite.
Matching local grants select versions and support explicit disable, replacement and removal. Query resolution integrates the enabled aliases
below legacy mappings. Full old-executable writer checks, independent feeds, typed facts and prices remain pending. P0 records toolchain
availability (Windows, Rust 1.99.1, Node 26.13.1). Worker selection and resource measurements remain open.

The first slice passed the locked Rust `plugin_packages` and `model_alias_catalog` suites (13 tests).
The tests exercise existing v11 usage, package activation across database reopen, version replacement, restore after revocation,
and peer versus manual name resolution. `npm run test:browser` exercised the Settings lifecycle with simulated IPC in Edge.
The unified check reached the desktop Rust tests. One existing OTLP lifecycle test failed with `CredReadW` access denied
inside the sandbox and passed in a targeted run with Windows credential-store access. The web build passed separately.
Native file-picker installation and package lifecycle acceptance remain open.

| Stage | Starts when | Deliverable and exit decision |
| --- | --- | --- |
| P0 | Independent of P1 | Baseline, executable-host prototype and package measurements. Decide whether to adopt the candidate runtime. |
| P1 | Independent of P0 | Local packages install disabled. The host manages identities, grants, storage upgrades and crash recovery. |
| P2 | P1 passes | Ship-capable data-only path for local model and price packs and opt-in fixed public feeds. |
| P3 | P0 approves an embedding and P1 passes | Managed worker, candidate WIT interfaces, SDK bindings, validator and conformance examples. |
| P4 | P3 passes | A new local Agent reaches the existing discovery, scheduling and ingestion paths. |
| P5 | P3 passes | A user can configure, preview and write a local report through a reference exporter. |
| P6 | P2, P4 and P5 pass | Cross-capability, upgrade, packaging and platform acceptance. Freeze SDK v1 here. |

P2 can be delivered while the runtime decision is open. Integrating an executable catalogue converter requires **both P2 and P3**.
P4 and P5 can proceed independently using the P3 test host. A data-only package or a sample that runs solely in the SDK cannot close P6.

### P0 · Establish the baseline and test the runtime

1. Prepare the 0.3.13 baseline executable, isolated source environments and one fresh database per run under `build/desktop-plugins/<run>/`.
   Capture an old database with an active source, consumed cursor, conflict, sealed partition, alias cache and existing price estimate.
   Save a consistent SQLite backup and expected row keys, known/unknown amounts, revisions and dashboard totals for P1–P6.
   Before choosing the P4 example, locate a new Agent with a documented native writer and a nonempty, minimally redacted local sample.
   A synthetic format suffices to exercise the prototype. Record native acceptance as pending until the sample is available.
2. Implement an isolated Wasmtime worker experiment outside production entry points. Exercise a bounded parse, incremental encoding, cancellation
   inside guest computation and a blocked host call. Record the engine version/features, license, security-support policy, target/toolchain support,
   SDK version and startup/termination behavior. Measure compilation and host allocations as well as guest memory.
   [Wasmtime resource limits](https://docs.wasmtime.dev/api/wasmtime/trait.ResourceLimiter.html).
3. Compare the release baseline, candidate build with plugins disabled, data-only run and executable run on the same Windows 11 x64 machine.
   Record cold-start and 1,000-record refresh P95, installer/installed bytes, ten-minute idle mean/peak, million-record first-import peak and the
   complete process tree. The current import harness generates Codex data. Retain that control and add an equivalent plugin dataset with independently
   known totals. Verify a disabled or data-only run starts no worker. Record sample counts and pre-existing target misses.
   [Resource targets](docs/design/desktop-usage/architecture.md#resource-and-performance-targets), [first-import harness](desktop/tests/native-first-import.mjs).
4. Inspect how each distribution would locate the trusted worker. Windows portable staging currently copies only the main executable.
   Compare a separate packaged worker with a dedicated child mode of the packaged executable if size or distribution is limiting.
   Verify a headless child starts without a WebView or writer connection. If either isolation or the applicable resource targets lack acceptance,
   leave executable delivery pending, record the failing measurement and continue P1/P2. [Portable packager](desktop/scripts/portable.mjs).

### P1 · Install packages and persist grants

1. Specify the v1 archive layout, canonical IDs, package digest and file digests, version and feature negotiation, signature coverage and trust-key
   lookup. Reserve built-in namespaces. Version each contribution, settings schema and cursor policy independently of the package.
   The validator and desktop installer must accept/reject the same sample bytes. Test compressed and expanded size, file count, nesting and
   platform-equivalent names at the documented limits. Identical bytes reinstall idempotently. Changed bytes under the same ID/version fail.
2. Add plugin state to the core store and desktop commands to the existing Tauri registration. Expose typed operations for inspect, install disabled,
   configure/grant, enable, disable, upgrade, roll back, remove and list status. Keep paths and native picker handles in the backend.
   Settings shows publisher verification, requested fields/roots/URLs/dataset, current digest and a permission diff. Bind grants to the package and
   contribution under the OS user's application data. Treat the dashboard's statistics-user selection as an export filter.
   [Tauri entry](desktop/src-tauri/src/main.rs), [frontend IPC](desktop/src/lib/api.ts), [Settings](desktop/src/components/SettingsPanel.svelte).
3. Start with additive, plugin-owned tables in the same SQLite database and a transactional plugin layout version. Keep legacy v11 data readable.
   Persist immutable package manifests, an active-version pointer, settings/grants with a monotonic generation, feed histories and export presets.
   Keep plugin-only prices out of the legacy unfiltered price-book query. Use separate settings keys for new state so an old settings save cannot
   discard it. Make both fresh creation and upgrade from a populated v11 database idempotent. Current `Storage::setup` rejects a nonmatching
   `user_version`; the GUI offers rebuild and headless exits. A plugin install/upgrade must never enter this recovery path.
   [Storage opening](desktop/src-tauri/crates/core/src/storage/mod.rs), [desktop recovery](desktop/src-tauri/src/app_state.rs).
4. Extract into an owned staging directory, validate before activation, then publish immutable files by digest. Only after files are durable may a
   short writer transaction switch the active pointer, settings, grants and generation. Record previous versions for explicit rollback. On restart
   verify active bytes, disable a missing/corrupt package and discard only unreferenced owned staging files. Check active digest/generation on each
   job commit. Rollback requires compatible settings/cursors and currently valid grants. A restored backup cannot silently restore authority to a
   missing, replaced or revoked package. Bind grants to a local activation receipt outside the database backup. Test restoring a pre-revocation
   backup and copying a database to a fresh profile. Both require fresh authorization when the current receipt does not match.
5. Interrupt before/after file publication and before/after DB commit. Race enable, disable, update and rollback with an in-flight job.
   Compare the P0 database before/after upgrade, reopen with the original executable on a disposable copy, perform a normal old-writer scan and
   reopen with the new host. Verify history, grants, disabled-feed exclusion, old source behavior and estimates. Exercise uninstall/reinstall,
   backup/restore, retention and Clear statistics. Clearing usage also clears its plugin-derived working state and cost bindings, retaining package
   preferences as settings. If an old writer can corrupt history or apply an inactive price, block this storage layout and review an explicit
   forward migration and downgrade recovery before shipping. [Database backup](desktop/src-tauri/src/db_backup.rs), [clear operations](desktop/src-tauri/src/commands.rs).

### P2 · Add model and price feeds without executing code

1. Start with a local data-only package containing one dated model fact, one alias and one exact route tariff. Implement schema validation,
   preview and activation via P1. For each contribution persist origin, revision, raw digest, publication/retrieval times and effective interval.
   Identify feed history by plugin/contribution/approved source. The current alias state has one version map and one successful raw cache.
   Read old candidates, origins and cache losslessly. Separate per-source histories and include the active metadata generation in query cache keys.
   Exercise Settings import and model details, including context-window units, absent fields and source/date display.
   [Alias storage and resolution](desktop/src-tauri/crates/core/src/model_aliases.rs), [existing alias tests](desktop/src-tauri/crates/core/tests/model_alias_catalog.rs).
2. Normalize plugin tariffs in a separate path. Carry explicit origin/rank into price matching. The existing SQL gives all other sources the
   same rank after manual and seed, and the engine can take the first eligible row. Group equally applicable plugin feeds before selecting a row.
   Equal tariffs can share a result with both provenances. Different rates or currencies at that rank stay ambiguous. Recency orders revisions
   within one feed. Test both load orders and cold restart. Reject official flags and substitution requests. Validate half-open dates, integer
   scaling, currency, channel, missing components and thresholds. Confirm a higher-ranked applicable tariff still wins.
   [Price-book loader](desktop/src-tauri/crates/core/src/storage/pricing.rs), [selection](desktop/src-tauri/crates/core/src/pricing.rs).
3. Keep immutable price rows and the basis of already saved estimates. Daily recomputation can revisit earlier events when new usage arrives.
   For new plugin-priced observations bind the event identity/revision to its selected price basis and exact components before rounding.
   Reuse that binding on ordinary same-day rebuilds. Preserve legacy saved partitions at their available granularity.
   Never manufacture per-event bindings from a legacy daily sum. Honor explicit recalculation and actual event corrections under the existing
   [pricing rules](docs/design/desktop-usage/pricing.md). Test an event at price A, a refresh to B, an unrelated same-day event, disable/remove,
   restart and retention. The original saved estimate keeps A, the current-price view can use B while enabled, and no currency totals are merged.
4. Add per-feed manual and TTL-based refresh only after offline import works. Fetch an approved exact public HTTPS URL through the host transport.
   Enforce address restrictions at connection time, response/decompression/time limits and redirect policy. Preserve the built-in URL restrictions.
   Inject transport, clock and resolver in tests so success, expiry and denied destinations are reproducible. The plugin cannot supply a request.
   Download and validate before the writer transaction. Compare source, generation and prior revision inside it. A stale worker loses to newer
   settings/imports. A failed or corrupt download preserves the last valid cache. Disconnect and restart to verify both feeds still work offline.

### P3 · Run executable contributions

1. Define the four WIT worlds, typed records, optional 64-bit counts, bounded result pages and stable error codes. Version the worker IPC handshake
   separately from the public WIT interface. Bind each operation to a job ID, contribution, active digest, generation, deadline and granted handles.
   Specify framing, maximum message size, backpressure, cancellation acknowledgement and terminal success/failure. Reject extra or out-of-order
   frames, unknown required features and stale handles before performing I/O. Keep internal Rust trait objects and database connections host-side.
2. Launch only the host-owned executable from its verified package location. Start one fresh component instance per contribution/operation with
   only that world's imports. Bound compilation, guest resources, host I/O, output and whole-worker lifetime. Put the measured limits and units
   in one host policy shared by runtime and conformance tests. For each bound, test just below, at and above it. Terminate and reap a stalled worker,
   revoke its handles and free the slot. The app and other sources remain usable. Recheck generation before accepting any side effect. Return
   typed diagnostics without free text from the guest. Check permission denials, slow input/output, interruption, crash and restart.
3. Deliver the WIT files, bindings, manifest validator, sample packages and a host conformance harness buildable without private app crates.
   Run an independently built sample on the release-mode host and an earlier compatible fixture on a later host. Compare validator and runtime
   decisions for missing imports, unsupported majors and boundary sizes. Cover a package declaring multiple contributions with disjoint grants.
   Once P2 is ready, run a converter from approved feed bytes to typed candidates and verify it cannot access usage or an export job.

### P4 · Register a local Agent and commit one complete page

1. Adapt static `adapter_id` and `agent` lifetimes at the registry/bridge boundary. Preserve every legacy ID, instance key, cursor and hook.
   Use the same enabled-registry snapshot for discovery, conflicts, both parsing slots, manual refresh and headless collection. Keep existing
   built-in repair routes scoped. The host probes declared files under a granted root and binds handles to the opened physical identity.
   Resolve candidates before dispatch. Preserve an existing valid owner, prefer a verified built-in on a new file and hold an ambiguous plugin
   match until the user selects a candidate. Persist that selection with its scope. Recheck ownership and generation inside the committing transaction.
   [Adapter framework](desktop/src-tauri/crates/core/src/adapters/framework.rs), [routing](desktop/src-tauri/crates/core/src/adapters/routing.rs),
   [scanner](desktop/src-tauri/src/scanner.rs).
2. Implement bounded JSON/JSONL framing and read-only SQLite column projections with stable typed keyset pagination. Bind proposed cursors to
   host-observed complete boundaries. Return pending on a partial tail and differentiate invalid rows, exhaustion and EOF. Test WAL-backed input,
   same-timestamp rows, a malformed row between valid rows, rotation/truncation and path replacement. Verify source DB/WAL integrity and sidecar
   policy. Validate host/user/source attribution, native version, event kind and token semantics against the reference writer.
3. Route plugin results through the existing single writer. `claim_file` performs a preliminary ownership check and some repair branches commit
   separately. `quota_history::record` also opens its own transaction. Factor a transaction-taking quota helper, retaining the existing wrapper.
   Namespace plugin quota series by their verified owner. In the accepted page transaction validate the active grant, owner, fingerprint/generation
   and expected old cursor, then persist events, aggregates, quota rows, cursor and applicable usage summaries together. Preserve the existing
   post-scan cost refresh and its recovery. Inject failure after quota/usage writes and before cursor commit. Reopen and retry the same page.
   Expect zero partial writes, identical native keys, one quota observation and one final cursor advance.
   [Ingestion](desktop/src-tauri/crates/core/src/ingest.rs), [quota storage](desktop/src-tauri/crates/core/src/quota_history.rs).
4. Compare two full-registry scans of the redacted sample with an independent field-by-field expectation. Include failed calls with unknown usage,
   cumulative/observation-only records, zero defaults and per-record versions. Use synthetic cases for formats the real sample does not cover.
   Exercise file and parent-root grants, two simultaneous claims, unchanged-file parser upgrade, rollback with an incompatible cursor, unavailable
   original, inherited conflicts and sealed partitions. Automatic pause, a merged trigger or a failed job cannot advance an unvisited deadline.
   Verify GUI, manual CLI and scheduled headless runs, including a restart. [Ownership regressions](desktop/src-tauri/crates/core/tests/source_routing.rs).

### P5 · Configure an exporter and publish one file

1. Register format descriptors beside the three native export choices. Add named presets with version, dataset, required/optional fields, timezone,
   date range and user/host/source/model filters. Validate the same typed request in preview and execution. Reject a missing grant, unsupported filter
   or empty required selection. Show row/partition coverage, excluded or expired detail, currency and exact destination before writing.
   Plugin destinations use a backend-issued one-time selection for the exact file. The native UI currently passes the parent directory.
   [Settings export flow](desktop/src/components/SettingsPanel.svelte), [existing command](desktop/src-tauri/src/commands.rs).
2. Return a preview identifier bound to the normalized request, dataset/metadata revisions, grant generation and output selection. Do not hold a
   read transaction open while waiting for the user. At execution open a dedicated read-only connection, start a transaction, perform its first
   read and compare the stamps there. A mismatch requires a fresh preview. Use a stable unique page key for that one snapshot. The existing
   `read_conn` can fall back to a locked writer. A plugin export must return a retryable error if its independent reader is unavailable.
   Limit snapshot duration and WAL growth, cancel and close an overdue reader, and discard the incomplete output.
   [Read connections](desktop/src-tauri/src/app_state.rs), [SQLite WAL and checkpoints](https://www.sqlite.org/wal.html).
3. Send only granted, filtered fields to the encoder with bounded backpressure. Keep known/unknown values, currencies and detail/archive coverage.
   Test with two users and hosts so an omitted filter cannot accidentally export all records. Compare every decoded row of a daily JSON Lines
   file with an independently computed expected result. Include DST, same-key sort ties, absent token fields, concurrent scans and retention.
   For normalized detail over sealed days expose the coverage gap. Compare native exchange round trips separately.
   [Detail exchange tests](desktop/src-tauri/crates/core/tests/detail_exchange.rs).
4. Write to an exclusively created sibling file. Journal its ownership and selected target identity. Serialize final grant/target/overwrite checks
   and same-volume publication against revocation, and report success only after publication. A cancellation that wins leaves the old target intact.
   A crash after publication must reconcile the journal before retry or cleanup. Never remove an unrelated file at a reused path.
   Exercise preexisting output, changed destination, disk full, crash on either side of publication, oversized output and a slow encoder.
   Check the host cell writer's quoting and formula handling when a format declares spreadsheet-delimited output.

### P6 · Verify installation, upgrades and end-user results

1. Build the P0 worker arrangement into release packages and integrity checks for each platform/architecture. If separate, include the worker in
   Windows portable staging, installer and managed update entries, and in the macOS app/Linux AppDir layouts. Verify worker version/architecture
   after extraction, install, update and rollback, including a mismatched or missing worker. Protect user plugin packages and app data across updates.
   Follow the existing six portable targets and Windows x64 NSIS scope. Report actual GUI checks separately from headless or cross-build checks.
   [Release matrix](.github/workflows/ci.yml), [portable update ownership](desktop/src-tauri/src/update_package.rs).
2. In one clean Windows installation use Settings to install/enable the four contributions. Collect the new Agent twice, inspect a sourced model
   fact, compare a known tariff with a hand calculation, then create, preview and export a filtered preset. Restart offline and repeat. Disable
   the plugin during a job, try an incompatible upgrade, roll back and remove the package. Assert privileges stop, collected usage and historical
   prices remain queryable, and built-in collection/export still work. Freeze SDK v1 only after the independent package passes this path.
3. Extend the existing harnesses with explicit plugin assertions. Run the following from the repository root on an isolated host, in order where
   outputs are consumed. A green pre-existing suite counts only for the behaviors it exercises.

   | Run | Required observation |
   | --- | --- |
   | `cargo test --manifest-path desktop/src-tauri/Cargo.toml --locked -p llm-usage-core --test source_routing --test model_alias_catalog --test pricing_archives --test detail_exchange` | Preserve legacy results and cover the P2/P4/P5 regressions. Include the new plugin-specific suites in the workspace default test members. |
   | `npm run verify` and `npm run test:browser` | Core, desktop, SDK/worker checks as integrated, frontend types, and English/Chinese Settings interactions. Browser IPC is simulated. |
   | `npm run build:desktop`, then `npm run test:headless`, `npm run test:desktop`, `npm run test:import` | Exercise the release executable, real SQLite, WebView2/IPC, bounded scans and whole-process peaks. Compare the built-in control and plugin workload with P0. |
   | `npm run test:update:windows` and `npm run test:install:windows -- --previous-installer <path> --previous-version <version> --installer <path>` | On a disposable Windows user, verify portable replacement and actual old/new NSIS install, upgrade, rollback and uninstall. Check the executable/worker pair and preserved data after each restart. |
   | `npm run check:docs`, `npm run test:docs`, `npm run build:docs` | Ship matching English/Chinese SDK and user instructions. Record frozen interface versions, grant examples, supported field provenance and recovery behavior. |

Use this synthetic example as one shared, independently calculated acceptance case for P2, P4, P5 and P6. Keep the real Agent sample in P4.

| Action on an isolated UTC day | Expected result |
| --- | --- |
| Under user A and host A, write one final call for raw model `demo-m1` with 1,000,000 uncached input and 200,000 output tokens. The fixture explicitly reports zero cache reads/writes. Write a second failed call with no token observations. Add a sentinel call under user B/host B. | Filtering A/A yields two calls, 1,000,000 known input and 200,000 known output. One call has unknown tokens. A second scan and an interrupted-page retry produce the same result. |
| Enable estimation and configure an exact standard USD route before first collection. Feed A supplies USD 2/input and USD 5/output per million tokens and a dated display name. | The priced call costs USD 3.00. The failed call has no known amount. The model details show the sourced label and the raw key is still `demo-m1`. |
| Publish a revision of the same feed applicable to the current reference date with rates of USD 4 and USD 10. Refresh, restart and append unrelated same-day usage on another model. | The original observation keeps its USD 3.00 estimate. Its current reference is USD 6.00 while the revised feed is enabled. Disabling it removes its eligibility for new lookups. |
| Run a detail JSON Lines preset for A/A and raw model `demo-m1`. Independently decode the file, then cancel a second export to the same destination. | Exactly two selected rows. The failed row has null token fields. The other user's sentinel and the unrelated model are absent. The canceled export leaves the first file byte-for-byte intact. |

For each stage, use a separate directory and database per run, including isolated source home/environment variables. Keep disposable payloads,
logs and reports under `build/desktop-plugins/`. Record exact revisions, package/API digests, platform, commands, exit codes, expected vs observed
values and outstanding native coverage using the [validation record template](docs/validation/desktop-usage/TEMPLATE.md).
A failed data-integrity, permission or recovery check keeps its stage open. Preserve the first failure and verify the fix with the same input.

## Completion boundaries

Collect only local Agent records. Preserve unknowns, conflicts, first observations,
diagnostic history and sealed partitions. Real read-only extraction is already authorized
under the [implementation boundaries](docs/design/desktop-usage/implementation-readiness.md).
Temporary artifacts stay in root `build/`. Version 0.5.0 includes offline model name plugins,
Copilot CLI reference pricing, oh-my-pi and Qoder collection fixes, and dashboard layout updates.
Version 0.5.1 adds Antigravity desktop generation recovery, dependency updates and the
November 23 offline model catalogue. Version 0.5.2 adds Antigravity pending-generation
retry bounds, GitHub Pages build-status retries, and Overview Zed thread interval visibility.
Version 0.5.3 integrates verified Zed model usage into charts and Details and
removes the separate Zed thread panel from Overview.
Version 0.5.4 adds forced Zed recollection for recent calendar days or exact hour ranges.
The tag workflow verifies all six platform/architecture packages and
creates a draft; publish only after matching its source, notes and uploaded package sizes/digests.
Both public mapping endpoints and real Windows automatic HTTPS refresh/restart were verified
for 0.3.11. Current pricing checks are recorded in [maintenance validation](docs/validation/desktop-usage/maintenance-current.md#preview-and-approval-review-pricing);
tagged CI and public-release acceptance are checked separately.
