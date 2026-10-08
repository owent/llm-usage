# Documentation site and language requirements

<a id="documentation-site-and-language-contract"></a>

<a id="文档站与语言要求"></a>

<a id="文档站与语言合同"></a>

Status: published; navigation and layout improvements are verified. The existing desktop specifications remain authoritative
for application behavior; [Plan.md](../../Plan.md) owns current work and acceptance conditions.

<a id="内容与归属"></a>

## Content and ownership

English is the default language for user, architecture and development documentation and
source comments. These documents have complete Simplified Chinese counterparts under
`docs/zh-CN/<repository-relative-path>`. Development references include field specifications,
implementation checks, expected test results and asset instructions. AI rules, `.agents/skills/`,
`Plan.md` and the delivery plan in `docs/design/desktop-usage/execution.md` keep a single
maintained original and are excluded from translation and generated site pages.
Translations preserve version identifiers, coverage IDs, quantities, commands, sources,
failures, unknown values and the distinction between historical and current results.
Original-language quotations and runtime examples retain their exact content when it is
needed to explain or reproduce behavior. Application translation catalogs remain intact.

The English file is the primary maintained version. Both languages change together; a checked translation
manifest records their hashes. Missing counterparts and unreviewed changes fail the docs
check. Relative document links resolve to the matching language; source and asset links
resolve to the actual repository file. Existing explicit anchors remain stable.

Source comments use English. Their Chinese counterparts are indexed in the Chinese
developer documentation by repository path and stable comment identifier. Comment-only
edits must preserve runtime strings, test data, licenses and executable behavior.

The public guide has paired English and Chinese pages in `docs/site/content/`. It covers
installation, first collection, dashboard interpretation, sources, supported clients,
telemetry, scheduling, estimates, usage/cost alerts, retention, exchange, privacy, troubleshooting,
accessibility, development, architecture, data rules, adapters, testing, releases and
documentation maintenance. Existing specifications and validation records are published as reference
pages from their authoritative files rather than maintained as another editable copy.

Translate each complete document directly with the current language model. Compare the
source and translation before registering a reviewed pair, and apply the project's
[writing guidance](../../.agents/skills/ai-maintenance/references/writing-guidance.md)
to both languages. State the actual file, field rule, check or result instead of vague
engineering shorthand. Review headings, navigation, cards, buttons and captions too.

<a id="站点行为"></a>

## Site behavior

Build a static Astro site with Starlight. English uses `/`; Chinese uses `/zh-cn/`.
An initial visit to the root selects the first supported browser language, with English
as the fallback. A user's explicit language choice takes precedence and is persisted
locally. Shared deep links retain their explicit language. Language switching preserves
the corresponding page, query and fragment; inaccessible local storage does not break
navigation. Without JavaScript, English remains the default root document.

Use Starlight's accessible navigation, local search, table of contents and theme controls,
with project branding and responsive typography. System, light and dark themes are
supported. Navigation, search labels, content and screenshot captions use the selected
language. No analytics, remote fonts, login or third-party runtime requests are required.

Use `AI usage dashboard` / `AI 用量看板` for the site and homepage titles. Introductions
describe tokens, calls, cache usage and trends; explain data storage and local-source limits
in the privacy and source instructions. The homepage's leading Download action opens the
matching-language installation/download section. Link existing build artifacts when no
Release packages have been published, and state their sign-in requirement.

Wide screens expand the main content area, including tables and screenshots. Keep the
left menu and right table of contents independently usable; avoid empty navigation groups
and content overlap. Generate localized Starlight `slug` links from the reviewed source
inventory rather than its assumed content directory. Verify actual category links, expansion,
keyboard navigation, active-page indication and mobile navigation in both languages.

<a id="截图"></a>

## Screenshots

Capture the actual desktop application with isolated synthetic sources and application
data. Do not expose local user paths, credentials, conversations or account details.
Capture English and Chinese separately after selecting the corresponding application
language. Replace owned test paths with Demo paths in the rendered interface before capture,
without changing numbers or controls, and record the anonymization in provenance. Preserve
original PNG pixels, readable labels and useful example data; no watermark
or AI-generated reconstruction. Record application version, viewport, theme, language,
data provenance and checks. Screenshots illustrate the UI, not real-provider acceptance.

The homepage pairs feature descriptions with full-width screenshots of Overview, Trends,
Details, Sources and Settings. Use the page's language and the selected site theme, including
an explicit theme that differs from the system setting. Keep the original image aspect ratio,
provide localized alt text and captions, and link to the original PNG for readable details on
small screens. Load later screenshots lazily. Repository READMEs show a localized Overview
example; the dashboard guide includes a Details example beside its field explanation.
Disclose synthetic data beside the images. Browser checks verify image loading, language,
theme switching, original-image links, keyboard access and layout at narrow widths.

<a id="构建与发布"></a>

## Build and publication

Root npm commands own dependency installation, development, checks and builds. Generated
content, preview output and task artifacts live under the repository-root `build/`.
Only authored content, configuration, scripts and approved screenshots are tracked.

The `gh-pages` branch stores the compiled static site at its root, including `CNAME` for
`llm-usage.atframe.work` and `.nojekyll`. A dedicated workflow checks and builds on changes
to documentation, translation inputs, site tooling and publishing configuration, and
publishes only successful builds from the default branch. Pull requests can check and
preview the build without publication permission. Deployment concurrency is serialized;
publishing preserves branch history and does not force-push other branches.

Git and GitHub commands time out after 60 seconds without interactive prompts. Pages build
polling has a ten-minute deadline, including request time. After a timeout, inspect the
actual remote branch/build before retrying a write. CI jobs also have fifteen-minute limits.

The separate administrator flag `--configure-environment` checks the `github-pages`
deployment environment.
If it uses custom branch rules, add the exact `main` branch required by the documentation
publisher and read the rules back. Preserve every existing branch/tag rule and all other
environment protections. Routine CI does not change these settings or receive administration
permission. Pages itself continues to build the compiled `gh-pages` branch.

Configure GitHub Pages and its custom domain through the supported GitHub API. A DNS
CNAME from `llm-usage.atframe.work` to `owent.github.io` is configured separately. A DNS
proxy may return A/AAAA addresses instead of exposing its CNAME; inspect actual DNS and
HTTPS responses before reporting a missing record. The presence of a `CNAME` file alone
does not establish DNS or HTTPS. Report build, branch
publication, Pages deployment, DNS and HTTPS as separate verified states.

<a id="验收"></a>

## Acceptance

- Complete English/Chinese coverage of user, architecture and development documents and source comments.
- No translated copies or generated site pages for AI rules, Skills or execution plans.
- Link, anchor, manifest, screenshot-language and generated-output validation.
- Successful static production build with usable local search in both languages.
- Browser checks for browser-language selection, explicit choice, deep links, storage
  failure, light/dark themes, desktop/mobile layout, keyboard navigation and broken links.
- Actual English and Chinese desktop captures with their recorded provenance.
- Appropriate application checks for comment-only source edits and `git diff --check`.
- A real `gh-pages` publication and observed Pages deployment; DNS and HTTPS verified
  independently, or an exact outstanding DNS prerequisite reported.

<a id="已核验实施依据"></a>

## Verified implementation sources

Retrieved on 2026-10-07:

- [Starlight localization](https://starlight.astro.build/guides/i18n/): localized content
  paths, navigation and UI; missing translated content falls back, which our coverage
  check must prevent for required pages.
- [Starlight component overrides](https://starlight.astro.build/reference/overrides/):
  extending page head and language controls while preserving theme/navigation behavior.
- [Astro internationalization](https://docs.astro.build/en/guides/internationalization/):
  request-based preferred-language APIs require on-demand rendering; the static site uses
  browser language selection at its entry point.
- [Astro GitHub Pages deployment](https://docs.astro.build/en/guides/deploy/github/).
- [GitHub Pages publishing sources](https://docs.github.com/en/pages/getting-started-with-github-pages/configuring-a-publishing-source-for-your-github-pages-site).
- [GitHub Pages REST API](https://docs.github.com/en/rest/pages/pages).
- [GitHub custom domains](https://docs.github.com/en/pages/configuring-a-custom-domain-for-your-github-pages-site/managing-a-custom-domain-for-your-github-pages-site).

Retrieved on 2026-10-08:

- [Starlight sidebar navigation](https://starlight.astro.build/guides/sidebar/): explicit
  `slug` links obtain translated page titles and target the corresponding locale.
- [Starlight styling](https://starlight.astro.build/guides/css-and-tailwind/): content-width
  variables and custom CSS. Installed 0.42.5 source defines the actual layout and sidebar assumptions.
- [GitHub artifact downloads](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/download-workflow-artifacts): sign-in and read access are required.
- [GitHub deployment branch policies](https://docs.github.com/en/rest/deployments/branch-policies?apiVersion=2026-03-10).
- [Cloudflare DNS proxy behavior](https://developers.cloudflare.com/dns/proxy-status/):
  proxied records expose Cloudflare addresses rather than the origin address.

The registry reports Astro 7.3.6 and Starlight 0.42.5. Their documented Node engine and
peer ranges will be checked against the installed lockfile; this does not upgrade the
desktop application's dependencies.
