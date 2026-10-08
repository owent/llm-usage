---
title: Documentation maintenance
description: Maintain complete translations and verify the published site.
sidebar:
  order: 6
---

## Content ownership

English is the primary language for user, architecture and development documentation. Complete Chinese counterparts
live in `docs/zh-CN/<repository-relative-path>`. User/developer guides live in
`docs/site/content/`, with identical page paths under `zh-cn/`. Maintain the English original
and its Chinese counterpart; do not edit generated reference pages in `build/`.
AI rules, `.agents/skills/`, `Plan.md` and `docs/design/desktop-usage/execution.md` retain
their original files without translations or generated site pages.

`Plan.md` owns active status. Desktop design specifications own behavior, `adapters.md` owns
support scope, `research.md` owns versioned product sources, and validation records own
commands, measurements and failures. Translate the complete record without changing its
numbers, identifiers, unknown states, first failure or validation scope. Translate directly
with the current language model and compare the complete pair. Review wording in both
languages using the [writing guidance](https://github.com/owent/llm-usage/blob/main/.agents/skills/ai-maintenance/references/writing-guidance.md).

Maintain English source comments and their Chinese indexed counterparts together. Runtime
catalog values, literal test data, original-language source material and third-party license text
are not prose to rewrite blindly. Preserve exact executable content in comment-only work.

## Add a page

Give authored pages a title, concise description and sidebar order. Add the Chinese page
with the same filename and appropriate localized links. Keep English and Chinese screenshots
separate. Use meaningful alt text and explain synthetic screenshot provenance.

On MDX pages, import `@docs/components/AppScreenshot.astro` and pass a page name (`overview`, `trend`,
`details`, `sources` or `settings`), the matching language (`en` or `zh-CN`) and localized
alt text. Put the caption inside the component. It selects the light/dark original with
the site's theme and links to the full-size PNG. Use `eager` for the leading example;
later images load lazily. Keep examples full-width so controls and labels have space.
Markdown guides can link a localized PNG directly; relative screenshot links in repository
READMEs become local `/screenshots/` links when those documents are published on the site.
The `@docs/` alias resolves against `docs/site/src/` even after build preparation copies
the authored page into `build/`; relative component imports would resolve from that copy.

Build preparation imports the user, architecture and development documents into reference categories and rewrites
document/source links through Markdown syntax nodes. It leaves fenced examples intact and
preserves version punctuation in routes. Generated reference pages have no separate editable copy.
The content loader removes a trailing `/index` from nested page IDs. This makes
`zh-cn/index.mdx` the Chinese language root (`zh-cn`) and prevents Starlight from
creating an English fallback at the same URL. The root `index.mdx` retains ID `index`,
which Starlight normalizes itself. Version punctuation in all other IDs is preserved.

`scripts/sidebar.mjs` reads the authored English page metadata and repository document
inventory, preserving page order and nested version directories. It supplies Starlight
`slug` links, which resolve localized titles and active-page states. Starlight 0.42.5's
directory autogeneration assumes `src/content/docs/`; our generated files live under root
`build/`, so that mode produces empty categories. Do not restore it without checking
actual menu links. Output checks reject missing required links, and browser checks open
all seven categories and navigate with pointer and keyboard on desktop/mobile screens.

Wide-screen layouts reserve fixed space for the left menu and right table of contents
and expand the remaining content up to 110rem (100rem on the homepage). Check responsive
boundaries as well as 1920px and 2560px screens. The homepage Download action and user
download links target `https://github.com/owent/llm-usage/releases/latest`.
The localized installation guide covers package selection and runtime requirements.

Keep theme colors in `src/styles/custom.css`: page, reading area, card, sidebar and border
colors have separate `--usage-*` variables, while Starlight variables supply text and actions.
The header and hero use a shared navy palette in both themes. Static CSS dots are 48px apart
in outer margins and 40px apart in the illustration area; circular decoration has no pointer
events. Reading areas and captions keep opaque backgrounds. Forced colors and printing hide
decorations. Avoid introducing animated backgrounds or external image/font downloads.

The browser suite measures actual rendered text against composited computed backgrounds on
both homepages, the dashboard guide and this developer page in both themes. It checks keyboard
focus and hovered actions separately, saves measurements beside viewport screenshots, and
rejects text over CSS background images that require a different measurement. Review the
English/Chinese screenshots, mobile layouts and wide-screen reading areas as well as the ratios.
These sampled checks do not replace a complete accessibility audit.

The translation review manifest stores the checked English/Chinese hashes. Missing pairs
or changes without corresponding review fail the content check. Check both languages for
meaning and completeness before updating a review entry; matching hashes alone do not
establish translation quality.

## Validate and capture

```powershell
npm run check:docs
npm run test:docs
npm run build:docs
npm run test:docs:browser
npm run lint:md
git diff --check
```

For updated UI screenshots, first build the desktop release and run `npm run docs:screenshots`.
The capture harness initializes isolated storage, saves manual-only roots before the GUI
starts, checks 240 synthetic calls through actual IPC, and captures English/Chinese in
both themes. Review images visually, check provenance and commit actual LFS assets.
Screenshots are interface examples, not real-provider/version acceptance.

## Publish and recover

The docs workflow checks and builds before publication. Default-branch documentation
changes publish the compiled root to `gh-pages`, keeping history, `CNAME` and `.nojekyll`.
PRs cannot publish. The Pages deployment step is explicit because a `GITHUB_TOKEN` push
alone does not start a branch-based Pages build.

The first publication uses the publisher's `--configure-pages` flag with an administrator's
GitHub CLI session after all checks pass. Creating or changing Pages settings requires
administration permission in addition to Pages permission; ordinary CI does not receive
administration permission. Later builds verify the configured branch, root and custom domain
before updating the live branch.
The separate administrator flag `--configure-environment` adds the exact `main` branch to
`github-pages` when that deployment environment uses custom branch rules. It retains existing rules and other protections, and
reads the resulting rules back. The documentation publisher runs on `main`; Pages builds
the compiled `gh-pages` branch. Routine CI keeps its existing contents/pages permissions.
Use that flag only with authorization to change deployment branch rules; `--configure-pages`
alone does not change them. See GitHub's
[deployment branch policy API](https://docs.github.com/en/rest/deployments/branch-policies?apiVersion=2026-03-10).
After the content, unit, production-build and browser checks pass on a clean source commit,
run `node docs/site/scripts/stamp-output.mjs` to record the source revision and output digests.
The publisher rejects unstamped output, changed assets and a revision that differs from current
`main`. The workflow performs this step before uploading its checked artifact.

The custom domain is `llm-usage.atframe.work`, with DNS routing to `owent.github.io`.
An enabled DNS proxy can expose its A/AAAA addresses rather than the underlying CNAME.
GitHub Pages settings, DNS and HTTPS must be checked separately. A domain file alone is
not a successful custom-domain deployment. Keep failed-build output out of the live branch.
To recover, rebuild a known-good source revision and republish it; do not force-push
another branch or delete application data.
