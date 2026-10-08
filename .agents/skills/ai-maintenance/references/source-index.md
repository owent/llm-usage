# Source index

<a id="来源索引"></a>

<a id="fields"></a>

<a id="字段与维护"></a>

## Fields and maintenance

Every fact records claim/scope, source_url/source_version, verified_at/method, installed_version/status,
review_cadence/update_trigger and impact/owner. Tables below share verified_at = 2026-09-24 and
owner = repository maintainers. Recheck before relevant work and review still-used changing facts quarterly.
Triggers are upgrades, deprecations, security notices, loading failures and observed behavior changes.
Do not automatically upgrade tools. Update current records; Git preserves history.

<a id="当前采用的事实"></a>

## Currently adopted facts

| ID | claim / scope | source_url / source_version | method / installed_version / status | impact |
| --- | --- | --- | --- | --- |
| S01 | Codex root-to-cwd rule chain, override priority, default 32 KiB | [Official rules](https://learn.chatgpt.com/docs/agent-configuration/agents-md), rolling | Full text; CLI 0.155.0-alpha.16.3; documentation verified, runtime in validation records | AGENTS, clients |
| S02 | Codex shared Skills, upward discovery, no same-name merge, implicit invocation enabled by default | [Build skills](https://learn.chatgpt.com/docs/build-skills), rolling | Full text; same version as S01; documentation verified | Skill routing, clients |
| S03 | Mandatory Skill fields, name/description limits, optional fields and progressive loading | [Specification](https://agentskills.io/specification), rolling | Full text; local static validation recorded separately | maintenance, SKILL |
| S04 | Positive/negative near-match triggers, real read traces, repetitions and held-out sets | [Description evaluation](https://agentskills.io/skill-creation/optimizing-descriptions), rolling | Full text; no installed version applies; documentation verified | skill-evaluation |
| S05 | Identical inputs, Skill/no-Skill/old-Skill comparison and output quality | [Output evaluation](https://agentskills.io/skill-creation/evaluating-skills), rolling | Full text; actual model evaluation unexecuted | skill-evaluation |
| S06 | Kilo rules/Skills verified per product, without inferring all clients match | [Rules](https://kilo.ai/docs/customize/agents-md), [Skills](https://kilo.ai/docs/customize/skills), rolling | Full text; VS Code extension 7.7.9; documentation verified, loading unverified | clients |
| S07 | markdownlint-cli2 local dev dependency, JSONC and CLI globs | [Maintainer README](https://github.com/DavidAnson/markdownlint-cli2), main/rolling | Full text/npm metadata; 0.23.3, Node >=22; runs recorded separately | package, lint configuration |
| S08 | rg default ignores/hidden files and search/enumeration choice | [Maintainer GUIDE](https://github.com/BurntSushi/ripgrep/blob/master/GUIDE.md), master/rolling | Full text/rg --version; 15.2.0; version/search verified | terminal-tools |
| S09 | jq -e distinguishes false/null from no-result exit codes | [Manual](https://jqlang.org/manual/), 1.8 | Full text/jq --version; 1.8.2; actual codes recorded separately | terminal-tools |
| S10 | Mike Farah yq implementation identity/YAML queries | [Maintainer README](https://github.com/mikefarah/yq), master/rolling | Full text/yq --version; 4.53.6; actual parsing recorded separately | terminal-tools |
| S11 | PowerShell native argument modes, quotes and --% limits | [Parsing](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_parsing?view=powershell-7.5), 7.5 docs | Full text; local 7.6.6/Windows mode; no claim that all options were tested | terminal-tools |
| S12 | PowerShell default encoding varies by version/command | [Encoding](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_character_encoding?view=powershell-7.5), 7.5 docs | Full text; local 7.6.6; new files specify UTF-8 | terminal-tools |
| S13 | Claude native version/session requirements, four-hop imports, path rules/loading checks | [Memory](https://code.claude.com/docs/en/memory), rolling | Full text; client absent from PATH; documentation verified, runtime unverified | maintenance, clients |
| S14 | VS Code Local/Agent Host prompt-file support differs; Agent Host migrates to Skills | [Prompt files](https://code.visualstudio.com/docs/agent-customization/prompt-files), rolling | Full text; VS Code 1.139.0; actual session type unconfirmed | maintenance, clients |

Root/desktop manifests and lockfiles have been restored. Current files govern versions/commands;
initial snapshots do not establish current state. Independent product sources/versions are in
[research](../../../../docs/design/desktop-usage/research.md), not duplicated in the AI compatibility table.
Rolling main branches are not lockfile versions. OpenAI rule-page .md endpoints failed; successfully
opened HTML text was used instead. Failed responses are not verified source information.

<a id="条件流程与未核验候选"></a>

## Conditional workflows and unverified candidates

The full tool catalog came from user input; original attachment hashes/chapter mappings are in
[coverage](records/initialization-coverage.md). Outside S08–S12/local probes, candidates retain purposes
only, without verified version/download/safety/performance claims. Unused products in
[clients](clients.md) likewise retain official entry points awaiting checks.

OpenSpec, Superpowers, MCP and ClawHub are not adopted. Do not copy template latest/current assertions.
Their workflows are conditional project design constraints. Before enabling, verify versions, fields,
authorization and implementation at official entry points, then add factual records:

- [OpenSpec commands](https://github.com/Fission-AI/OpenSpec/blob/main/docs/commands.md),
  [Superpowers releases](https://github.com/obra/superpowers/releases).
- [MCP specification](https://modelcontextprotocol.io/specification).
- [ClawHub API](https://docs.openclaw.ai/clawhub/api),
  [security audits](https://docs.openclaw.ai/clawhub/security-audits).
- [OWASP secrets management](https://cheatsheetseries.owasp.org/cheatsheets/Secrets_Management_Cheat_Sheet.html),
  [GitHub Actions security](https://docs.github.com/en/actions/reference/security/secure-use),
  [AWS idempotent retries](https://aws.amazon.com/builders-library/making-retries-safe-with-idempotent-APIs/).

These candidate pages were not individually verified this round; they do not establish completed
integration, security review or production acceptance.

<a id="本地写作参考"></a>

## Local writing references

On 2026-09-24, read `.agents/skills/ai-agent-maintenance/references/writing-guidance.md` in both
workspaces below and adapted their methods for this repository. Runtime does not depend on outside paths.
HEAD identifies the repository snapshot; file hashes identify actual workspace contents read,
without assuming they match HEAD exactly.

| Repository | HEAD at the time | File SHA-256 |
| --- | --- | --- |
| atframework/AICodeReviewer | 782fb752f1a588a2cc99ec29e9e037860ed113bb | CCD79F3D50BDDB0334D3319B2203D58F8A48DD86DD010F71E015763D659D4309 |
| atframework/atsf4g-co | 1f9aea233bdd3e58a362e7127fe8665b2928a741 | A71879408C0C3C535C5740D03BDEEF8897D147E01D8EA193218DD226F702DD15 |

Adopted: facts first, concrete verbs, stable terms, paragraph structure, Chinese/English empty-prose
review and technical exceptions. Not adopted: other projects' paths, website banned-word scripts,
historical-record rewriting or AI-author inference by word frequency. Maintainers own review after
guidance changes, actual misclassification or sample changes. External webpages in original references
were not reverified this round; their old verification dates are not this round's dates.
