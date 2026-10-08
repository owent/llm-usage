# Client adoption and compatibility records

<a id="客户端采用与兼容记录"></a>

<a id="当前范围"></a>

## Current scope

2026-09-24: the current Codex session was confirmed; other team clients were unconfirmed.
Verify installations, file existence, official documentation and actual loading separately.
All 13 clients are listed below. Unknown adoption remains a gap, not inapplicability.

| Client | Local verification and adoption | Entry handling and acceptance |
| --- | --- | --- |
| Codex | Current session; CLI 0.155.0-alpha.16.3; IDE extension 26.917.62051 | Root AGENTS.md/.agents/skills; official mechanisms verified, actual tests in validation records |
| Claude Code | Not found on PATH; team use unknown | No CLAUDE.md yet; after adoption, verify versions/session flags/native reading/imports/shadowing |
| VS Code Copilot | VS Code 1.139.0; only related websearch found in extension list, insufficient to confirm a Copilot session | No dedicated file; confirm Local/Agent Host, nesting switches, Skills/prompt files |
| OpenCode | Not found on PATH; use unknown | No .opencode; verify rule fallback, permissions and worktree boundaries for Skills discovery |
| Kilo Code | CLI wrapper exists; VS Code extension 7.7.9; existing .kilo/.gitignore | Preserve directory/shared entry; verify CLI/VS Code external Skill switches and dynamic shell independently |
| Pi | Scoop shim points to pi-coding-agent; installation is not adoption | No .pi; verify project trust, rule overrides, Skills and SYSTEM/APPEND_SYSTEM differences |
| Oh My Pi | Scoop shim points to oh-my-pi; installation is not adoption | No .omp; verify nearest nonempty native directory shadowing, user-source opt-in, persistent rules and skill:// |
| Command Code | Not found on PATH; use unknown | No .commandcode; verify same-name priority for Skills and Agents separately |
| Zoo Code | No project configuration/confirmed session | Do not infer .zoo; before enabling, verify .roo, mode rules, switches and Skills |
| OpenClaw | No project configuration/confirmed session | Verify workspace/state/library/workshop, allowlists and personal-source boundaries |
| Hermes Agent | Not found on PATH; use unknown | No .hermes; verify context first-match, root Skills, external directories and trust |
| Devin Desktop | No project configuration/confirmed session | Verify Desktop/Local/Cascade and .devin/.windsurf compatibility |
| Antigravity | No project configuration/confirmed session | Verify 2.0/CLI/IDE/.agents; do not infer standalone Gemini CLI behavior |

Except Codex/Kilo documentation and the Claude/prompt-file entries below, verification column entries
are user-provided research leads, not verified support promises. Open current official text and compare
installed versions at adoption. Official entry points follow; do not create every client directory.
Record Kilo/Pi/Oh My Pi CLI versions after team adoption is confirmed. The original round identified
launchers only, without model/network/initialization calls.

<a id="已核验机制"></a>

## Verified mechanisms

Codex combines rules from project root to cwd at startup, preferring AGENTS.override.md then AGENTS.md
per directory, with a default 32 KiB total limit. Nesting does not preload the whole tree.
Skills scan .agents/skills from cwd toward the root; same names do not merge automatically.
Rules/Skills search in opposite directions. Ordinary Markdown links are not discovery entries.
Sources: [rules](https://learn.chatgpt.com/docs/agent-configuration/agents-md),
[Skills](https://learn.chatgpt.com/docs/build-skills), both rolling documentation.
Installed-version results are separate in [validation records](records/validation.md).

Kilo official docs list root AGENTS.md, AGENT.md fallback and nested rules triggered by reads.
Skill products/locations/switches differ; CLI options do not automatically apply to VS Code.
Sources: [rules](https://kilo.ai/docs/customize/agents-md),
[Skills](https://kilo.ai/docs/customize/skills). No Kilo session-loading acceptance was performed.

Claude import boundaries and VS Code prompt-file session differences were checked in documentation;
see the [maintenance guidance](maintenance.md#客户端兼容与角色).
Neither had project-session runtime acceptance.

<a id="启用时查阅"></a>

## Read before enabling

| Client | Official entry points; version assertions on these pages are not currently adopted |
| --- | --- |
| Claude Code | [Memory](https://code.claude.com/docs/en/memory), [Skills](https://code.claude.com/docs/en/skills) |
| VS Code Copilot | [Instructions](https://code.visualstudio.com/docs/agent-customization/custom-instructions), [Skills](https://code.visualstudio.com/docs/agent-customization/agent-skills), [Prompt files](https://code.visualstudio.com/docs/agent-customization/prompt-files) |
| OpenCode | [Rules](https://opencode.ai/docs/rules), [Skills](https://opencode.ai/docs/skills) |
| Pi | [Configuration](https://pi.dev/docs/latest/configuration), [Skills](https://pi.dev/docs/latest/skills) |
| Oh My Pi | [Context](https://github.com/can1357/oh-my-pi/blob/main/docs/context-files.md), [Skills](https://github.com/can1357/oh-my-pi/blob/main/docs/skills.md) |
| Command Code | [Memory](https://commandcode.ai/docs/memory), [Skills](https://commandcode.ai/docs/skills) |
| Zoo Code | [Custom instructions](https://docs.zoocode.dev/features/custom-instructions) |
| OpenClaw | [Skills](https://docs.openclaw.ai/tools/skills), [Workspace](https://docs.openclaw.ai/concepts/agent-workspace) |
| Hermes Agent | [Context](https://hermes-agent.nousresearch.com/docs/user-guide/features/context-files), [Skills](https://hermes-agent.nousresearch.com/docs/user-guide/features/skills) |
| Devin Desktop | [Rules](https://docs.devin.ai/desktop/cascade/memories), [Skills](https://docs.devin.ai/desktop/cascade/skills) |
| Antigravity | [Rules](https://antigravity.google/docs/rules-workflows), [Skills](https://antigravity.google/docs/skills) |

<a id="无副作用验收协议"></a>

## Side-effect-free acceptance protocol

Record versions, session types, switches and redacted invocation records for each adopted client:

1. Start fresh sessions at the repository root and .agents/skills/ai-maintenance/references.
   Inspect actually loaded files/override sources; model self-reports of loading are insufficient.
2. Find/invoke ai-maintenance through the directory/selector and confirm SKILL.md read traces/resource reachability.
3. Verify same-name conflicts, disabling and permission rejection in owned isolated test setups/configuration.
   Do not change global configuration, delete existing Skills or access secrets/publication APIs.
4. Use the [trigger/quality set](skill-evaluation.md), distinguishing static format, directory discovery,
   real invocation and behavior; record failures/next steps.

Manual reading makes shared content reachable but does not replace loading acceptance.
