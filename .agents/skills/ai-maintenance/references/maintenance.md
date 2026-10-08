# Maintenance guidance

<a id="维护说明"></a>

<a id="维护合同"></a>

<a id="范围与证据"></a>

<a id="范围与核验依据"></a>

<a id="maintenance-contract"></a>

<a id="scope-and-verification-evidence"></a>

## Scope and verification sources

Before product changes, verify commands against actual languages, lockfiles, callers, tests and runtime
configuration. The previous-draft README describes collection, SQLite caching and a static dashboard;
these descriptions have not established runtime acceptance for this project. [AGENTS.md](../../../../AGENTS.md)
is authoritative.

Maintenance guidance uses one root entry, one Skill and selective references. Coverage/validation records
live in references/records and are read only for initialization recovery or delivery review. Documentation
lint and unified verification run from the root; M0 restored package.json/package-lock.json. Product
dependencies remain governed by desktop npm/Cargo manifests and lockfiles. Retain Kilo's local ignore file.
No module differences require nested rules. No adopted Claude workflow currently requires CLAUDE.md.
Without independent roles, product changes, deployment resources or configuration needs, do not create
empty client configurations, OpenSpec changes, roadmaps, development guides or secret examples. Unconfirmed
client scope does not establish inapplicability; see [client records](clients.md).

Research in this order: establish scope/authorization, inspect workspace and parent/nearest rules, check
indexes, read relevant implementations/tests/versions, then verify official text, release notes and necessary
source. Search snippets only locate sources. Trace conflicting conclusions to corresponding versions;
record unresolved facts as unverified. Match distributed packages to actual versions, integrity and source
commits. Public npm build provenance may differ from same-version GitHub tags: check declared commits and
distinguish reading declarations from signature verification. Declared commits may differ from compiled
code; verify field rules in installed modules. Runtime samples need independent checks. Distribution
commits/database-wide schemas do not identify historical record versions.

Raw SQLite samples need valid WAL files. Main-file digests do not establish integrity of uncheckpointed records.
Read-only access may update SHM; check database/WAL files and shared memory separately. Trace SDK-computed
totals/default zeros to actual writers/codecs. Numeric total/token fields are not necessarily independent
reports. Record key facts with [source fields](source-index.md#fields) and recheck before relevant tasks.
Upgrades, deprecations, security notices, loading failures or behavior changes trigger updates;
do not automatically upgrade dependencies.

<a id="ai-规则维护"></a>

## AI rule maintenance

Deliver shared rules readable by target clients, necessary compatibility differences, item-level coverage
and actual verification. Ordinary product changes do not redo initialization. Work from the root; confirm
complete user input, existing edits, applicable rules and clients. Read truncated inputs in segments.
Clarify missing critical requirements while continuing independent work.

1. For initialization/full maintenance, extract lists, tables, conditions and templates chapter by chapter
   from original requirements. Preserve stable IDs, source references and check results, file sections, status and checks in the
   [coverage table](records/initialization-coverage.md). Local maintenance updates affected entries only;
   no parallel plans.
2. Determine applicability before assigning requirements to root rules, Skills or selective references.
   Migration updates actual reading entry points/cross-references. File existence, matching headings and
   line counts do not replace content review.
3. Verify changing conclusions against official text/installed versions and record sources. Verify rule
   startup, Skill discovery, manual invocation and authorization separately. When automatic discovery is
   absent, describe the temporary manual-reading workflow.
4. Read changes back against every requirement. Run documentation lint, references and necessary
   side-effect-free client probes. Substantive description/workflow changes use evaluation sets;
   unrun model evaluations remain unverified.
5. Synchronize affected sources/rules/plans and deliver files, checks and outstanding work. Initialization
   chapter/subitem counts belong in coverage records. Passing local edits do not complete initialization.

Unavailable sources remain unverified; do not infer conclusions from snippets. Record missing client
conditions. Preserve diagnostics and fix causes rather than deleting assertions, expanding permissions or
clearing blockers. Retry terminal startup through platform-permitted mechanisms. For timeouts, verify actual
state under the [execution requirements](operations.md#timeouts). Roll back only this task's differences.
Verify cleanup paths and preserve user files and required records.

<a id="authoring"></a>

<a id="分层与编写合同"></a>

<a id="规则组织与编写要求"></a>

<a id="layering-and-authoring-contract"></a>

## Rule organization and authoring

| Requirement | Authoritative location and limits |
| --- | --- |
| Stable constraints needed by most tasks | Root AGENTS.md: scope, real commands, tool hints, boundaries and selective navigation |
| Module/file-type differences | Nearest AGENTS.md or client path rules, after checking nested discovery/globs |
| Reusable specialist workflow | .agents/skills/name/SKILL.md; name/description route work and body loads as needed |
| Claude compatibility | Verify native reading after adoption; a thin CLAUDE.md imports shared rules if needed, adding actual differences only |
| Independent role | Target-client schema when isolation, tool restrictions or handoff are needed |
| Repeated manual task | Prefer Skills; verify client/session support before workflow/prompt files |
| Reviewable behavior change | One design/spec/ADR; create OpenSpec changes only after adoption |
| External live data/actions | Existing official APIs/CLIs or necessary MCP, with data/permissions/timeouts verified |
| Enforced restrictions | Sandbox, permissions, hooks and CI; documentation describes constraints only |
| Proven experience | Automate tests where possible; workflows in Skills, local facts in local docs |

Root rules do not duplicate directory trees, incident reports, coverage tables or tool manuals. Retain
migration-detail reading triggers. Full imports still consume context; ordinary links do not promise loading.
Preserve sources/dates/versions/rollback records; history belongs in Git/change archives. User instructions
and platform boundaries outrank lower-level rules. Every constraint needs a trigger/checkable result.

### Skills

Shared format does not guarantee discovery across clients. New Skill names match their directory, using
1–64 lowercase ASCII letters/digits/single hyphens without leading/trailing/repeated hyphens. Descriptions
contain 1–1024 characters, leading with outcome/intent and, if needed, nearest exclusions. name/description
are mandatory. State actual licenses; do not invent them. metadata uses string keys/values; compatibility
is at most 500 characters. allowed-tools is experimental, not cross-client permission enforcement.
See the [Skills specification](https://agentskills.io/specification).

Bodies cover outcomes, inputs, steps, recovery and validation, with resources as needed. Fewer than
500 lines is a maintenance goal, not a universal parser limit. Resolve resources against the Skill
directory; use actual paths. Copies identify the primary maintained file and verify consistency. Indexes provide navigation,
not proof of discovery.

Add scripts only for repeated needs/deterministic benefits. State dependencies, inputs/outputs, cwd,
exit codes, timeouts, side effects, noninteractive --help and applicable dry-run/idempotency/rollback.
Audit whole bundles, frontmatter, dynamic shell, hooks, dependencies and outbound access, not just scripts.
Recheck diffs after pinned-version updates. This Skill is documentation only, without scripts, hooks,
external dependencies or new license declarations.

Separate manual invocation, automatic discovery and execution authorization. This Skill retains default
automatic selection. Client-specific Codex allow_implicit_invocation/Claude disable-model-invocation
require client verification and do not replace permissions. Verify specification, resources, placement,
actual discovery, triggers and quality; see [evaluation](skill-evaluation.md).

<a id="客户端兼容与角色"></a>

### Client compatibility and roles

Before adopting Claude, check versions, sessions, providers, flags and existing local rules that shadow
shared rules. Choose native AGENTS.md/imports only after those checks. Do not delete imports merely because
new documentation exists. Maintain one shared source and verify depth/context costs. Check paths rules,
ordinary links and imports separately. Actual loading UI/logs establish reading; file existence does not.

Claude docs verified on 2026-09-24 permit `@AGENTS.md` imports in CLAUDE.md, at most four hops with startup
context cost. `.claude/rules/` paths select files; inspect actual loading with `/context`. Native reading
requires v2.1.277+. Before v2.1.281, some provider/telemetry sessions have extra limits. Existing
CLAUDE.md/CLAUDE.local.md may shadow AGENTS.md by default; built-in plugins/session settings also affect
reading. Before v2.1.280, directly read AGENTS.md may be absent from /memory or /context. Use version-specific
UI/logs rather than inferring failure from missing UI entries. Remove compatibility layers only after all
target sessions read natively and contain no dedicated content. These are future-adoption instructions;
no compatibility file is created here. See [official Claude guidance](https://code.claude.com/docs/en/memory).

Distinguish Local/Agent Host for VS Code prompt files: Local supports them; Agent Host deprecated them and
does not load them, so use Skills. This project uses shared Skills rather than .github/prompts. Actual session
loading awaits team confirmation. See [official VS Code guidance](https://code.visualstudio.com/docs/agent-customization/prompt-files).

No permanent custom Agent is needed. If added later, verify target schemas: Markdown/YAML/TOML and similarly
named model/tools/permissions/mode/handoff/isolation fields are not interchangeable. Specify inputs/results,
completion, edit scope, failure returns and escalation. Planning/review prefer real read-only permissions;
prompts cannot enforce shell/MCP restrictions. Delegation needs independent subtasks/current authorization.
Assign file ownership; isolate databases, ports, caches and paths; the primary Agent integrates/verifies.
Check rule/Skill/permission/context inheritance. Shared filesystems are not isolation. Respect user model
choices and specified time, token or cost limits.

<a id="workflow"></a>

<a id="工作分流"></a>

## Work routing

For cumulative semantics, inspect normalization, actual APIs and native records. Distinguish total/uncached
input, output/reasoning subsets and default/reported zeros. Database migrations do not identify historical
client versions. Regress complete old summaries, consumed positions, concurrency/rollback and invalid rows.
Repeated reconciliation snapshots do not verify formats; see
[real Hermes acceptance](../../../../docs/validation/desktop-usage/hermes-container-sample.md).

For SDK saved-format changes, inspect writers/codecs/migrations instead of reusing namesake fields. Mutable
session versions support compatible reads without verifying historical records. Metrics merging runs/retries
are observations, not one call each. Do not add duplicate message/manifest/database totals. Verify root/
environment priority, full registry/manual roots, partial writes, duplicate identities and rollback;
see [Cline SDK acceptance](../../../../docs/validation/desktop-usage/cline-container-sample.md).

Daily query acceleration checks cross-dimension provider-session deduplication, NULL/empty unknowns,
instance allowlists, old layout/writer invalidation, retention and rollback. Layout/name changes update
derived versions/expression indexes. Measure unfiltered/provider/model/Agent/combined filters individually;
one tested query does not verify other queries. Aggregate native resources across all processes/fixed roles;
command lines/paths never enter records.

| Task | Action and reviewable result | Acceptance |
| --- | --- | --- |
| Copy/format/simple local change | Small sufficient change; research as needed | Lint, links, facts and diff |
| Defect repair | Minimal reproduction, root cause, fix and feasible regression | Old symptoms, corrected behavior and failure branches |
| Feature/cross-module/API/data model | Design covering goals, scope, options, tradeoffs, compatibility, failures, verification and rollback | Observable tests for every requirement |
| Security/migration/deployment | Establish permissions, environment, impact, sequence and rollback | Rehearsal and actual publication separately |
| Unclear needs/architecture disagreement | Bounded exploration/critical clarification; continue independent work | Decisions, supporting references/results and unresolved questions |

Neither OpenSpec nor Superpowers is adopted. See [Plan.md](../../../../Plan.md) and the
[design entry](../../../../docs/design/desktop-usage/README.md) for specifications, adapter sources and stages.
Plan.md owns implementation status; planned entries are not implementation. Shorten completed entries while
retaining IDs, current design, remaining conditions and latest result links. Designs do not accumulate
logs; replace stale descriptions and preserve history in Git. Reconcile plans with the adapter matrix/latest
acceptance. Distinguish unimplemented, unaccepted, missing-sample/permission, deferred and canceled/excluded
items, each with next steps/completion criteria. Link details, retain first failures and avoid per-round
test counts in plans. Acceptance indexes summarize tested scope/results. Authorization changes update
delivery requirements; old restrictions on installation/client execution cannot override later permission.

Source health distinguishes record errors, independent reconciliation, coverage limits and version
compatibility per file/database format. Verify reevaluation with real unchanged old cursors. SQLite invalid-row state
persists across incremental windows and recovers after correction/deletion. Invalid reconciliation types
must not bulk-fail valid messages. Mouse tests cover no query before release, forward/reverse selection,
matching distribution/table/cost ranges, reset and stale responses.

Parser-upgrade regressions need genuine old summaries, not old version columns/new summaries. Check complete
fields, source revision priority, same-batch conflict order, old cursor/position replay and rollback.
Verify unchanged counts/usage on consistent backups before authorized statistics databases under a
single-writer lock. Do not edit original Agent sources. Update these requirements for later work;
ordinary maintenance adds no approval requirement.

Cost fixes compare usage/cost scopes/retention before model identity/official prices. Verify coexisting
archives/details, detail deletion, hourly/weekly/monthly selections and many small calls. Compare independent
integer recomputation/application results. Unknown splits/tiers cannot be inferred from zeros/total input.
Merge model rows by source provider/model, retaining multiple price bases; see the
[pricing rules](../../../../docs/design/desktop-usage/pricing.md). Authorized cross-model references separate
identity aliases/substitution rules and list approved mappings only. Regress exact-price priority,
ambiguity rejection, unchanged history and substitution labels in summary/model amounts/unit prices.

Only local Agent sources are in scope. Platform/CI/scheduling use dedicated design specifications. Record local
extraction attempts/field limits per new adapter; remote billing/APIs cannot substitute. Automation shares
collection rules. Plans do not authorize system-task registration/environment startup. Container samples
record actual versions/distribution hashes; independent model responses are checks only. Installed sample versions cannot identify records
lacking their own versions. Distinguish prefilled zero/
actual API reports; interval totals cannot invent calls/models/complete days. Corrections compare complete
old summaries and regress cursors, concurrent wrappers, conflicts and rollback;
see [M8 samples](../../../../docs/validation/desktop-usage/m8-container-samples.md).

Initialization times are not completion times. Environment versions do not identify all session messages.
New non-usage types require fixed-version source checks showing how records are generated and read; permit proven types only. Verify shared-engine
zero fallbacks per product/API, preserving conflict/diagnostic history for corrections. Exclude auxiliary
JSON only after verifying generation, complete shapes and paired identity. Test unknown versions, invalid
shapes and manual usage/invalid files through real discover, without name inference/database clearing.

Closed-source official distributions may show how events are constructed/normalized in compiled modules;
third-party namesake fields cannot substitute. Xum custom providers omit streaming usage requests by default;
five native zero fields do not establish reported values. Explicitly record gateway controls that add only real-model
usage options and forward responses unchanged. Controls do not establish defaults or fabricate source records.
Display input is uncached, text output excludes reasoning and default zeros remain unknown. Cursor
correction needs complete old aggregates, retaining conflicts/history.

Roo archiving announcements do not prove historical local providers cannot run. Inspect official VSIX/native
extension-host/API independently. Four bucket/cost zeros remain unknown. Record lost OpenAI-compatible cache
details/cancellation deletion of unfinished request files as separate coverage gaps. Do not invent missing calls/
uncached input. Full-registry tests omit HOME to activate fixed host candidates, with isolated APPDATA;
verify default Linux discover separately. Source corrections regress complete old summaries/unchanged
cursors, concurrent rollback and conflicts, without clearing databases. Failed Junie tasks still contain
per-call usage. Separate uncached input, zeros, client cost and duration; preserve native keys and restore
only proven defaults in old-summary candidates.

Real local Agent extraction is already authorized: read-only/redacted under the
[implementation prerequisites](../../../../docs/design/desktop-usage/implementation-readiness.md), without asking again.
Unverified local-format IDEs such as JetBrains/TRAE were deferred to F1/outside current probing/implementation;
an earlier all-Agents-first-release attempt must not block main work. Follow later user authorization and
current Plan.md when scope changes. Extended research/M8 documentation-level implementation is complete:
18 adapters (17 parsers and Qoder probe), including Junie CLI/built-in Zed. Amazon Q/Codebuff/iFlow are excluded
for absent local per-call usage records/shutdown. Cursor/Warp/TRAE remote routes are outside the local boundary.
Optional tool installation/upgrades need adoption/current authorization; concepts are not executed tools.
Select workspace isolation by risk; worktrees do not isolate databases/remote systems.

If adopting OpenSpec, verify installed version/profile/schema/generated client commands and read implementation,
specs, active changes and roadmap. Follow supported exploration/proposal/implementation/update/sync/archive;
command names vary by client. Verify then-current spec-driven artifacts; no empty changes. Execute once
design requirements are sufficiently clear. Ask new permission with concrete reviewable content only when needed.
status/show/validate structure checks are not behavior acceptance. Reproduce automatable failures, fix minimally
then refactor; record alternatives for unstable automation. Correct the design requirements rather than adjusting
assertions to wrong code. Check main-spec/archive diffs; resolve project blockers before archiving.
Command success/warnings do not verify implementation behavior.

If adopting Superpowers, read actual-version Skills and verify classification, Native/subagent execution
and stage completion criteria within user scope; do not generalize one version. Combining tools is a project choice;
retain one primary specification with references elsewhere. Unexecuted workflows are not complete.

<a id="测试与质量"></a>

## Testing and quality

For collection, verify active sessions, archives, databases and cleanup under the
[data rules](../../../../docs/design/desktop-usage/data-contract.md). Without shared call IDs, select one verified source for the relevant
partition instead of matching times/tokens. Regress archive transitions, rescans, late copies,
source loss and partial history against independent counts/totals. Export/import first asserts nonempty data.
Refresh checks request counts, stale responses, user switching, filters/paging; fmt/Clippy cover the workspace.

Automatic collection covers global pause, source deadlines, busy manual merging, residual system triggers
and task-definition drift. Source completion cannot postpone all sources; GUI full completion synchronizes
system deadlines. Test both with actual collection. Native tests isolate source/configuration environments;
`--data-dir` does not narrow discovery. For [installation lifecycle](../../../../docs/design/desktop-usage/installation-lifecycle.md),
compare official templates/local diffs before real old/new packages. Uninstall checks executable, arguments/
task identity/current user precisely; failure blocks program removal. Upgrade temporary uninstall retains
intent and preserves other startup entries.

macOS desktop/specific hardware requirements were canceled this round. Actual GTK/WebKit/package lifecycle
in rootless Podman is authorized. Record image/package hashes, ordinary user, display/sandbox and extraction/
FUSE mode. FUSE rejection checks devices/kernel/container capabilities first. Explicit SYS_ADMIN is limited
to this task's rootless container, with ordinary GUI user/effective capabilities zero/default seccomp.
Check real read-only mounts/release. Report host login/logout/system integration separately; stop owned
containers only, never globally clean. Inspect screenshots/target-language fonts; DOM text alone does not
prove readable rendering. Add official fonts for missing glyphs and reverify; compilation/IPC/nodes are insufficient.

Screen-reader acceptance checks native keyboard, AT-SPI focus and actual speech. Injected-name probes only
locate defects. Verify log buffering against installed source; separately record diagnostic settings/bounded
owned-process cleanup without altering events/speech. Without hardware audio, verify speech output/control
names only, not physical audibility/all controls.

Official container clients may call local models using verified provider interfaces without personal accounts/
paid requests. Independently compare per-call usage records, CLI totals and real API usage. Reasoning timing does
not replace request/total tokens. Main-loop/session matches do not verify every title/background call.
Never subtract cumulatives to fabricate events. Disabling background/setting titles does not verify default behavior.
Verify file serialization, identity and parents before treating suffixes/span names as JSONL. Compatible real
samples still need record versions, mixed databases, empty sessions and unchanged-old-cursor reevaluation
before registering support.

OpenCode paging includes time/stable row ID; replay cannot skip old records. Test same-millisecond pages,
complete-old-summary parsing upgrades and checkpoint rollback together. Qwen SDK reads complete JSON objects
using verified spans. Host/user/session/local-day source selection/trace+span deduplication cover both import orders, unknown
ownership/versions, sealed partitions with deleted details and identity clearing. Regress new-directory copies
after sealing, real retention/reopen/late source copies, not only simulated deletion. Missing per-call identities
cannot prove no overlap with sealed source partitions. Without SDK verified reasoning-inclusion rules, nonzero
reasoning does not establish totals; shared sessions do not prove parent-child relationships.

Credential concurrency records contain only operations, numeric codes and missing/different/failed states;
retain first failures. Windows bounded missing read-back after writes does not extend to authentication.
Mismatches/read errors cannot be fixed by rewriting/permissive cleanup. Revocation read-back/deletion confirmation
is bounded. Cross-process/real HTTP tests check that successful revocation rejects that source, another source
works and owned leftovers are zero; one successful Delete is insufficient.

System tasks do not inherit test environments; persist manual roots and independently check nonempty results
read-only before GUI restart. Resource samples assert complete durations/live processes; early exits fail.
Cache tests cover same/other connection writes, filter/user/date isolation and capacity. Time cache misses/hits
separately; hits do not establish first-query speed. Optional derived tables cover old writers, rollback,
empty dates, DST, overflow and retention/clear identity deletion without blocking valid ingestion;
see [query acceleration](../../../../docs/design/desktop-usage/query-acceleration.md).

When a time limit interrupts collection, check the actual job state and resumed reads. Per-source intervals cover clock jumps; directory
notifications cover pause, source toggles and fixed-time exclusions. Native Windows close messages verify
tray hide/restore. Real Storage proves write-lock release during parsing, two slots for one Agent's roots,
same-source merging before read and isolated failure. Ownership uses real registry discovery. Interruptions
cover JSON/JSONL, SQLite VM/backup pages, authoritative archives and retention/cost transactions; scoped
callbacks cannot leak to later jobs. Commit fingerprints/generations with events/cursors; reread same-size
replacements after failure. Unvisited/merged/interrupted sources do not advance deadlines. Committed complete-line
windows retain resume state. Bounded file scans regress large historical/small current files, cross-round/
reopen rotation and idempotency. One repeated window cannot postpone unvisited files; persist rotation with
acknowledged file states.

Resource measurements include every child process, full duration and actual schedule counts. [Resource limits](../../../../docs/design/desktop-usage/architecture.md#budgets)
distinguish idle averages/sample peaks and GUI/headless imports. Changes to limits retain raw measurements/
environment limits. Warm empty pages do not establish minimum overhead. Do not omit GPU processes or extrapolate
single-machine results. Wait for release builds before native tests. WebView2 diagnostics are recorded
test conditions, not product defaults.

Verify HTTP authentication through preview/apply/failure recovery/revocation/real requests. Checks are read-only;
previews never expose headers. Reject missing credentials, failed stores and unverified exporter authentication.
Real stores use random owned entries/precise cleanup without secret output; report synthetic/native results
separately. Background credential reads never unlock/show authentication UI; Linux timeouts are bounded.
Distinguish persistent default/other/duplicate collections; default aliases cannot point to session.
macOS queries/writes/deletes disable cloud synchronization. Linux tests use isolated D-Bus/disposable keyrings;
check service name owners before probes to avoid activating other daemons. Cross-platform type checks do not
verify native macOS storage/desktop.

Each repeat run creates an independent database rather than reusing PID paths. Existing directories fail
creation; close connections/locks and clean owned directories. Incremental performance includes collection,
archive/retention and UI updates. Archive skips regress late corrections, partition deletion, missing results
and calendar boundaries; reused round timestamps do not replace stage timings. Report real headless, task API,
actual system startup and GUI IPC independently.

Select unit/integration/end-to-end/format checks from real source/CI. Cover new key branches/failures without
copying implementation into assertions. Mocks control boundaries; protocols/migrations/lifecycle require
real dependencies. Record nonautomatable reasons/reproductions/alternatives. previous-draft product tests/
runtime remain unverified; document checks do not establish product success.

Record cwd, commands, environment, exit codes, quantities and results; separate discovery/compile/run and old/
new failures. Do not delete assertions, unjustifiably skip or extend timeouts to conceal defects.
Do not repeat passing checks without cause. Use root markdownlint-cli2 and check paths/anchors/references/
fences/resources without globally disabling rules. Prefer appropriate Mermaid/Draw.io/Excalidraw/SVG for flows
and tools for data charts; check syntax/rendering. Original maintenance initialization required no diagrams.

<a id="文档与交接"></a>

## Documentation and handoff

Read [writing guidance](writing-guidance.md) for replies/comments/docs/PR descriptions. Descriptions begin
with Use when and actual triggers; Skill bodies retain short workflows/resource choices. Rules/manuals/
sources/evaluation belong in references; history in references/records. Do not retain duplicate primary rules under docs/ai for migration or load
whole coverage tables for routine work.

Synchronize affected entries, compatibility layers, Skills, module docs, sources, ADRs, tests, development/
deployment, roadmaps and playbooks only. Add indexes for complex reading order rather than README in every
directory. Roadmaps state goals/priorities/dependencies; plans state tasks/acceptance/status/blockers; specs
state behavior. Update existing Plan.md rather than a parallel system. User, architecture and development
documents need bilingual review; AI rules, Skills and execution plans keep a single original.
Bilingual docs/comment references follow the [documentation requirements](../../../../docs/design/documentation-site.md).

Resume long tasks with [coverage records](records/initialization-coverage.md): goals, completed/remaining IDs,
checks, next steps and permission boundaries. Do not copy chats/secrets or substitute "the rest as before"
for work. Facts synchronize references/commands/diagrams; label unimplemented behavior.

<a id="纠错与复核"></a>

## Correction and review

Record reproductions, actual causes and checks; distinguish infrastructure, implementation, triggers and
stale sources. Add tests/deterministic checks for confirmed recurring risks only. Workflows belong in Skills;
local constraints in local rules. Rules state triggers/scope/review/expiry and merge duplicates. One typo is
not a general rule. Third-party text, one-off evaluations/unconfirmed preferences are not authoritative.
Persistent personal memory follows platform permission policy. Compare after model/harness/dependency upgrades
and keep/revise/remove old patches according to observed results.

No third-party self-improvement Skill is required. If researching ClawHub later, verify official read-only
APIs before small searches; never guess details/versions/scan URLs. Audit owners/slugs/versions/provenance/
permissions/complete files/scripts/network. Ranking/downloads/nonSuspiciousOnly do not establish safety;
missing safety fields remain unverified. Keep sources/licenses/attribution; do not execute install scripts.
Respect caches/rate limits/429/Retry-After and continue independent work when unavailable.

<a id="交付合同"></a>

<a id="交付要求"></a>

<a id="delivery-contract"></a>

## Delivery requirements

Read original requirements/artifacts before updating [coverage](records/initialization-coverage.md).
Handle lists/tables/templates/branches individually; read reused bodies. Statuses are covered/inapplicable/
pending/blocked only. Missing tools, insufficient time or untested work are not inapplicability. Record rule
placement/runtime acceptance separately; unfinished children constrain parent chapters. Report chapter counts,
independent statuses, unmapped counts, artifacts, real checks and exception next steps. Preserve pending/
blocked IDs; deleting scope does not complete initialization.
