# Dependency, pricing and source maintenance

Date: 2026-10-24. Version 0.3.4. The review covered the ten source commits in
`c2940401..f2df0a6e`, from 2026-10-12 through 2026-10-23, comprising 165 files.
Work in this record is uncommitted; no commit, push, deployment or release was performed.
No Agent was installed or launched and no model request was made. Real local Agent data
was read-only and limited to redacted counts, versions, token sums and diagnostic categories.
Temporary sources and verification databases stayed under the repository's ignored `build/`.

## Environment and dependencies

The local environment is Windows 11 x64, Node 24.21.0, Rust 1.99.1, pnpm 12.15.0,
Tauri CLI 2.12.4 and WebView2 155.0.4283.62. Both workspace manifests now declare
Rust 1.90 as the minimum version; rusqlite 0.41.0 itself requires Rust 1.89.

| Component | Selected version and constraint |
| --- | --- |
| Astro / Starlight / Markdownlint | 7.4.2 / 0.42.7 / 0.24.0 |
| Tauri API and CLI / crate / build crate | 2.12.4 / 2.12.5 / 2.7.4 |
| Svelte / Vite / Playwright | 5.57.2 / 8.3.5 / 1.64.1 |
| rusqlite / bundled libsqlite3-sys | 0.41.0 / 0.39.0 |
| pnpm / repository Rust toolchain | 12.15.0 / 1.99.1 |
| TypeScript | 6.0.3, retained for compiler API and peer compatibility |

Both npm manifests and all npm/pnpm locks were synchronized. Root and desktop
`npm outdated` show only TypeScript 7.0.2. The latest `@astrojs/check` 0.9.12 and
`svelte-check` 4.7.7 both accept TypeScript `^5 || ^6`; no peer check was bypassed.
`cargo update --dry-run` resolves no further compatible updates. Five older indirect
packages remain for upstream reasons: brotli requires `~6.0`, crypto-common requires
`generic-array =0.14.7`, and proc-macro-crate 2.0.2 pins toml_datetime 0.6.3 and
toml_edit 0.20.2, retaining toml 0.8.2. The WASM-related locks were also advanced to
sqlite-wasm-rs 0.6.2 and rsqlite-vfs 0.2.0.

The first npm audit used the configured Tencent mirror, which returned 404 for the audit
endpoint. The official npm endpoint reported zero vulnerabilities for root and desktop.
The first pnpm audit found nine advisories in stale transitive resolutions. Updating within
the existing ranges selected undici 8.12.3, zod 4.6.7, fast-uri 3.1.9 and
postcss-selector-parser 7.1.7, without a new override. The final pnpm audit is also zero.
`pnpm install --frozen-lockfile --strict-peer-dependencies` passes. KaTeX 0.19.1 is a direct
root dependency and an override; tests cover ordinary rendering and reject an inherited
`trust` option.

## Offline prices

`desktop/src-tauri/crates/core/prices/seed-2026-10-24.json` adds 25 official standard
reference rows. Five snapshots now contain 51 rows. The seed records the official OpenAI,
Anthropic, Gemini, Kimi, bigmodel and z.ai page URLs and the 2026-10-24 verification date.
The 27 price regression tests pass.

OpenAI's 30-minute/12-hour cache-write TTLs do not fit the existing 5-minute/1-hour fields.
All eight new OpenAI rows keep both write prices NULL and leave that component unpriced;
notes are not executable rates. GLM-5.1 output prices are 240000/280000 in the stored
0.0001 CNY per million tokens, meaning ¥24/¥28 per million, not ¥2.40/¥2.80.
No exact Kimi K2.8 price was invented; the authorized K2.7 Code substitute rule is unchanged.
Estimates remain opt-in, with original currencies, channels, dates and unknown components kept.

## Code repairs and regressions

- **Codex inherited ownership and conflicts.** Complete old summaries and full native
  carry-normalized digests distinguish inherited carries from native responses even when
  both lack a source revision. Contradictory or unmatched native candidates cannot be
  skipped during replay or clear a real conflict. Historical conflicts with incomplete
  candidates stay `legacy_unknown`. Codex parser references may stay unchanged or advance,
  never regress during replay. Tests cover multiple carries, a contradictory native response,
  candidate order, rollback and ownership correction with the current parser.
- **OpenCode v2 pagination and malformed IDs.** Typed non-null IDs advance without SQLite
  coercing them into text; NULL IDs receive diagnostics without blocking valid rows. Saved
  cycle-completion state preserves non-finite-ID exclusion across bounded pages, including
  a first partial page containing only such an ID. A `max_lines=1` regression reaches both
  later valid records instead of restarting forever. Existing contexts without the new state
  retain their initial-cycle behavior.
- **Qoder detection.** Fingerprint probing stays outside the writer and retains the 4,096-entry
  LRU. Per-path probe generations exist only while probes are active. Work on another file
  cannot force a redundant probe or overwrite a result; a newer unstable probe of the same
  file prevents an older stable result from being cached. Cancellation and incomplete tails
  do not become negative cached evidence. Ten detection tests and 16 transcript integrations
  cover concurrency, replacement, partial writes, registry routing and history.
- **Kimi wire state records.** The verified Kimi Code 2.3.1 protocol-1.5 manifest ignores
  persistent/control state only in that product's reader. `goal.update.tokensUsed` and other
  cumulative fields do not create calls. Other protocol versions and Kimi Work 1.4 retain
  unknown-type diagnostics and product-specific version references. Tests cover all 21 newly
  listed state types and their cross-product boundaries.
- **Copilot CLI verification helper.** Empty source tables leave `SUM(...)` values as `None`
  instead of raising `InvalidColumnType`. The result is explicitly `CHECK`, not a successful
  native-format acceptance or an invented zero. `Option` sums preserve unknown usage.
- **rusqlite 0.41 migration.** Local source handlers use mutable connections as required by
  the new `progress_handler` signature. Shared storage retains its existing interface through
  scoped `InterruptHandle` workers. A worker repeats interruption while cancelled, makes the
  state visible to the executing thread's checks, and is awakened and joined before connection
  reuse. Drop restores outer control state and retains the pause classification. Tests force
  cancellation before a later long statement and override a fresh allowed-cache entry.
- **Harness and compatibility checks.** Portable tests handle a Windows `D:` drive root;
  native checks restore the selected language and search across both Details pages. The
  Rust 1.90 lint updates preserve the original Option conditions, and the backup test uses
  the new mutable handler signature. The complete standard verification below supersedes
  earlier intermediate failures.

## Real local data

The registry contains 41 adapters, all exercised by registry-driven structural and
synthetic checks. Bounded discovery found 11 adapters with local roots; nine produced
nonempty usage, while Copilot CLI and Zed had empty records. This does not certify every
native format of all 41 adapters or every interface of the nine sources.

| Source | Read-only extraction and repeat scan |
| --- | --- |
| Codex | 2,016 calls; no diagnostics or conflicts; repeat adds 0 |
| Pi | 165 calls; no diagnostics; repeat adds 0 |
| oh-my-pi | 8,769 calls; no diagnostics; repeat adds 0 |
| Kilo | 13,606 calls; one cumulative reconciliation mismatch and one latest-fallback reference retained; health `ok`; repeat adds/updates 0 |
| ZCode | 6,782 calls; 127 turn comparisons give 102 matches and 25 differences; turn totals remain reconciliation-only; repeat adds 0 |
| Kimi Code | 2,769 calls in a fixed 42-file snapshot; per-file protocol references; no diagnostics; repeat adds 0 |
| Kimi Work | 1,337 calls across 69 protocol-1.4 files; no diagnostics; repeat adds 0 |
| CodeBuddy | Two events, with no diagnostics and a stable repeat count |
| VS Code Copilot Chat | 1,604 calls and 72 observations; repeat adds 0. All 72 have `turn_input_incomplete`, so whole-turn input and total-token completeness remain limited and health stays `ok` |

The other 30 registered adapters had no local candidate roots in this check. Copilot CLI's
`assistant_usage_events` and `sessions` tables are empty, and Zed's threads table is empty;
these are empty sessions, not successful native-format verification. No Qoder client was
available, so Qoder evidence remains source-derived synthetic coverage. Existing synthetic
and historical records for other adapters are not new real-client acceptance.

All nine real-source instances report `ok` health. Remaining diagnostics are limited to the
known `latest_fallback`, `reconcile_mismatch`, `turn_input_incomplete` and
`unmapped_query_source` categories. No original Agent file or live statistics database was
rewritten; only new isolated verification databases were used.

## Verification commands and results

| Command or check | Verified result and scope |
| --- | --- |
| `npm run verify` | Exit 0: 1,123 Rust tests passed, nine explicitly ignored for platform or opt-in native conditions; 20 script tests and 32 frontend tests passed; Markdown/assets, Svelte diagnostics, rustfmt, Clippy and frontend build passed |
| `npm run build:desktop` | Windows x64 release executable and NSIS installer built with rusqlite 0.41.0 and the final cancellation state handling |
| `npm run test:headless` | 11 real executable/SQLite checks passed with isolated synthetic sources |
| `npm run test:receiver` | Eight native IPC/HTTP/credential checks passed; `credentials_remaining=0` |
| `npm run test:update:check` | Both unpackaged/public-version and missing-identity checks passed; actual status `up_to_date`, without asset selection or download |
| `npm run test:update:windows` | Native synthetic portable replacement and recovery checks passed; this does not claim a newer public release |
| `npm run test:desktop -- --runs 1` | 27 real WebView2/IPC checks passed: five main pages, nine settings sections, 21 events/20 calls/one observation, 20+1 Details rows, no added events on repeat/restart; `errors=[]`, `outbound=[]`; first-screen P95 810.9346 ms |
| `npm run test:browser` | Complete mocked-IPC browser checks passed: update/settings/cost layouts, grouped/hourly tokens, telemetry, ten locales, themes, filters and pagination |
| `npm run test:import` | One million synthetic events/15,000,000 tokens reached the GUI; visible after 648,808.4962 ms; 1,036 samples over 650.0523515 seconds; process-tree peak 355.55078125 MiB, within the 512 MiB limit; repeat adds no events; `errors=[]`, `outbound=[]` |
| `npm run test:install:windows -- --previous-installer build/install-lifecycle/windows/previous/LLMUsage_0.3.3_x64-setup.exe --previous-version 0.3.3` | All 12 actual NSIS lifecycle checks passed: install, upgrade, rollback, uninstall/reinstall and refused cleanup; usage/settings retained; no test installation, startup entry, scheduled task or process remained |
| `pnpm install --frozen-lockfile --strict-peer-dependencies` | Both workspace importers and the shared lock passed, including scoped build approvals and peer checks |

The 282 core-library tests are included in the 1,123 above, not additional checks.
Registry-driven tests cover all 41 adapters. Real-source acceptance is limited to the
nonempty local data described above and does not establish missing-source formats.

## Failure history and scope limits

The first one-row OpenCode non-finite-ID case repeated the same page; the saved
cycle-completion state fixes it. The added Codex case first cleared a real conflict and
could downgrade the parser; full-digest comparison and monotonic replay now retain both.
The rusqlite upgrade initially required mutable handlers, then exposed 19 MSRV-gated
`Option` simplifications and four example/test issues. These were corrected without lowering
lint levels or restoring an older dependency.

The first native pause check changed `last_finished_ms`. Independent source reviews then
identified a one-shot interrupt that could be lost between SQL statements, a cached-true
misclassification and nested-scope masking. Final code publishes cancellation before
interrupting, repeats it while the scope is active, stops and joins before restoring control,
and retains each outer scope's flag. A third review confirmed those orderings; the final
27-check native acceptance passes. One NSIS attempt overlapped the native GUI and was
refused by its process precheck; the serialized 12-check lifecycle passed.

The 41 adapters are not all verified against real local data. This round has nine nonempty
sources, two empty records and 30 without local candidates. Qoder has no native client check.
No new Linux/macOS desktop, installer or credential-store acceptance is claimed, and no
newer public release was available. These are unverified scopes, not claims of failure or
support. Offline prices are references, not evidence of an actual billing channel or bill.

The separate Windows notification test read its test notification from native history;
`Setting` returned element-not-found (0x80070490), so no visual banner or click is claimed.
Blocking OS calls still have no guaranteed immediate interruption, and tests cannot exhaust
all possible concurrent schedules. The original Kimi snapshot and real verification databases
were deleted after redacted inspection. Receiver credentials and owned OS tasks are cleaned;
work is not committed, pushed or released.
