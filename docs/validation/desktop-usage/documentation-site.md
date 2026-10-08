# Documentation site implementation record

<a id="文档站实施记录"></a>

Started: 2026-10-07. Review date: 2026-10-08. Status: all document and source-comment pairs
have been reviewed and complete local checks passed; remote publication remains unfinished. [Plan.md](../../../Plan.md) owns active status;
the [documentation requirements](../../design/documentation-site.md) define behavior.

<a id="环境与范围"></a>

## Environment and scope

Working directory: D:/workspace/projs/github/owent/llm-usage. Windows, PowerShell 7.6.6,
Node 24.21.0 and Git 2.56. Astro 7.3.6 and Starlight 0.42.5 are locked at the root.
The documentation has an English root and /zh-cn/ Chinese routes, browser-language
negotiation with explicit preference, system/light/dark themes and local Pagefind search.
No third-party browser requests were observed in the browser checks.

The user authorized documentation publication, including gh-pages and the custom domain.
Earlier application repairs remain separate local changes; no commits, pushes or remote
publication were performed for this documentation work. Temporary logs, databases, generated
content and previews stay under build/documentation-site/.

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
The repository inventory is now 538 source files, including 460 files with 7939 actual comments.
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

The first attempts exposed source-isolation issues: omitting HOME could discover host sources;
a synthetic USERPROFILE prevented WebView2 CDP startup; using an isolated HOME alone still
allowed fixed absolute source candidates. One attempt observed 279 calls instead of 240.
Initializing storage without collection and persisting manual-only roots before GUI startup
fixed the discovery scope. Final captures use only the expected 240 synthetic calls.

Visual review found owned repository test paths on sources/settings pages; captures were
repeated after disclosed Demo path substitution. A later dimension check found Playwright's
PNG capture was 1440×1000 despite scale-2 metadata. Native CDP capture with explicit metrics
and pixel assertions corrected it; all final files match 2880×2000 and their recorded digests.

<a id="验证结果"></a>

## Validation results

Commands ran from the repository root with the installed toolchain. Results were refreshed on
2026-10-08 for all 460 source files and 220 document pairs. After adding publication timeouts,
content, type and unit checks passed again; that change does not alter static pages or browser behavior.

| Check | Observed result |
| --- | --- |
| npm run verify | Passed with the complete corpus: 1032 Rust tests passed, 8 remained ignored; 22 UI and 5 script tests passed; Markdown, Svelte, fmt, clippy and frontend build passed |
| npm run test:docs | 22 tests passed; NSIS template preservation, empty-comment references, document scope, language selection, Markdown links/anchors, source parsing, Pages configuration and publication integrity |
| npm run check:docs | All 199 repository pairs, 21 guide pairs and source comments synchronized; Astro checked 29 files with zero errors, warnings or hints |
| npm run build:docs | Complete build: 1363 pages / 2851 files passed; links/fragments, language roots, local search, domain markers and screenshot digests/dimensions checked |
| npm run test:docs:browser | 12 checks passed on installed Edge; English/Chinese search, themes, root negotiation, explicit deep links, storage denial, no-JavaScript fallback, keyboard and mobile layout; zero JS errors/outbound requests |
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

These are local/static/browser/native screenshot results. Remote Actions, Pages deployment,
DNS/HTTPS have not passed acceptance.

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
The configured source is gh-pages at / with llm-usage.atframe.work. The initial read-only
inspection found no gh-pages branch, a Pages API 404 and no domain CNAME/A response.
A later GitHub recheck encountered connection resets; this is a network failure rather than
a verified change to remote settings. Read-only checks succeeded again on 2026-10-08: remote main is
dda555f22b3092e444be19db3467b14159fcdd59; gh-pages is absent and Pages still returns 404.
The authenticated user has repository administration permission. Local main has an additional
unpushed application commit, 4296121185af04456d1e5fef29294781f4a100c7. Its push scope awaits
user clarification; no remote writes were performed. Cloudflare hosts authoritative DNS,
but the required CNAME is still absent and no DNS-editing connection for that zone is available.

DNS needs a CNAME for llm-usage.atframe.work pointing to owent.github.io. Verify DNS and
HTTPS separately after configuring Pages; a CNAME file does not establish either.

An optional offline translation environment was proposed under ignored build/ using pinned
CTranslate2/SentencePiece/PyYAML and official OPUS-MT weights, with local inference and no
repository text sent to a translation service. Automatic approval review rejected its
installation because optional tool installation requires adoption/current authorization under
the maintenance guidance. No translation tool or model was installed. The user subsequently
requested translation directly by the current language model, which is now the documented
workflow; the optional installation is no longer part of this task. Manual translation and
site implementation continued.
