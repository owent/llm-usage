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

Build preparation imports the user, architecture and development documents into reference categories and rewrites
document/source links through Markdown syntax nodes. It leaves fenced examples intact and
preserves version punctuation in routes. Generated reference pages have no separate editable copy.

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
After the content, unit, production-build and browser checks pass on a clean source commit,
run `node docs/site/scripts/stamp-output.mjs` to record the source revision and output digests.
The publisher rejects unstamped output, changed assets and a revision that differs from current
`main`. The workflow performs this step before uploading its checked artifact.

The custom domain is `llm-usage.atframe.work`, with a DNS CNAME to `owent.github.io`.
GitHub Pages settings, DNS and HTTPS must be checked separately. A domain file alone is
not a successful custom-domain deployment. Keep failed-build output out of the live branch.
To recover, rebuild a known-good source revision and republish it; do not force-push
another branch or delete application data.
