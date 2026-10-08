# Maintenance validation records

<a id="维护验证记录"></a>

This page preserves initialization/migration result snapshots. Old states do not describe current
engineering. Root AGENTS.md owns current rules/commands; see [latest acceptance](../../../../../docs/validation/desktop-usage/current-acceptance.md)
for current product/rule checks. Historical docs/ai command paths were real then, not current entry points.

<a id="环境与证据范围"></a>

<a id="环境与验证范围"></a>

<a id="environment-and-evidence-scope"></a>

## Environment and validation scope

Date: 2026-09-24; cwd: D:/workspace/github/owent/llm-usage.
Initially Git main had no commits/tracked files; only Kilo ignore configuration existed, without product
code, builds, tests, CI or Plan. No AGENTS.md was found from parents to drive root. Existing .kilo/.gitignore
was preserved. Versions/fallbacks: [terminal records](../terminal-tools.md); clients: [compatibility](../clients.md).

That task authorized repository maintenance/local static verification, without user-configuration writes,
publication API calls or commit/push. The harness supported patches, shell and read-only web research.
Client directory checks used installed Codex's actual protocol, not invented tool names.

<a id="已执行检查"></a>

## Executed checks

| ID | Command or method | Actual result and limits |
| --- | --- | --- |
| V01 | git rev-parse --show-toplevel, git status --porcelain=v1, git ls-files, rg --files --hidden | Correct Git root, initially zero tracked files; not product implementation review |
| V02 | Get-Command, --version, code --list-extensions --show-versions, shim inspection | Frequent CLI paths/versions confirmed; fd/sd/bat absent from PATH, not necessarily absent from disk |
| V03 | npm install --ignore-scripts --no-audit --no-fund | Exit 0, 87 documentation packages, markdownlint-cli2 0.23.3 pinned with integrity lock |
| V04 | npm run lint:docs | After first corrections: 12 Markdown files, zero issues, exit 0; final file set below |
| V05 | python -X utf8 system skill-creator/scripts/quick_validate.py .agents/skills/ai-maintenance | Exit 0, Skill is valid; not actual triggering/full specification/security acceptance |
| V06 | codex app-server generate-json-schema --out build/ai-init/schema | Exit 0; installed schema checks initialize/skills/list arguments |
| V07 | python -X utf8 build/ai-init/check-discovery.py | Exit 0; root/docs/ai each find one repository Skill, enabled=true, errors=0 |
| V08 | V07 with temporary skills.config disabling in a second isolated process | Root/docs/ai find the same Skill, enabled=false, errors=0; no configuration writes |
| V09 | Read requirements through end marker, item mapping and content review | Coverage retains source lines/attachment hash; static inventory does not verify runtime effects |

V07/V08 together made four real directory/enablement assertions, zero model calls and zero user configuration
writes. Subprocess stdio started no model session/custom Agent. After stdin closed, waited for exit and
terminated owned processes only on timeout. Redacted output: build/ai-init/discovery-result.json.
Temporary schemas/scripts/results remained ignored under build/ai-init/ for local review, outside commits.

Initial CreateProcessAsUserW failed: 5 was a platform process error; permitted retries resumed without
weakening security. The first Skill validator used Windows default GBK and raised UnicodeDecodeError;
Python -X utf8 passed without changing Skill encoding/validator. First lint found seven blank-line issues;
document fixes passed without disabling rules. MD033 allowed only used a anchors; MD013 handled tables/
code structurally, retaining other defaults.

<a id="最终静态验收"></a>

## Final static acceptance

| Check | Actual command/method | Result |
| --- | --- | --- |
| Full Markdown set | npm run lint:docs | 13 files, zero issues, exit 0 |
| Relative paths/anchors | node build/ai-init/check-docs.mjs, markdown-it parsing actual links | 13 files, 390 reachable local references, exit 0 |
| Coverage/tool structure | Same script; unique IDs, chapters and actual tool rows | 24 chapters, 290 items, 31 tools; no duplicate IDs |
| Original-input cross-check | Extract unfenced chapter/list/table lines and compare source lines; templates separately | 24 chapters, 225 source list/table lines, zero unmapped; 12 template subitems/role goals recorded |
| Lockfile installation | `npm ci --ignore-scripts --no-audit --no-fund --registry=https://registry.npmjs.org` | 87 packages, exit 0; no install scripts |
| Lockfile origins | jq parses resolved URLs/checks hosts | registry.npmjs.org only, integrity retained; no user npm configuration changes |
| Skill YAML | yq --front-matter=extract -o=json '.' .agents/skills/ai-maintenance/SKILL.md | name/description/string metadata parsed correctly, exit 0 |
| Expected exit codes | jq -n -e false; jq -n -e empty; rg searches absent fixed marker | 1, 4, 1 respectively; expected exit behavior, not failures |
| Ignore rules | git check-ignore for build, node_modules, .env and development/secret samples | All four matched, exit 0 |
| Diff/protection | git diff --check, git status --short, .kilo/.gitignore reread | Exit 0; initially untracked files also reviewed in full |

AGENTS.md was 72 lines/4124 bytes, Skill 48 lines/3046 bytes; complete requirements remained in selectively loaded records. All actual references
existed locally without attachment-path runtime dependencies. Only docs/
documentation tools existed; no product unit-test counts/production results were claimed.

Initial npm installation inherited the local Tencent mirror. A new lockfile was generated/verified from
official npm in the task directory, replacing only that generated lock, followed by npm ci. No global
settings changed. Full scripts/redacted JSON remained ignored under build/ai-init/. This permanent page
explains acceptance without requiring temporary artifacts.

<a id="未执行的运行验收"></a>

## Unexecuted runtime acceptance

- Actual AGENTS.md loading in new model sessions, same-name conflicts, permission refusal and side-effect-free
  Skill invocation were unexecuted; directory listings do not verify these behaviors.
- Twenty automatic-routing queries and three enabled/disabled comparison tasks were unexecuted.
  Policy did not allow additional model agents; no-model discovery was unaffected.
  [Evaluation sets](../skill-evaluation.md) specify inputs/criteria/records, not successful effects.
- Team adoption, versions/sessions and loading of the other 12 clients were unconfirmed.
- Product purposes, stack, build/test/typecheck and real-service acceptance had no available project requirements.

Blocked IDs remained in [coverage](initialization-coverage.md). MCP, permanent custom Agents, OpenSpec/
Superpowers and production deployment were not adopted; runtime acceptance lay outside that task.
Future conditional workflows were documented.

<a id="规则迁移验证"></a>

## Rule migration validation

Scope: migrate AI maintenance guidance, shorten entries, use Use when and add writing guidance;
no previous-draft content changes. Eight rule/validation files moved into Skill references, with large
validation records in references/records; the old index became part of the Skill. Reviewed summaries/bodies/all
local links and preserved all 290 original coverage IDs/statuses and 31 tools. Actual model triggering/
quality comparisons remained unexecuted; full initialization acceptance was not claimed again.

| Check | Actual command/method | Result |
| --- | --- | --- |
| Changed Markdown | node node_modules/markdownlint-cli2/markdownlint-cli2-bin.mjs --no-globs AGENTS.md README.md '.agents/**/*.md' | 13 files, zero issues, exit 0 |
| Entire repository Markdown | npm run lint:docs | 14 files, exit 1; six issues in unchanged previous-draft/README.md: two MD040 at L26/L65 and four MD060 at L35 |
| References/coverage | node build/ai-skills-refactor/check-docs.mjs | 14 files, 396 local references, 25 chapters, 296 independent IDs, 31 tools; exit 0 |
| Content preservation | Item comparison with premigration snapshot, ID/status/body checks | All 290 old IDs/statuses preserved; operations/terminal-tools unchanged except links; other diffs reviewed paragraph by paragraph |
| Skill format | python -X utf8 system skill-creator/scripts/quick_validate.py .agents/skills/ai-maintenance | Skill is valid, exit 0 |
| Native discovery/disabling | python -X utf8 build/ai-skills-refactor/check-discovery.py | Root/references each find one Skill; four enable/temporary-disable assertions pass, errors=0, model calls/user configuration writes=0 |
| Diff/old directory | git diff --check; migration map/old paths | Exit 0; docs/ai removed; untracked artifacts also fully reviewed |

Entry sizes are UTF-8 bytes, not model tokens or behavioral improvements:

| Entry | Before migration | After migration | Byte reduction |
| --- | --- | --- | --- |
| SKILL.md | 48 lines / 3046 bytes | 22 lines / 1260 bytes | 58.6% |
| AGENTS.md | 72 lines / 4124 bytes | 49 lines / 2490 bytes | 39.6% |

Rules/history load by scenario; shortening did not delete bodies/original blockers. previous-draft was
added during that round; only its README was read to correct empty-repository descriptions, without
modifying/running the prototype. Prototype lint failures remained unchanged; no exclusion/rule disabling.
Passing --no-globs through npm script produced local npm EUNKNOWNCONFIG; scoped checks called installed
markdownlint-cli2 directly with the same root rules. Snapshots/maps/redacted results remained ignored
in build/ai-skills-refactor/, not runtime dependencies.
