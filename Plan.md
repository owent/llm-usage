# Desktop usage execution plan

<a id="桌面用量客户端执行计划"></a>

Current version: 0.2.1. Windows 11 x64 is the first desktop target; Windows/Linux/macOS CI
is retained. This file owns the executable work for the current round. Removed scope and
first failures are recorded in the [round record](docs/validation/desktop-usage/plan-20261007.md);
removal does not mean acceptance passed. Historical results are indexed in
[current acceptance](docs/validation/desktop-usage/current-acceptance.md).

Design entry points: [product and architecture](docs/design/desktop-usage/README.md),
[deliverables](docs/design/desktop-usage/execution.md), [data rules](docs/design/desktop-usage/data-contract.md),
[adapter matrix](docs/design/desktop-usage/adapters.md) and [acceptance criteria](docs/design/desktop-usage/validation.md).

<a id="当前进度"></a>

## Current implementation

| Stage | Implemented scope and results |
| --- | --- |
| M0/M1/M1a | Engineering baseline, SQLite transactions/recovery/statistics/backups, provenance and aggregate exchange. This round added complete normalized detail export, preview and transactional Merge, preserving revisions, conflicts, unknown values, cumulative data and archives; [detail merge rules](docs/design/desktop-usage/detail-merge.md). |
| M2–M5/M8/M9 | Registered parsers, local telemetry and receiver authentication; real-sample labels apply only to tested versions/formats. M8 Zed adds 1.22.0 native external-provider samples for two models and caching. Claude Code 2.1.197 was downloaded through a domestic mirror and validated with native Zhipu main-loop usage for two models and old-database rereads; [record](docs/validation/desktop-usage/claude-container-sample.md). Per-source limits remain in the matrix. |
| M6/F3 | Five pages, selections, retention/export, source schedules, pause/cancel, two-source concurrency, Windows tray/power/notifications and ten languages; [interaction/scheduling](docs/validation/desktop-usage/plan-finalization.md), [Linux Orca](docs/validation/desktop-usage/orca-multilang.md). |
| M7 | Existing Windows NSIS, Linux packages/GTK/WebKit/FUSE/Orca, scale and resource acceptance. Historical CI/artifacts do not verify this round's code; [installation](docs/validation/desktop-usage/installation-lifecycle.md), [scale](docs/validation/desktop-usage/plan-execution.md), [existing CI](docs/validation/desktop-usage/ci-plan-validation.md). |
| F2 | Costs, price snapshots, online cache/failure fallback and official API references. This round added default-off daily/monthly token or single-currency observation-time estimate reminders, exact thresholds and persistent deduplication; [usage and cost alert rules](docs/design/desktop-usage/budget-reminders.md). |
| F1 | Authorized checks of candidate IDE installations and data paths found no testable installation/local usage files/database. Removed from this round without claiming the product is unsupported. |

<a id="本轮结果"></a>

## Application repair results

Unified checks, browser tests, Windows release/native IPC, headless collection and native
Zed rescans completed for the new features. Actual results, first failures and scope
conditions are in the round record; affected design specifications, capabilities and acceptance indexes
were synchronized. Claude's separate work verified domestic download integrity, real
main-loop usage, default-zero correction and new/old-database executable rereads, without
additional GUI/IPC or release acceptance.

Missing current-day Codex collection was fixed by prioritizing unvisited files and rotating
visits across scans when large historical files exhaust the bounded read window. The local
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

Current coverage: all 199 required repository document pairs and 21 guide pairs reviewed,
and all 7944 comment pairs across 460 source files reviewed. Complete product/document checks
passed; the local theme build has 1363 pages and 2853 files, with 31 unit and 30 browser checks passed.
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

## Draft Release validation

- Verify the tag-push workflow locally, then commit the release configuration and recreate v0.2.1.
- Observe successful platform builds, one draft Release and all package/report sizes and digests.
- Repeat publication and verify the same Release ID with replaced same-name assets.

<a id="执行边界"></a>

## Execution boundaries

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
