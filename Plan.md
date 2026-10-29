# Desktop usage execution plan

<a id="桌面用量客户端执行计划"></a>

Current version: 0.3.7. Windows 11 x64 is the first desktop target; Windows/Linux/macOS CI
is retained. This file owns the executable work for the current round. Removed scope and
first failures are recorded in the [round record](docs/validation/desktop-usage/plan-20261007.md);
removal does not mean acceptance passed. Historical results are indexed in
[current acceptance](docs/validation/desktop-usage/current-acceptance.md).

Design entry points: [product and architecture](docs/design/desktop-usage/README.md),
[deliverables](docs/design/desktop-usage/execution.md), [data rules](docs/design/desktop-usage/data-contract.md),
[adapter matrix](docs/design/desktop-usage/adapters.md) and [acceptance criteria](docs/design/desktop-usage/validation.md).

<a id="当前进度"></a>

## Current implementation

Windows Claude 2.1.312 and DSH 0.2.1-rc.2 extraction/recovery checks are recorded in
[native Agent validation](docs/validation/desktop-usage/agent-native-20261029.md).
Zed 1.26.2 now has native mixed custom GLM/DeepSeek thread validation and old-database recovery;
input inclusion/model ownership remain unknown. The reported macOS diagnostics remain unverified.

Software update checks: verify a real newer public-version update. The v0.3.2 public
Release-triggered main documentation refresh is verified in
[0.3.2 checks](docs/validation/desktop-usage/release-032.md). Endpoint publication and the
earlier v0.3.1 draft/source/asset checks retain their scope in
[the earlier release record](docs/validation/desktop-usage/release-031.md).

| Stage | Implemented scope and results |
| --- | --- |
| M0/M1/M1a | Engineering baseline, SQLite transactions/recovery/statistics/backups, provenance and aggregate exchange. This round added complete normalized detail export, preview and transactional Merge, preserving revisions, conflicts, unknown values, cumulative data and archives; [detail merge rules](docs/design/desktop-usage/detail-merge.md). |
| M2–M5/M8/M9 | Registered parsers, local telemetry and receiver authentication; real-sample labels apply only to tested versions/formats. M8 Zed adds 1.22.0 native external-provider samples for two models and caching. Claude Code 2.1.197 was downloaded through a domestic mirror and validated with native Zhipu main-loop usage for two models and old-database rereads; [record](docs/validation/desktop-usage/claude-container-sample.md). Per-source limits remain in the matrix. |
| M6/F3 | Five pages, selections, retention/export, source schedules, pause/cancel, two-source concurrency, Windows tray/power/notifications and ten languages; [interaction/scheduling](docs/validation/desktop-usage/plan-finalization.md), [Linux Orca](docs/validation/desktop-usage/orca-multilang.md). |
| M7 | Six native Windows/Linux/macOS x64/arm64 portable archives and v0.2.2 draft publication/overwrite verification completed; [CI record](docs/validation/desktop-usage/ci-plan-validation.md). Existing Windows NSIS, Linux packages/GTK/WebKit/FUSE/Orca, scale and resource acceptance retain their original scope; [installation](docs/validation/desktop-usage/installation-lifecycle.md), [scale](docs/validation/desktop-usage/plan-execution.md). |
| F2 | Costs, price snapshots, online cache/failure fallback and official API references. This round added default-off daily/monthly token or single-currency observation-time estimate reminders, exact thresholds and persistent deduplication; [usage and cost alert rules](docs/design/desktop-usage/budget-reminders.md). |
| F1 | Authorized checks of candidate IDE installations and data paths found no testable installation/local usage files/database. Removed from this round without claiming the product is unsupported. |

<a id="本轮结果"></a>

## Application repair results

0.3.3 fixes Codex inherited-compaction ownership conflicts and adds the independent OpenCode v2
projection reader. Valid compatible reads no longer show a source warning; missing-file notices
describe the observed path state and history limits. Real local read-only samples and a consistent
statistics-database backup verify recovery without clearing data. Windows release/headless and
native v2 fixture repeat reads passed. A follow-up traced invisible OpenCode usage to null-total
display filtering; overview/trend now show known bounds across cards, tables, shares and curves,
with field-specific coverage tooltips. The rebuilt Windows executable passed isolated native
WebView2/IPC checks for those seven calls and bounds. Further source tracing found discarded raw
usage in v2.0.32 and verified independent recovery of inclusive output. Parser 2 corrects
output/total coverage against complete old summaries; unknown input detail remains unknown.
Draft Release notes now use the annotated tag message and verify the body after asset upload.
Checks and remaining acceptance
boundaries are in the [0.3.3 repair record](docs/validation/desktop-usage/release-033.md).
Codex parser 8 handles validated oversized non-usage objects in bounded streaming
windows and rechecks old cursors; native statistics-copy recovery preserves prior
events and restores usage after the blocked line. See the same record's
[oversized-record checks](docs/validation/desktop-usage/release-033.md#oversized-codex-records).
Qoder CLI, IDE and JetBrains are requested local-support targets. User-authorized
source inspection now implements CLI 1.1.76 transcripts with synthetic fixtures,
positive token buckets, redacted-zero uncertainty and completed-response counts.
IDE 1.35.0 and JetBrains 2026.1021.81866741 distribution inspection has not established
native per-call token persistence; those readers and all real-client acceptance remain
pending. See the [field contract](docs/design/desktop-usage/adapters.md#qoder-local-support)
and [source checks](docs/validation/desktop-usage/release-033.md#source-derived-qoder).
Qoder marker detection now shares a bounded fingerprint cache and runs outside the writer;
ownership/history protection is retained. See [issue 3/4 validation](docs/validation/desktop-usage/issues-3-4.md).
Windows CI test timeout repair batches new-database creation and Kilo sample writes, and
keeps native-sample queries within the dates of all retained events. Local timing, rollback
coverage and the original remote timeout are recorded there; CI shows test compilation and
execution separately without removing platform or durability checks.

The 2026-10-24 maintenance round updates dependencies and offline prices, repairs source
pagination/conflict/detection and SQLite cancellation, and adds regression coverage.
The [maintenance record](docs/validation/desktop-usage/maintenance-2026-10-24.md) holds
current verification results and the limits of real-source and platform acceptance.

Kimi K3.1 Pro and Highspeed pricing now includes verified model aliases and independent
CN/global offline rates. Existing databases import the added snapshot on startup; historical
estimates remain unchanged. [Pricing repair validation](docs/validation/desktop-usage/kimi-k31-pricing.md)
records regression coverage and verification limits.

Release 0.3.5 includes this pricing repair and the existing dependency/source maintenance.
The user authorized the version bump, commit, annotated tag and push on 2026-10-25.
See the [release preparation record](docs/validation/desktop-usage/release-035.md) for local
verification and the tag workflow's draft-release behavior.
The SQL cancellation tests synchronize with actual worker interrupts to avoid macOS
timing failures; [repair validation](docs/validation/desktop-usage/release-035.md#macos-cancellation-test-repair)
records the original CI failure, regression checks and current verification limits.

System update notifications are implemented under the [software update specification](docs/design/desktop-usage/application-updates.md#system-notifications):
native Windows/macOS/Linux delivery, persisted version deduplication, an independent saved
switch, and isolated notification failures. Each version has three persisted submission
attempts at most, including interrupted attempts; restarts and version alternation retain the
limit. Windows Notification Center read-back and local
regression gates passed; Linux function compilation/error handling and macOS source review
remain distinct from desktop acceptance. See the [notification validation record](docs/validation/desktop-usage/application-updates.md#system-update-notifications-2026-10-20).

Unified checks, browser tests, Windows release/native IPC, headless collection and native
Zed rescans completed for the new features. Actual results, first failures and scope
conditions are in the round record; affected design specifications, capabilities and acceptance indexes
were synchronized. Claude's separate work verified domestic download integrity, real
main-loop usage, default-zero correction and new/old-database executable rereads, without
additional GUI/IPC or release acceptance.

Missing current-day Codex collection was fixed by prioritizing appended content after EOF
(including partial final lines), then rotating historical files across bounded windows. The local
old database was backfilled and independently reconciled; see the
[recovery record](docs/validation/desktop-usage/codex-today-recovery.md).

Visual Studio Copilot discovery was repaired for TMP/TEMP and launcher environment differences,
with a read-only official-vswhere inspection tool covering all versions/SKUs. Investigated
VS 2022 components lack the verified VS 18 JSONL exporter; this does not verify all versions
with native samples. See [cross-version references](docs/validation/desktop-usage/m9-vs-copilot-discovery.md).
The previously authorized application round has no remaining active executable items;
environment-blocked work was removed at the user's request while its limits were preserved.

Additional DPI, complete screen-reader coverage, host login/logout and OS wake measurements
were removed at the user's request. Main-branch merge, signing/notarization and Release were
reported complete by the user and removed from that application round; this statement is
not new release verification. Sources/system cases missing accounts, protocols, artifacts,
historical records or permission remain outside active application work with their original
conditions and historical acceptance requirements preserved.

<a id="文档发布进行中"></a>

## Documentation publication results

The user has now explicitly authorized an English-default repository documentation/comment
migration, complete Chinese counterparts, an Astro documentation site, real localized
screenshots, a gh-pages branch, automatic CI publication and `llm-usage.atframe.work`.
The [documentation requirements](docs/design/documentation-site.md) define content ownership,
language behavior, screenshot provenance and deployment acceptance.

- [x] Translate user, architecture and development documents and source comments, preserving original sources and results and complete Chinese
  counterparts. AI rules, Skills and execution plans keep one original.
- [x] Write 21 paired user/developer guides, with matching localized links and screenshots.
- [x] Complete required-document translation and source-comment reference checks.
- [x] Verify the local production build, localized search, language negotiation, themes, keyboard navigation and mobile layout.
- [x] Review 20 real English/Chinese screenshots from isolated synthetic sources, in both themes at 2880×2000 pixels.
- [x] Add five localized feature screenshots to each homepage, plus README and record-details examples; verify theme switching and original-image links.
- [x] Publish the compiled site to gh-pages and configure/test automatic publication.
- [x] Verify Pages deployment, custom-domain DNS and HTTPS independently.
- [x] Repair empty sidebar groups, expand wide-screen content, add localized download actions and simplify related titles/copy; pass local checks.
- [x] Observe successful CI publication and public-domain navigation for the latest layout update.
- [x] Improve light/dark color separation and sparse decorative backgrounds; measure contrast
  and review both languages, responsive layouts, forced colors and printing.
- [x] Commit/push the theme update and verify its automatic publication and public HTTPS pages.
- [x] Apply the requested cooler, less bright background colors and decorative card fills;
  check contrast and review screenshots in both languages and themes.

Current coverage: all 209 required repository document pairs and 21 guide pairs reviewed,
and all 8007 comment pairs across 470 source files reviewed. Complete product/document checks
passed; the local theme build has 1403 pages and 2939 files, with 41 unit and 31 browser checks passed.
Screenshot source db44b0300750ef4f3e00c89ad21edbfa83e60813 was published, and run 37716140836 attempt 2
passed build/publication after the explicitly authorized exact main deployment rule was added.
Both language homepages and all 20 original PNGs were checked through the custom HTTPS domain.
Navigation/layout source 16c77e3fd386071ce266d862e70914a33cf2b4ed passed automatic run 37719512134;
its Pages deployment and public HTTPS menu/download interactions passed. Both guide languages use
1760px content at a 2560px viewport, with no mobile overflow. Theme review measured minimum text
contrast of 4.93674:1 (light) and 5.54803:1 (dark), with keyboard outlines at least 4.57120:1
after the cooler, less bright background adjustment. All 30 browser checks still pass.
Theme source 6025b0750f852686f9e874ea6f9a1b97f70887dd passed automatic run 37722735785;
its Pages build and public HTTPS light/dark checks passed in both languages. Menu, download and
mobile navigation passed; all 20 remote PNGs match their originals. The cooler background
follow-up has passed local checks; publication of pushed documentation uses the same automatic
[workflow](https://github.com/owent/llm-usage/actions/workflows/docs.yml).
Translation uses the current language model, with complete
pair review and contextual wording checks. First failures, validation commands and exact remaining work are in the
[documentation record](docs/validation/desktop-usage/documentation-site.md).

<a id="执行边界"></a>

## Execution boundaries

Software updates follow [the update requirements](docs/design/desktop-usage/application-updates.md);
implementation and local results are in [update validation](docs/validation/desktop-usage/application-updates.md).
Settings now save automatically with immediate selection commits, 600 ms text debounce,
IME deferral, serialized writes and navigation flushing under the
[interaction requirements](docs/design/desktop-usage/dashboard-polish.md). Automatic downloads
default on for new/missing settings; explicit opt-outs are preserved.
Unrelated autosaves retain statistics and trend/today selections; calendar settings still
invalidate them, with regression evidence in the same issue 3/4 record.
Version 0.3.0 cost-card, settings and native Codex corrections are recorded in
[local release checks](docs/validation/desktop-usage/release-030.md).
Version 0.3.2 registers individually checked Codex/Kilo native versions and clarifies
Copilot telemetry wording, and saves updated creation fingerprints so large Codex files
can finish rereading. Its tagged commit passed all 12 CI jobs and produced a Release
with 13 verified assets, now publicly available. Actions now use verified numeric version tags;
local and tagged-build results are recorded in
[0.3.2 validation](docs/validation/desktop-usage/release-032.md).
The creation-fingerprint regression now also runs without filesystem birthtime and checks
the persisted fingerprint after reopening; native and simulated coverage is recorded there.
Remaining acceptance: run real NSIS updater/lifecycle checks when the existing application is stopped;
verify a newer public release when available, and record Linux/macOS native GUI updates separately.

- Inspect source, configuration, tests and version references before conclusions; synchronize affected rules, skills, design specifications and records.
- Collect only local sources. Preserve unknown values and separate calls/messages/cumulative data/quotas. Estimates default off; currencies stay separate.
- Read-only real-data extraction follows the [readiness rules](docs/design/desktop-usage/implementation-readiness.md), allowlist and redaction.
- Existing authorization covers Podman, specified-provider minimal requests, local Zed
  configuration/tests and F1 inspection. Credentials stay in the target process or OS store,
  never records or Git.
- Specified endpoints were verified as Coding Plan, not actual pay-as-you-go bills; do not
  price subscription usage as observation-time API spend. Other records without channel
  information remain limited.
- Preserve user changes. Temporary artifacts stay in root build/. Do not sign into another product or fabricate authentication state.
- Keep source references/checks per record version. Discovery, old positions, revisions/conflicts and sealed
  partitions follow the data rules. Update status only from observed results.
- Documentation publication is authorized; unrelated application changes are preserved and kept outside the documentation deployment snapshot.
