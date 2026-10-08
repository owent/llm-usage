# Annual activity, Copilot charts and telemetry configuration

<a id="全年活动图copilot-图表与遥测配置入口"></a>

2026-10-01, Windows 11 x64. Earlier Copilot review/other uncommitted changes were preserved;
no commit/push, real Agent configuration changes or model calls occurred.
See [Copilot OTel configuration requirements](../../design/desktop-usage/copilot-otel.md).

<a id="修复与范围"></a>

## Changes and scope

- Activity previously reused the last-30-days query. It now queries Jan 1–Dec 31 of the current
  statistical-timezone year, with year selection and 366 cells in leap years. Future dates,
  measured zero calls, missing daily history and partial coverage remain distinct. Weekday
  distribution independently queries the statistical filter range, avoiding missed dates on
  year changes/cross-year ranges; weekday names use the UTC calendar.
- Copilot has known input with unknown cached/uncached splits; the old split-only graph hid
  input. Added a reported-input curve. Unknown totals show known input/output and a notice,
  without deriving a complete total. Trends/today's overview tooltips preserve unknown totals,
  known components and observed zero values; zero is not filtered into no data.
- Startup runs a read-only asynchronous check on a blocking worker, sharing results with the
  frontend. Overview indicates missing configuration; general settings offers the full list,
  recheck, preview, merged apply and revocation within the current process.
- VS Code default/profile/Insiders/Agent Host, Gemini, Qwen, Claude, Codex and CodeBuddy use
  their own configurations. CLI Copilot gets a process launcher without editing managed state.
  PATH-file/existing-directory checks do not execute CLIs; normal installation manifests
  verify VS Code configuration keys.
- JSONC edits exact ranges; TOML formatting is retained. Checks cover bounded reads, duplicate
  keys, excessive depth, links, read-only files and concurrent edits. Codex merges exporter
  leaf fields while retaining ordinary/inline-table headers/options and refusing conflicting
  shapes. Original bytes are backed up; temporary files replace targets. Profile Sync
  exclusions merge into the default user file with conditional rollback. Revocation skips
  later user edits; existing destinations, enterprise policies, startup environment and
  explicit Sync conflicts are not overwritten forcibly.
- Local receiver binding succeeds before OTLP configuration writes; failures undo enabling
  performed by this operation. Corrected OTLP AnyValue field numbers/old test encoding;
  successful protobuf responses are empty protobuf. Added strict logs attribute allowlists;
  logs/supplemental traces remain separate from native statistics.

<a id="核验资料"></a>

<a id="证据"></a>

<a id="核验依据"></a>

## Source references

Shared fields: verified_at=2026-10-01, owner=repository maintainer. Recheck for affected
configuration tasks/client upgrades/format or policy changes. Rolling documentation describes
configuration methods without verifying actual exports from the installed version.

| ID / scope | source_url / source_version | method / status / impact |
| --- | --- | --- |
| T01 Copilot file/Agent Host | [Existing fixed-source references](../../design/desktop-usage/copilot-otel.md#依据与本轮验证)，dc546cc3c9979a19adafccd439889d7b64298def | Read-only source/local-manifest checks; extension 0.68.0 declarations/app 1.140.0 baseline; new exports not enabled |
| T02 Sync scope | [userDataSync.ts](https://github.com/microsoft/vscode/blob/dc546cc3c9979a19adafccd439889d7b64298def/src/vs/platform/userDataSync/common/userDataSync.ts) | Fixed source: ignoredSettings is APPLICATION-scoped, default empty; profiles must also edit the default user file |
| T03 Gemini outfile/home | [Official telemetry](https://geminicli.com/docs/cli/telemetry/)、[Storage](https://github.com/google-gemini/gemini-cli/blob/main/packages/core/src/config/storage.ts)、[paths](https://github.com/google-gemini/gemini-cli/blob/main/packages/core/src/utils/paths.ts), rolling | Official documentation/source: append .gemini after HOME substitution; only synthetic write checks |
| T04 Qwen configuration/content switches | [Official settings](https://github.com/QwenLM/qwen-code/blob/main/docs/users/configuration/settings.md)、[Telemetry](https://github.com/QwenLM/qwen-code/blob/main/docs/developers/development/telemetry.md), rolling | Official documentation: QWEN_HOME differs from runtime; sensitive log/span switches disabled; synthetic checks only |
| T05 Claude logs/env | [Official monitoring](https://code.claude.com/docs/en/monitoring-usage), rolling | Official documentation: independent HTTP/JSON logs endpoint/content switches; no beta traces or unsupported metrics configuration |
| T06 Codex TOML/exporter | [Official sample](https://developers.openai.com/codex/config-sample/), rolling | Official documentation: HTTP/JSON logs exporter and log_user_prompt; checked with openai-docs Skill |
| T07 CodeBuddy env/traces | [Official user settings](https://www.codebuddy.ai/docs/cli/settings)、[Official monitoring](https://www.codebuddy.ai/docs/zh/cli/monitoring), rolling | Public HTML confirms settings env/session semantics; HTTP/protobuf traces only; no native acceptance claim for newer local exports |
| T08 CLI user state | [Official configuration directories](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-config-dir-reference), rolling | config.json is managed state, settings.json user configuration; user telemetry keys unverified, so create only a launcher |
| T09 OTLP encoding | [common.proto](https://github.com/open-telemetry/opentelemetry-proto/blob/main/opentelemetry/proto/common/v1/common.proto)、[logs.proto](https://github.com/open-telemetry/opentelemetry-proto/blob/main/opentelemetry/proto/logs/v1/logs.proto), rolling | Official proto/independent standard samples verify fields; corrected old integer/boolean/double field numbers |

Public-source checks remain under ignored build/telemetry-setup-evidence/.
Fixed Sync-source SHA-256: bdff135f2943761075df3e238a6bc190c1dd5289e033585acbfebfa3835bf329.

<a id="本机只读检查"></a>

## Local read-only checks

This describes the preceding check, when user directories still supplied configuration
candidates. The subsequent UI stage below tightened installation requirements.
Explicitly ran `cargo test --manifest-path desktop/src-tauri/Cargo.toml --locked -p llm-usage-desktop local_configuration_audit -- --ignored --nocapture`,
exit 0. Final discovery found missing user-level outputs for VS Code default/profile and
their Agent Hosts, Codex, Copilot CLI and CodeBuddy. Output contains only client IDs/status/
reasons, without configuration contents/credentials. The read-only check does not identify
CLI versions from directories or guarantee that other startup environments have no overrides.
Passing candidate checks does not establish output files/token records.

<a id="验证与缺口"></a>

## Checks and missing coverage

This is the preceding acceptance baseline; subsequent batch-action results follow below.
Environment: Node.js 24.21.0, Rust/Cargo 1.98.1, Edge 154.0.4258.48, Windows 11 x64.
All commands ran from repository root and exited 0.

| Command | Result |
| --- | --- |
| npm run verify | Markdown 163 files; assets 67 derived/82 files; scripts three/UI ten tests; Svelte zero errors/warnings; fmt/Clippy -D warnings passed; Rust 752 passed/one ignored by default; frontend 745 modules built |
| npm run test:browser | Actual Edge, nine screenshots/159 simulated IPC calls; no pageerror; all interaction assertions passed |
| cargo test ... local_configuration_audit -- --ignored --nocapture | Explicitly ran the default-ignored read-only local audit; seven configuration candidates |
| python build/copilot-otel-setup/check-links.py | 98 relative links in six affected documents passed; external links/online anchors unverified |
| git diff --check | Passed; temporary artifacts stayed under ignored build/ |

Full logs: build/telemetry-setup-evidence/{verify,browser,local-audit}.log; screenshots/
interaction results: build/browser-smoke/, all outside Git. Browser checks use actual Edge/
Vite with simulated IPC and no real Agent connection. Verified annual/leap-year/future
cells, known input/unknown cache, unknown total/zero-output tooltips, asynchronous checks
without configuration writes, overview/settings navigation, preview/apply/revoke, ten
languages/themes/narrow screens and existing filters/users/pagination. Rust configuration
tests use synthetic files under build/, covering profile two-file rollback, Codex inline/
ordinary-table retention/revocation, source-publisher checks, depth limits and standard OTLP encoding.

Unverified: macOS/Linux desktops, automatic portable/custom-user-data discovery, real user
configuration writes/reloaded OTel exports, automatic statistics from new exports, statistical
source selection across formats, trace SQLite and persistent revocation. New exports supplement
checks; they do not establish recovered history or inclusion in total tokens.

<a id="总览简化与批量配置2026-10-01-后续"></a>

## Overview simplification and batch configuration: later 2026-10-01 stage

Overview shows a short notice with Enable all/View details. Details select the independent
local telemetry settings panel and move focus there. This panel retains per-item preview/
merged apply/revocation and adds batch actions. Already-configured targets, managed policies
and other blocked items are skipped. Each item is previewed immediately before applying,
avoiding stale precomputed plans when profiles share files. Failures do not stop remaining
items; results report successful/failed/manual items. Operation/revocation state survives navigation.

Installation checks accept PATH CLI launch files or publisher-verified IDE manifests.
Standalone Copilot extensions additionally require their IDE launch file. Leftover user
directories/globalStorage/profiles/extension caches without corresponding IDE installation
references do not alone display an Agent. Installed CLIs without configuration files still
appear. Non-PATH CLI installations, portable/custom user data and desktop-platform gaps remain.

New tests cover shared-file write order, continued processing/redacted errors after failures,
zero writes without configurable items/after check failures, excluding installation remnants,
extension host requirements, overview's two buttons without paths/keys/Agent lists, detail
navigation/focus, page changes during execution, partial-failure retries, revocation after
navigation and three-language narrow layouts. Same Windows/Node/Rust/Edge environment;
root commands finally exited 0.

| Command | Final result |
| --- | --- |
| npm run verify | Markdown 163 files; assets 67 derived/82 files; scripts three/UI 13/Rust 754 passed, one ignored by default; Svelte zero errors/warnings; fmt/Clippy/frontend build 746 modules passed |
| npm run test:browser | Two independent passing runs, each 192 simulated IPC calls; 25 check groups/11 screenshots, no pageerror; existing statistics/users/pagination assertions retained |
| cargo test ... local_configuration_audit -- --ignored --nocapture | One explicit read-only test passed; VS Code default/profile extensions/Agent Hosts plus Codex, five items |
| python build/copilot-otel-setup/check-links.py / git diff --check | Six documents/98 relative links and patch whitespace passed |

Local PATH lacked Copilot CLI/CodeBuddy launch files; their directories no longer alone create
configuration candidates. This does not establish uninstallation from nonstandard paths or
delete existing statistics. Logs: build/telemetry-batch-ui/{verify,browser,browser-recheck,local-audit}.log;
screenshots/simulated interactions: build/browser-smoke/. Overview/narrow details screenshots
were reviewed manually.

The page-change regression uses an explicitly releasable asynchronous mock, avoiding short
timers ending before navigation. Debugging also produced one 30-second initial-card timeout
without that run's page errors. After adding failure JSON/screenshots, two independent runs
passed identical assertions. Startup-timeout cause remains unknown; behavior assertions were
not weakened. Real Agent configuration was unchanged and no Agent was executed; actual
writes/new exports/cross-platform acceptance remain pending.
