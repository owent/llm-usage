# User-installed desktop plugins

Design proposal for review, based on desktop 0.3.13 at `4c016d68b643`, inspected on 2026-11-20.
The proposed host would let a user install a local package, enable individual contributions and configure them in Settings.
The implementation stages and acceptance criteria are in [Plan.md](../../../Plan.md#desktop-plugin-delivery).

## Integration points in the current application

| Area | What the source implements | Consequence for the design |
| --- | --- | --- |
| Collection | The Rust registry constructs 41 built-in adapters. The desktop scanner uses the same registry for manual and background collection. Two parsing slots share a writer. The framework commits events, aggregates and checkpoints together. [Registry](../../../desktop/src-tauri/crates/core/src/adapters/mod.rs), [scanner](../../../desktop/src-tauri/src/scanner.rs), [framework](../../../desktop/src-tauri/crates/core/src/adapters/framework.rs) | Keep a single discovery and ingestion path. The internal trait returns static IDs, takes filesystem paths and even offers an archive hook with storage access. A public plugin interface needs owned identities and narrower inputs. |
| Model information | The core stores dated display-name candidates and a cache in one settings value. Its version map is keyed by rule version, and the persisted origin check recognizes manual and online imports. The desktop refresh worker knows two URLs for the same catalogue. [Core](../../../desktop/src-tauri/crates/core/src/model_aliases.rs), [refresh](../../../desktop/src-tauri/src/model_alias_refresh.rs) | Concurrent feeds need separate identities, caches and version histories. A context window or a modality needs its own field and provenance. A display name is insufficient. |
| Prices | Seed files, local snapshots and the optional models.dev download feed one price engine. Accepted source types are `manual`, `seed` and `community`. Omitting `official_vendor` currently defaults to true for a non-manual snapshot. [Parser and price selection](../../../desktop/src-tauri/crates/core/src/pricing.rs), [storage](../../../desktop/src-tauri/crates/core/src/storage/pricing.rs), [refresh](../../../desktop/src-tauri/src/price_refresh.rs) | Introduce an explicit plugin origin and ranking. Passing a third-party document directly through the community or manual import route could grant it unintended authority. |
| Export | Settings and typed IPC enumerate `summary-csv`, `exchange` and `details`. CSV uses the summary query. The exchange branches select source instances by user/host and construct their own history range. The UI passes the chosen file's parent directory and the backend generates the name. [Settings](../../../desktop/src/components/SettingsPanel.svelte), [IPC](../../../desktop/src/lib/api.ts), [export command](../../../desktop/src-tauri/src/commands.rs) | Give each contributed format a descriptor, an explicit dataset and filters. Bind the job to the exact chosen output file and preview the scope used by the writer. |

## Package and runtime choice

Use a versioned manifest for every plugin. A package containing model facts or tariffs in the host schema needs only data.
An Agent reader, a converter for a different feed format, or a custom export encoder can contribute a WebAssembly component.
Use WIT, the WebAssembly Interface Type language, to describe typed inputs, outputs and resource handles.
Keep the released SDK independent of the application's Rust traits, Serde structures, SQLite schema and Tauri commands.

| Option considered | Decision for this application |
| --- | --- |
| Manifest and structured data | First delivery for model and price sources. Installation and validation can work offline. General incremental parsers and custom encoders need computation. |
| WIT components in a host-owned worker | Preferred for executable contributions. Load a fresh instance for one contribution and operation. Start the worker on demand so ordinary startup and built-in collection do not load the engine. Wasmtime is the candidate to measure. |
| Native libraries, Tauri Rust plugins, user executables or Node/Python scripts | Suitable for code shipped and reviewed with an application. Installing arbitrary native code would give it the process user's authority. A subprocess alone provides crash isolation and still needs permission enforcement. Bundling another language runtime also changes packaging. |
| WebView extensions | The main window already calls privileged application commands. Loading plugin UI there would expand its authority and would require a different execution path for headless scans. Render settings from typed descriptors in the existing UI. |

Tauri documents plugins as Rust crates and assigns permissions to windows and WebViews. WIT defines imports, exports and resource handles.
Wasmtime publishes a working multi-language plugin example. These support the interface choice. Package cost and platform support in this application
still require measurement. [Tauri plugins](https://v2.tauri.app/develop/plugins/), [Tauri capabilities](https://v2.tauri.app/security/capabilities/),
[WIT reference](https://component-model.bytecodealliance.org/design/wit.html), [Wasmtime plugin example](https://docs.wasmtime.dev/wasip2-plugins.html).

### Installation, grants and compatibility

- Use a local archive with a UTF-8 manifest, declared data assets and optional component files. Record package format, namespaced plugin ID, publisher,
  package version, supported host API range, contribution IDs/kinds, entry paths and SHA-256 digests, requested inputs, settings schema and license.
  Keep package version, API version, parser policy/cursor version, native Agent version and feed revision independent.
  Each contribution selects its interface major and required features. Check both the manifest and the component's actual imports/exports.
  Reject unsupported requirements before granting access. Test older v1 packages on later v1 hosts. Breaking changes get a new major.
- Settings previews identity, version, provenance, requested roots/fields, exact feed URLs and export datasets. Install disabled.
  A personally authored unsigned package can be enabled explicitly as unverified. A digest identifies bytes. A verified publisher label requires
  a signature bound to a trusted key. Reserve built-in IDs and bind update continuity to the trusted key or explicit local replacement approval.
  First delivery uses local installation and manual updates.
- Validate the entire archive before activation. Limit compressed and expanded size, member count and depth. Reject escaping paths, links, duplicate
  or platform-colliding names, unknown required interfaces, unexpected files and incompatible versions. Load immutable, verified content by digest.
  Reinstalling identical bytes is idempotent. Changed contents under the same ID and version require a newly versioned package.
  A staged update keeps the last usable version until its new settings, grants and parser state are compatible. Added permissions require fresh consent.
- Store the active package digest, enabled contributions, grants, typed setting values and generation under the application's data directory.
  Use one host transaction for the active references. A crash leaves either the previous or the verified new selection usable.
  Updating, disabling or removing a plugin invalidates outstanding operations. Check generation and grants at commit or file publication.
  Retain collected usage, checkpoints, candidates, diagnostic history and original estimates. Exclude disabled feeds from new lookups.
  A rollback uses a compatible checkpoint or replays from native input with existing identities. Surface missing input as limited coverage.
- No directory auto-execution, install scripts or plugin-rendered HTML. Settings accept bounded text, enums, numbers and host-owned selectors.
  Handle real credentials only through an independently designed integration. Keep real roots, native handles and user selections in the host.

### Execution and resource limits

Give each executable contribution its own WIT world. Link only that world's host imports. No ambient WASI filesystem, sockets, environment,
process APIs, application DB or inherited stdio. A worker receives a bounded message stream over a private host channel. Handles expire at job end
and are bound to the plugin digest, contribution, grant generation and operation. Even contributions from the same package cannot share live
instances, handles, scratch storage or a guest-selected network request.

The host bounds module compilation, guest memory/tables/instances, input and output bytes, rows, calls and elapsed time. Meter guest computation,
cancel host I/O separately, and terminate a stuck worker. Size-check IPC frames before allocation. Keep the writer lock outside plugin execution.
Record bounded error codes and counts. Exclude raw records, arbitrary guest logs and crash dumps containing inputs.
Wasmtime distinguishes guest resource limits from host allocations and documents fuel and epoch interruption. Measure whole-process peaks
and host-call timeouts. [Security model](https://docs.wasmtime.dev/security.html),
[resource limits](https://docs.wasmtime.dev/api/wasmtime/trait.ResourceLimiter.html),
[execution interruption](https://docs.wasmtime.dev/examples-interrupting-wasm.html).

## Contribution interfaces

| Contribution | Proposed input → output | Authority held by the host |
| --- | --- | --- |
| Agent collector | `probe` receives bounded native format evidence. `scan-page` receives granted source records, a resume position and limits. It returns observations, aggregate candidates, diagnostic codes and the proposed next position. | Local provenance, file ownership, source/user identity, record keys, validation, cursor commit and scheduling. |
| Model catalogue | Bundled data, a selected catalogue file or bytes from a granted public feed → dated alias rules and optional typed model facts. | Scope, provenance, candidate history, conflict resolution and display. |
| Price source | The same restricted feed inputs → dated tariff candidates. | Snapshot identity, source rank, official status, unit validation, cost calculation and historical amounts. |
| Exporter | `begin`, `encode-page`, `finish` consume one selected, normalized dataset and emit bounded bytes. | Filter evaluation, redaction, consistent read transaction, destination and final file publication. |

### Agent collection

The user associates the collector with a specific local root. Declare relative discovery patterns, expected file formats and read fields.
The host enumerates within the grant and checks the actual opened file identity, including link/reparse points and path aliases.
Limit the initial broker to bounded JSON/JSONL records and SQLite projections over declared tables/columns with typed keyset pagination.
The host frames records, projects the necessary native fields and regenerates queries from validated identifiers and bound values.
Plugins receive neither raw SQLite connections nor general SQL. Specify separate reviewed readers for other transports.
Verify native structure and local Agent ownership before claiming any file. A suffix match, a cloud copy or an empty session cannot establish support.

Keep the built-in registry for native adapters and place a host-side bridge for enabled contributions in the same scanner.
Use owned or borrowed runtime IDs without leaking permanent strings. Preserve every built-in ID and source key.
Namespace new adapter/format IDs by plugin and contribution, independently of package versions. An existing valid owner keeps its file.
Prefer a verified built-in for a new file. If two plugins match, hold ingestion and show both candidates for explicit selection.
Recheck ownership in the writer transaction. Cover manual files, promoted parent roots, headless scans and races between parsing slots.

For each native version, document field references, unknown/default-zero rules, token inclusion, record kind and stable native identity.
For example, a failed model call can count once with unknown tokens. A cumulative session total cannot create individual model calls.
Type observations separately from calls, aggregates and quotas. Route quota-only results through a host quota path with native units and sample time.
Host-validate bounded IDs, numeric ranges, coverage and cursor progression against the supplied complete page. Derive keys from the approved
identity rule. Store package digest/parser version and per-record native evidence as provenance, outside stable deduplication keys.
Where native text is required, expose only declared fields within the grant and persist only statistical fields. A plausible value from an
unknown author still requires native comparison to establish accuracy.

Write accepted observations, aggregates, quota rows where applicable, file fingerprint/generation, cursor and derived summaries atomically.
A partial tail resumes at the last
complete boundary. A trap, failed validation or failed commit leaves the previous position available for retry. Revocation stops the job until
the user grants access again. Respect pause/cancellation and
the existing two-slot scheduler. Changes to parsing policy must replay unchanged consumed inputs when needed, compare complete old summaries,
preserve conflicts and sealed partitions, and never let the guest authorize an old-record correction.
[Data rules](data-contract.md), [scheduling](scheduling.md), [ownership routing](../../../desktop/src-tauri/crates/core/src/adapters/routing.rs).

### Model facts and prices

The host can open a selected data file or fetch a public catalogue independently for each enabled feed. Online refresh is opt-in per feed.
Show the exact fixed HTTPS URL before granting it. The host makes a bounded GET with no credentials, local usage, model-derived query, guest headers
or request body. Reject redirects and non-public destinations, validate resolution at connection time, and keep requests independent of local events.
Retain separate built-in URL restrictions. Key each cache by plugin, contribution and approved source. Store retrieval time, raw digest, source
revision and last valid content. Use the existing three-day default and 1–365 day range. Fetch/parse outside the writer. Recheck settings, generation
and active digest inside it. A failed feed retains its last good data and reports its own error. A manual refresh can run immediately.
Schedule background requests from cache expiry, independently of which models appear in local usage.

- **Model facts.** Identify catalogue entries by exact provider/model, applicable Agent scope, effective interval and source revision.
  Accept typed optional fields such as display name, context window in tokens, maximum output tokens and supported modalities. Show them with
  source and date in model details. Omission means unknown. Keep raw usage, filter/export keys and price matching independent of these facts.
  Keep existing native and built-in/manual mapping rules. Plugin facts fill uncovered fields in the applicable scope. An unresolved higher-ranked
  conflict stays visible. Conflicting peer claims retain both candidates and leave the field unknown (raw key for a name). A change in declared
  context length never changes the observed token count or infers the tier paid. [Model mapping rules](model-aliases.md).
- **Tariffs.** Namespace snapshot and price-row IDs by source. Record provider/model, region/channel, currency, service tier, threshold, unit,
  rate components, effective dates and evidence URLs. Validate integer scaling and the engine's supported units/currencies. Missing components
  stay unknown. Use a new host-assigned plugin rank below manual, seed and built-in community for equally applicable exact rows.
  Same-scope, same-priority conflicting feeds stay ambiguous. A feed revision supersedes only its own earlier applicable version.
  Disallow self-declared `official_vendor`, privileged source types and cross-model substitutions. An enabled plugin can therefore price an exact
  configured route without becoming an official supplier fallback. Preserve the existing region, priority, channel/currency, tier and official
  fallback rules. A refresh may change current references. Previously stored occurrence-time estimates retain their price IDs and amounts.
  Estimation still defaults off. [Pricing rules](pricing.md).
- **Migration.** Map existing catalogue origins, version history and successful cache to their legacy identities. Retain price IDs/ranks and old
  estimates. Check the same source/version with different bytes as a conflict, and allow different feeds to reuse a version string. Recompute display
  caches under a metadata generation. Enabling a second feed or removing the first must not erase either history.

### Additional data exports

The user enables an exporter, creates a named preset, selects fields and filters, previews its coverage and chooses a local file.
For example, a daily JSON Lines preset could export day, raw model, known input and known output for one user and period.
A missing output is encoded as null. A normalized-detail preset requires its own explicit grant. Network destinations and scheduled delivery
would introduce new disclosure and retry semantics and are outside this first design.

The descriptor declares supported dataset (summary or normalized detail), required/optional fields, format, extension and bounded options.
The host computes the intersection of descriptor, grant and preset. A missing required field or unsupported filter fails before writing.
Pin dates/time zone, user/host/source/model
selection, retention coverage and data revision for preview and export. Revalidate a stale preview. Page through one consistent read snapshot
in stable order. Use the existing rules for details and sealed aggregate partitions. Expose unknowns, coverage and monetary currency explicitly.
Keep raw transcripts, paths, credentials, private plugin state and collection cursors outside both datasets.
[Exchange rules](detail-merge.md), [SQLite snapshot isolation](https://www.sqlite.org/isolation.html).

Issue a single-use output handle for the selected filename. The backend writes a temporary sibling and publishes it only on successful finish,
after rechecking grants and overwrite consent. Clean up the owned temporary file on cancellation, disk-full errors, crash or failure.
Protect application data and source files from use as output, including links. For spreadsheet-delimited text, offer a host cell writer with
quoting and formula neutralization. Treat custom output as untrusted data and never execute or automatically open it. Keep the native exchange
formats, their size limits and import detection separate. A successfully generated report makes no round-trip compatibility claim.

## Local model name packages

The first implementation supports one offline `model_aliases` contribution per package.
Open **Settings → Model mappings → Model name plugins** to inspect a local `.tar.zst` file.
Review the plugin ID, version, publisher, licence, rule count and digest, then choose **Install disabled**.
Enable one version to apply its labels. Enabling another installed version switches the active version.
Disabling or removing a package withdraws its labels. Removal retains its catalogue in inactive history.
The same catalogue can be installed again with identical content. The total stored manifest and catalogue history is limited to 32 MiB.

The [plugin authoring guide](/development/plugins/) provides a complete catalogue, a packaging command,
and the install, update and troubleshooting steps. The archive contains exactly two regular files at its root,
`manifest.json` and `aliases.json`. The latter uses the [alias catalogue format](model-aliases.md) with `catalogue` basis.

Use a namespaced lowercase ID and a SemVer package version. The manifest digest covers the exact
manifest bytes, which include the asset digest. Repackaging those same files is idempotent.
Changing either file requires a new package version. The app verifies the asset digest and limits
the compressed package to 2 MiB, the manifest to 16 KiB, and the alias catalogue to 1 MiB and 2,000 rules.
It rejects extra files or link entries. Publisher names are unverified for these local unsigned packages.

The host stores validated manifest and catalogue bytes in an additive v11 SQLite table. A per-database local
grant file selects one digest per plugin ID. A database-only backup carries the installed packages and
requires a matching local grant before using them. In this slice an applicable built-in or manually/online
imported rule has priority from its first effective start. Plugin feeds must agree on the displayed name
throughout the selected range. Conflicts retain the raw name. Aliases change presentation only.

## Delivery decision

Deliver data-only packages first and validate the executable host on Windows before freezing the SDK.
The first usable release must cover a new local Agent, an additional model/price source and a user-configured export through normal application entry
points. Record the source and native sample used for each claim. Runtime choice, package sizes, performance, SDK compatibility and native behavior
are pending the measurements and release checks in the [execution plan](../../../Plan.md#desktop-plugin-delivery).
