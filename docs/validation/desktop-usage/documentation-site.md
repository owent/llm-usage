# Documentation site implementation record

<a id="文档站实施记录"></a>

Started: 2026-10-07. Review date: 2026-10-08. Status: all document and source-comment pairs
have been reviewed and local checks passed. Screenshot publication, automatic CI and public-domain
HTTPS are verified; the latest navigation/layout update awaits publication. [Plan.md](../../../Plan.md) owns active status;
the [documentation requirements](../../design/documentation-site.md) define behavior.

<a id="环境与范围"></a>

## Environment and scope

Working directory: D:/workspace/projs/github/owent/llm-usage. Windows, PowerShell 7.6.6,
Node 24.21.0 and Git 2.56. Astro 7.3.6 and Starlight 0.42.5 are locked at the root.
The documentation has an English root and /zh-cn/ Chinese routes, browser-language
negotiation with explicit preference, system/light/dark themes and local Pagefind search.
No third-party browser requests were observed in local build checks. The public domain's
Cloudflare proxy adds an analytics script separately; it is absent from the compiled artifact.

The user authorized documentation publication, including gh-pages and the custom domain.
Before this screenshot update, the user committed and pushed the initial bilingual documents
as af64b8951936c5b009640537cdad51e1f4da4164, including the preceding application commit.
The screenshot and navigation updates change documentation and site tooling. Temporary logs, databases,
generated content and previews stay under build/documentation-site/.

<a id="已实施内容与剩余覆盖"></a>

## Implemented content and remaining coverage

There are 21 paired user/developer guide pages. They cover installation, first collection,
dashboard interpretation, sources, clients, scheduling, telemetry, pricing, data management,
privacy, troubleshooting, accessibility, development, architecture, adapters, testing,
releases, documentation maintenance and source-comment references.

All 199 user, architecture and development document pairs have been translated and manually
reviewed, including this implementation record, design specifications, test expectations, asset
guidance and the historical prototype usage instructions. AI rules, Skills and execution plans
retain a single original. Their 14 unnecessary translated copies were removed from docs/zh-CN;
their links now point to the original repository files. The complete original Chinese counterparts
were preserved alongside the completed English editions. All 7939 comment pairs across
460 source files have been reviewed. Comments in migrated prototype/configuration/Rust files
were changed without changing executable content outside comments/docstrings. Runtime
catalogs, native test samples and third-party license notices retain their original content.

The content check rejects missing or changed pairs without review. Hashes record synchronization;
translation quality is checked by directly reading complete pairs.

The review also found 19 nonempty comments whose Chinese reference incorrectly used
the empty-line marker. Their paths, formulas and explanations now have complete Chinese
references. A regression rejects that marker for comments containing letters or numbers,
while permitting actual blank or separator comments. Draft preflight also rejected ASCII-only
Chinese continuation lines before source files were written; those references were completed.

Installer review found 16 Handlebars template expressions incorrectly counted as NSIS
comments because they contained `#`. The scanner now skips complete template expressions,
including quoted delimiters, while retaining actual inline hash comments permitted by the
[NSIS syntax reference](https://nsis.sourceforge.io/Docs/Chapter4.html#4.1). Two regressions check
template/non-comment preservation and all 32 conditional/loop directives in the actual installer.
After that installer review, the inventory had 539 source files, including 460 files with 7939 actual comments.
The current inventory has 541 source files; the two sidebar modules add no comments.
The first real-installer regression used an incorrect repository-relative path and failed with
ENOENT; correcting that path made all 22 documentation tests pass. Installer translation
preserved executable content, template directives and license notices; this is not new package-lifecycle acceptance.

Further comment review corrected Crush pagination/event keys, Copilot row-error handling,
the Kilo synthetic timestamp date and test-scope descriptions for Kimi Work and calendar checks.
Hermes enumeration uses depth 3; OpenCode accepts direct-database manual roots, with format
detection rather than filename prefixes establishing product identity. Both languages now
describe those implementations. Runtime strings, synthetic values and assertions remain unchanged.

Final data-rule, adapter-matrix and M8 historical-report pairs retain unknown values,
version limits, first failures, sample values and artifact hashes. The adapter matrix now
reflects the implemented Qwen 0.25.0 SDK reader and assigns Visual Studio its unique A49
reference. Source-comment review corrected obsolete Cline/Zoo detection descriptions and
ZCode nullable-field descriptions against the implementation, without changing executable code.
Document and comment pairs are complete; pass all required checks before publication.

The current language model directly translates complete documents. The wording review covered
paired guides, engineering instructions, navigation, home cards, headings and captions. It replaced
vague engineering labels with named specifications, field rules, test data, processing positions
and results, preserving identifiers, original output and historical anchors. An earlier stage
translated the Chinese writing-guidance copy; that unnecessary copy was subsequently removed
when the user excluded AI rules and Skills from bilingual maintenance.

Source/sample review also found two inaccurate calculations: native Codex 0.139.0 has eight
positive increments after its first call and one unchanged repeat, producing nine calls;
the Pi cost sample has input 100+5=105, matching pi_gaps_synthetic.rs. Both language editions
were corrected. Architecture now describes the implemented two-worker scanner and scoped
JSON/JSONL/SQLite cancellation, checked against scanner.rs/framework.rs and scheduling rules.

Further review corrected stale descriptions: Goose 1.53.0 has native container validation and reads pages of up to 50,001 rows;
the price-cache default is three days and UI query debounce is 200ms. Unvisited, merged or interrupted sources do not advance
collection deadlines. Codex registry parser/supported-version changes reset checkpoints, contrary to the old no-automatic-rescan
comment. Application schema mismatches require explicit handling rather than automatic migration. Both languages were checked
against implementation, preserving native values, versions, runtime strings and assertions.

The first product check rejected ingest parameter alignment after comments became longer. rustfmt changed whitespace;
non-whitespace code and comment contents were compared before refreshing positions/hashes. The next clippy run rejected
unindented Kiro documentation-list continuations; correcting them made complete verify pass. Reapplying that draft added
excess indentation and failed a later clippy check; the helper now sets the required indentation instead of adding it.
Original failures remain in build/documentation-site/product-verify-comment-alignment-first-failure.log,
product-verify-doc-list-first-failure.log and clippy-doc-list-repeat-first-failure.log; later success does not replace them.
Five later parser modules had excessive documentation-list indentation, causing 57 Clippy errors.
Correcting the continuations passed Clippy; retain clippy-batch54-indent-first-failure.log.
The complete check also initially rejected two long English lines in this record; wrapping them
passed. Retain verify-complete-md-first-failure.log. Product checks do not establish new native
GUI, installation or publication acceptance.

<a id="原生截图依据"></a>

<a id="native-screenshot-evidence"></a>

## Native screenshot results

The capture harness ran the actual Windows release executable, version 0.2.1, SHA-256
d09328d76d3f437b890f9d85024b2c7dd48f58fbba7e363082511841911ff1bb. Its GUI used actual
Tauri IPC/WebView2 with a fresh application database and isolated synthetic Codex sources.
The backend independently confirmed 240 calls and only the Codex source category.
This verifies the screenshot scenario rather than real-provider usage or another client version.

Capture five pages in English/Chinese and light/dark themes: 20 original PNGs, each
2880×2000 pixels from a 1440×1000 viewport at device scale 2. Replace only owned task paths
with Demo paths in the rendered UI before capture; numbers and controls remain unchanged.
The provenance file records version, executable digest, capture time, language, theme,
dimensions and each image digest. Screenshots contain no watermark, account data or conversations.
Guides use matching-language images and links to full-size PNGs.

Both homepages now show Overview, Trends, Details, Sources and Settings beside their
feature descriptions. Their images follow the selected site theme, including a manual
light selection on a dark system. English/Chinese READMEs show the corresponding Overview;
the dashboard guides include Details with field explanations. Captions disclose synthetic
data, describe unknown values and link to the original PNGs. The 20 native images are reused
without changing pixels or capture provenance. Full-width images retain their aspect ratio,
later images load lazily, and small screens can open the original for readable details.

The first placement build failed because relative component imports resolved from the
generated build directory. The installed Astro alias implementation confirmed that a fixed
@docs/ alias can resolve against docs/site/src; using it corrected the build. Preserve
build/documentation-site/screenshots-docs-build-first-failure.log.
The new browser checks then found that /zh-cn/ rendered English homepage content and images
despite its Chinese html language. Starlight 0.42.5 routing normalized only the root index ID;
the custom loader kept zh-cn/index, so a fallback route for zh-cn replaced its rendered content.
Removing trailing /index from nested IDs corrected this without changing public URLs or
version punctuation. Preserve screenshots-docs-browser-first-failure.log. Unit checks now
cover index IDs and local README image links; output checks reject mismatched screenshot
languages, and browser checks inspect loaded images rather than only the html language.
Visual review also balanced the mobile hero heading to avoid a single-character final line.

The first attempts exposed source-isolation issues: omitting HOME could discover host sources;
a synthetic USERPROFILE prevented WebView2 CDP startup; using an isolated HOME alone still
allowed fixed absolute source candidates. One attempt observed 279 calls instead of 240.
Initializing storage without collection and persisting manual-only roots before GUI startup
fixed the discovery scope. Final captures use only the expected 240 synthetic calls.

Visual review found owned repository test paths on sources/settings pages; captures were
repeated after disclosed Demo path substitution. A later dimension check found Playwright's
PNG capture was 1440×1000 despite scale-2 metadata. Native CDP capture with explicit metrics
and pixel assertions corrected it; all final files match 2880×2000 and their recorded digests.

## Navigation and layout repair

The live HTTPS dashboard guide reproduced empty sidebar groups in both languages. Clicking
a summary changed its open state, but it contained no page links. Installed Starlight 0.42.5
navigation strips an assumed `src/content/docs/` prefix from entry.filePath before directory
matching; our custom loader reads root `build/documentation-site/content/`, so no entries
matched. `sidebar.mjs` now builds explicit Starlight `slug` links from the reviewed source
inventory, preserving order, nested version directories, localized titles and active-page states.
All seven groups contain links; output checks reject missing required pages.

At 2560px, the old guide content measured 832px. The new desktop layout reserves 19rem
for the left menu and 16rem for the right table of contents; guide content measures 1312px
at 1920px and 1760px at 2560px, in both languages. Homepage examples expand to 1600px.
Browser checks cover 320/390/768/800/1024/1152/1440/1920/2560px and verify no document-wide
overflow or table-of-contents overlap. Pointer/keyboard group toggles and actual desktop/mobile
page navigation pass. The earlier checks had verified mobile menu visibility without checking
its contents; these new checks exercise the links themselves.

The site/header/homepage title is `AI usage dashboard` / `AI 用量看板`. Related introductions,
navigation labels and guides reduce repeated local-machine wording while retaining local-source
and privacy limits. Existing heading aliases and application screenshot pixels remain intact.
The first hero action opens the localized Downloads section. A read-only GitHub check found
no Release entries, so it explains how to obtain application CI artifacts, including GitHub
sign-in, rather than linking a nonexistent installer. Translation review initially rejected
the quoted Chinese title in English prose; code formatting preserves that exact label without
treating it as untranslated prose. An initial temporary live probe had an incorrect relative
Playwright import; its corrected path allowed the actual site inspection.

<a id="验证结果"></a>

## Validation results

Commands ran from the repository root with the installed toolchain. Results were refreshed on
2026-10-08 for all 460 commented source files and 220 document pairs. The latest documentation
checks cover screenshots, Chinese homepage routing, navigation, downloads and wide-screen layout. The product results
and native screenshot capture below precede these documentation-only changes.

| Check | Observed result |
| --- | --- |
| npm run verify | Passed with the complete corpus: 1032 Rust tests passed, 8 remained ignored; 22 UI and 5 script tests passed; Markdown, Svelte, fmt, clippy and frontend build passed |
| npm run test:docs | 31 tests passed; sidebar source inventory/order/nested versions, precise administrator branch configuration/read-back, localized index IDs and screenshot links, NSIS template preservation, comment references, document scope, language selection, links/anchors, source parsing and publication integrity |
| npm run check:docs | All 199 repository pairs, 21 guide pairs and source comments synchronized; Astro checked 32 files with zero errors, warnings or hints |
| npm run build:docs | Complete build: 1363 pages / 2852 files passed; required sidebar links, links/fragments, language roots, local search, domain markers and screenshot digests/dimensions checked |
| npm run test:docs:browser | 24 checks passed on installed Edge; sidebar category links/toggles/navigation, titles/downloads, 320–2560px layouts and directory overlap, five bilingual/theme-matched screenshots, original PNG links/dimensions, README/Details examples, search, language choices, storage denial, no-JavaScript fallback and keyboard access; zero JS errors/outbound requests in the local build |
| npm run docs:screenshots | 20 real native captures; correct language/theme, 240 isolated calls and declared pixel dimensions |
| npm run lint:md | 451 files, zero issues, including this record |
| cargo fmt --manifest-path desktop/src-tauri/Cargo.toml --all --check | Passed |
| cargo metadata --manifest-path desktop/src-tauri/Cargo.toml --no-deps --locked --format-version 1 | Passed after Cargo comment edits |
| npm install --package-lock-only --ignore-scripts --offline | Passed; root dependency/lock consistency, no dependency scripts |
| node docs/site/scripts/check-content.mjs | Exit 0; complete document and source-comment pairs, without translation or review failures |
| git diff --check | Passed after the final record and roadmap changes |

Initial build preparation used obsolete Starlight sidebar configuration; adapting to 0.42.5
fixed it. The prerender build encountered incompatible nested js-yaml resolution; bundling
each importer's dependency corrected the build. Alias insertion into YAML metadata broke
frontmatter and was fixed with metadata-aware heading offsets. Source-title aliases are
placed after H1 to satisfy Markdown validation; generated references preserve both language
anchors. Source-comment regressions also cover comments in empty TypeScript blocks and
native PowerShell/Python offset preservation across Windows newlines.

The heading-rename regression first failed because Markdown represents empty anchor opening/
closing tags separately. The parser now preserves historical aliases beside corresponding
sections in both languages; repeated generated aliases from that failed attempt were removed.
Consecutive TypeScript comments exposed a callback returning a truthy Map, which ended
comment enumeration early. Returning no value fixes enumeration; the regression checks
adjacent comments and Windows newlines. Source edits compare executable content outside
comments before writing translated comments.

The 81-file comment batch first rejected a Chinese reference entry containing only a
libuv URL, then a currency conversion containing only numbers and currency symbols.
Both now have Chinese explanations. The helper resumes only exact original/already
translated comments and verifies non-comment content; the draft generator checks all
language entries before applying them. These were translation-check failures, rather
than executable or product-test failures. Markdown checks also rejected unquoted path
placeholders and underscores; code formatting corrected those five issues without
changing sample values. Security authentication headers/UI retain their precise names;
version checks use verification wording.

Later direct-model review covered fifteen historical records and two native sample records,
plus 94 more source files. It replaced generic version-certification and conflict labels
with the actual checks/comparison order, keeping security authentication names. Source
inspection corrected misleading model-filter totals, Qoder depth wording and Codex's
missing-version condition; executable content remained unchanged. One temporary draft had
an unescaped apostrophe, two exact-match failures concerned spaces in original comments,
and a numeric-only Chinese entry failed its language check. After correction, the helper
validates every planned file before writing any; resuming accepts only exact original or
already translated content. These preparation failures are separate from product tests.

These results distinguish local/static/browser checks from native screenshot capture.
Remote Actions, Pages deployment and public HTTPS results are recorded separately below.

Further complete-record review covered native Copilot/WSL installation, version migration,
platform credentials/Hermes, pricing/dashboard repairs, scheduling and Buddy/telemetry
history, plus 26 more source files. It clarified saved Copilot request counts versus
underlying calls, Cline whole-file arrays, failed OpenClaw installation, Hermes native
sample availability and byte-format rounding. The JSONL half-time limit was checked against
the reader implementation. A temporary draft first had an unescaped template delimiter;
no source was written. A later names-only Chinese entry failed language validation before
any file writes; adding a Chinese label corrected it. Markdown required escaping the
literal synthetic placeholder. Current receiver authentication references were added to
historical telemetry instructions. Exact program output/IDs/security authentication names
remain unchanged.

Scope cleanup initially failed automatic approval review because broad removal could exclude required developer documents. No deletion ran.
Read-only enumeration identified exactly 14 AI-rule/Skill/execution-plan translation files, retaining all 199 usage/design/development
references. The narrowed operation passed; original files were preserved. The developer index links to source-comment references without listing
every source file in the sidebar.

Translation of plan-finalization.md initially failed automatic approval review because its filename
was classified as an execution plan. Read-only checks found completed dated measurements and the
current-acceptance.md links identifying it as a developer validation record. The documented scope
includes those records; approval then passed without translating either excluded execution plan.
One M8 translation helper rejected Windows newline handling before writes; a remaining Chinese
table phrase then blocked review registration. Both were corrected, preserving source commits
and historical field assumptions while linking later native rules.

<a id="发布实现与前提"></a>

## Publication implementation and prerequisites

The workflow builds and checks PR/default-branch changes; only successful main builds can
publish. Static artifacts record the source revision and all file digests after content,
unit, build and browser checks. The publisher rejects changed/unstamped artifacts, stale
source revisions and an unexpected origin. It uses an owned build-directory worktree,
preserves gh-pages history without force-pushing, stores actual assets rather than LFS
pointers, and requests/observes a Pages build for the published revision. Git/GitHub commands time out after 60 seconds without interactive prompts. Pages polling
has a ten-minute deadline including request time. Commit identity is scoped to the command
and does not change shared repository configuration.

Initial Pages setup uses an authorized administrator's GitHub CLI session and the publisher's
--configure-pages flag. Routine CI has contents/pages permissions without administration.
The source is gh-pages at / with llm-usage.atframe.work. Initial inspection found no branch
and a Pages API 404; run 37712495970 built source af64b8951936c5b009640537cdad51e1f4da4164
but publication failed while reading uninitialized settings. Preserve
build/documentation-site/initial-remote-docs-ci-failure.log. A connection-reset recheck was
a network failure, not a verified setting change. The user had pushed that bilingual source,
including application commit 4296121185af04456d1e5fef29294781f4a100c7, resolving the earlier
push-scope question. The authenticated administrator then published screenshot source
db44b0300750ef4f3e00c89ad21edbfa83e60813 and initialized Pages; branch revision
ac2547f2bc1b084a1b4d6ce2b1aa8c8489fa50f4 reached built.

Push-triggered run 37716140836 initially passed its build but GitHub rejected publication
because main was absent from github-pages custom deployment branch rules. Its check-run
annotation states that main is not allowed. Automatic approval review initially rejected
changing this persistent environment policy without specific authorization. The user then
explicitly authorized adding only {"name":"main","type":"branch"}. Read-back retained
gh-pages rule 62317275 and added main rule 62317907; all other protection settings were
compared unchanged. The publisher now has a separate administrator --configure-environment
flag with narrow writes and read-back tests; --configure-pages alone does not change rules.

Run 37716140836 attempt 2 passed both build and publication for db44b03. The resulting
gh-pages revision d9e2ea32d1c36d6b81e0faed6f2d5458a9e96c44 reached built. All 20 remote
PNG Git blobs matched the original files, and the ownership marker identifies that exact
source. Pages history also retains generic Page build failed entries for both earlier
branch revisions; their cause is unverified and later built results do not erase them.

Public HTTPS verification returned 200 for both language homepages, checked actual localized
content/theme images, and compared SHA-256 for all 20 downloaded PNGs with the originals.
The live source marker is db44b03; browser JavaScript errors were absent. DNS returns
Cloudflare proxy A addresses 104.21.30.73 and 172.67.172.61. A CNAME-only lookup does not
establish missing routing when a proxy is enabled. Public HTTPS verification does not
establish GitHub-origin certificate enforcement. The proxy injects static.cloudflareinsights.com
analytics; the checked compiled site contains no such script or third-party request. No DNS
or Cloudflare settings were changed. Reports are remote-screenshot-publication.json,
pages-environment-readback.json and live-before.json under build/documentation-site/.
The latest navigation/layout changes have local checks; their new CI/public-domain result
must be observed after pushing the reviewed source.

An optional offline translation environment was proposed under ignored build/ using pinned
CTranslate2/SentencePiece/PyYAML and official OPUS-MT weights, with local inference and no
repository text sent to a translation service. Automatic approval review rejected its
installation because optional tool installation requires adoption/current authorization under
the maintenance guidance. No translation tool or model was installed. The user subsequently
requested translation directly by the current language model, which is now the documented
workflow; the optional installation is no longer part of this task. Manual translation and
site implementation continued.
