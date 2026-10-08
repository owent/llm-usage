# Version 0.3.0 repairs and local release checks

2026-10-08, Windows 11 x64, Node 24.21.0, Rust 1.98 and installed Edge/WebView2.
This record covers the cost-card, Codex and settings repairs. Software updates and
ten-language terminology checks retain their [separate record](application-updates.md).
Disposable probes, screenshots, database copies and logs are under `build/release-030/`;
browser screenshots are under `build/browser-smoke/`.

## Visual reproduction and controls

The failing browser regression reproduced overflow in both overview and trends at
1660/1920 pixels. At 1660 pixels, eight columns left about 143 pixels of content in
the trend cost card, while a single-line USD conversion needed about 249 pixels.
The main amount already had ellipsis rules; converted amounts and tier hints did not.
Screenshot inspection confirmed clipping beyond the card. Each compact child now
has a bounded width and ellipsis. Tooltips retain full original/converted ranges.
Amounts, exchange rates and rounding rules are unchanged.

Edge checks pass at 760/1440/1660/1920 pixels for both pages. Settings checks cover
custom switches in general, startup/background, costs and software updates; keyboard
Space, label clicks and saved settings; and custom export checkboxes with the system
high-contrast fallback. On/off options use switches; multiple export selections retain
checkbox semantics under the [WAI pattern](https://www.w3.org/WAI/ARIA/apg/patterns/switch/).
Final screenshots were visually inspected. The initial tooltip assertion selected a
substitute hint; it was corrected to select the actual USD conversion.

## Native Codex investigation

A read-only query reproduced 251 active, 48 compatibility-only, three degraded and
28 incompatible files. All 79 flagged native files were inspected with bodies removed,
IDs hashed and token fields limited to the six native counters. Each inspected file's
session metadata consistently identified its own version. All 79 had valid JSON and
valid retained numeric usage; none had a malformed non-null compaction carry.

| Native version | Files inspected | Selected fixture calls | Reader |
| --- | --- | --- | --- |
| 0.148.0-alpha.21 | 1 | 1 | rollout_legacy |
| 0.149.0-alpha.4 | 3 | 61 | rollout_legacy |
| 0.150.0-alpha.8 | 8 | 12 | rollout_legacy |
| 0.150.0-alpha.12.2 | 5 | 6 | rollout_legacy |
| 0.151.0-alpha.7.2 | 12 | 3 | rollout_legacy |
| 0.153.4 | 11 | 25 | rollout_v1 |
| 0.155.0-alpha.16 | 18 | 2 | rollout_v1 |
| 0.162.0-alpha.2 | 21 | 1 | rollout_v1 |

The 28 incompatible files used legacy `token_count` increments, but their exact
versions were absent from the registry, causing dispatch to the modern fallback.
Five exact legacy entries and three exact modern entries are now registered.
Unknown versions still require compatibility checks; alpha version ranges are not
implicitly certified. Native version strings and local records establish these samples,
without claiming all official distributions or other transports use the same fields.

The three degraded 0.153.4 files had `latest_token_usage_record: null` in compaction
records. Null means no carried call, matching absent usage. The parser now skips it;
non-null malformed objects remain errors. Parser version `codex-rollout-5` triggers
automatic rechecking of consumed unchanged cursors without clearing data/history.

Eight redacted fixtures and independent six-field expectations are committed under
`desktop/src-tauri/crates/core/tests/fixtures/codex/releases-030/`. The new regression
failed before the mapping fix. Two new tests now pass across eight native versions,
111 selected calls, repeat reads, old parser cursors and missing/null/invalid carries.
Five existing health/replay regressions also pass. Cumulative discrepancies retain
their original diagnostics; active file health does not promise complete history.

## Release verification

Root/desktop npm manifests and locks, Tauri configuration, both Rust manifests and
Cargo.lock agree on 0.3.0; the packager's version check returns 0.3.0. Local native
build, old-database copy recovery and final acceptance results are recorded below.
Creating a local tag does not publish a remote Release or establish signing,
NSIS updater lifecycle, other-platform GUI or a real newer public-release upgrade.

The SQLite online backup included the live database's WAL. Only the independent copy
was modified, with non-Codex sources disabled and the native Codex root read-only.
Two `--scan-once` runs of the 0.3.0 release executable both exited 0. All 332 discovered
Codex files became active, including two newer files. All 1,348 original event identities,
tokens, quality, model/time/provider fields and native costs were preserved. Retained
events reached 1,568 under the existing retention policy and stayed unchanged on repeat;
conflicts remained zero. Diagnostics grew from 6,422 to 26,615 and were not discarded.
The original live database and existing development application were left untouched;
the new executable applies the correction on its next collection.

| Command | Result |
| --- | --- |
| `npm run verify` | Exit 0; 1,046 Rust tests, 22 frontend tests, 13 script tests pass; eight platform tests ignored; types, formatting, Clippy and frontend build pass |
| `npm run test:browser` | Exit 0; simulated IPC in Edge, including overflow and switch regressions; screenshot inspection |
| `npm run build:desktop` | Exit 0; optimized 0.3.0 executable and x64 NSIS package |
| `npm run test:headless` | Exit 0; 11 real executable/SQLite checks with isolated synthetic sources |
| `npm run test:update:windows` | Exit 0; three native public-check/portable replacement/interruption recovery groups; synthetic newer package |
| `node desktop/tests/native-smoke.mjs --runs 1` | Exit 0; 19 real WebView2/IPC checks, three lifecycle startup samples, P95 987.08 ms; isolated synthetic sources |
| `npm run check:docs`, `npm run test:docs`, `npm run build:docs` | Exit 0; 202 repository translation pairs, 21 guide pairs, 31 tests and 1,375 pages/2,880 published files |
| Native Windows packager | Exit 0; fresh extraction, CPU identity, SHA-256 and isolated extracted headless checks |

The first unified run exposed an expected-version list that still omitted the eight
new entries; its independent expected list was updated. The documentation inventory
test likewise needed the new evidence route and page count. npm rejected forwarded
`--runs` before launching the native GUI test; the direct Node entry point was used.

The local Windows portable archive is 4,200,402 bytes, SHA-256
`b90e4990bf07b4ffd73863796f750de65f8197e74a29bda09f0c59805a3c6de3`.
The NSIS installer is 4,126,266 bytes, SHA-256
`2c039e88830bbcd4894124e0fbb35f1c67d47f6072780de4b9d24fc57449f59b`.
Artifacts and the report are under `build/release-030/packages/`, marked as local
working-tree builds; historical 0.2.2 CI/signing results do not certify these packages.
