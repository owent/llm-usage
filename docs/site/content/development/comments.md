---
title: Source comments and translations
description: Maintain English source comments with complete Chinese references.
sidebar:
  order: 7
---

Source comments use English. Their Chinese counterparts live in
`docs/source-comments.json`, indexed by repository path, stable identifier and comment order, and are
published as [source comment references](/reference/comments/). The generated reference
links to the current source line. Application interface strings, translation catalogs,
native test samples and third-party license notices retain their original content.

## Review a change

Read the surrounding implementation and the Chinese reference before editing a comment.
Update its English text and the matching Chinese entry together. Keep versions, identifiers,
units, unknown-value rules, failure paths and coverage limits intact. A comment explaining
an unverified scenario must retain that qualification in both languages.

The registry records the comment kind, English text, Chinese text and a digest of executable
content with comments removed. Checks use TypeScript syntax, Rust string/comment rules,
YAML concrete syntax, Python tokens/docstrings and PowerShell's native parser to avoid
mistaking runtime strings for comments. Dependencies and technical directives remain valid.

## Validate the result

Run `npm run check:docs` for pairing and registry consistency, `npm run test:docs` for
parser regressions, and the relevant application checks for the changed files. Run
`npm run build:docs` and `npm run test:docs:browser` for generated references and navigation.
A registry update records a reviewed translation; it does not establish runtime acceptance.

After reviewing a source file, save one Chinese string per comment in a JSON array under
`build/documentation-site/`, in source order, then record the pair:

```powershell
npm run docs:comments:review -- desktop/src-tauri/build.rs --translations build/documentation-site/comments-build.json
```

The command verifies the English comments, parses current source positions and updates the
registry with the reviewed Chinese strings. It keeps the source file unchanged. For repository
documents or authored guides, use `npm run docs:review -- <English-file>` after reviewing both editions.

Generated pages belong under `build/documentation-site/content/`. Edit source comments
and the registry rather than generated pages. For the full publication workflow, see
[documentation maintenance](/development/documentation/).
