# Implementation boundaries and real-data validation

<a id="实施边界与真实数据验证"></a>

Product, platform, architecture and local-data development permission are established;
[Plan.md](../../../Plan.md) owns current status. Do not reconfirm the same read-only permission.
Plans alone do not authorize publication/push. On 2026-10-07 the user additionally authorized minimal
real requests to specified Zhipu Providers, local Zed configuration/testing, Podman samples and F1 checks.
This does not authorize other product logins/credentials or unlimited model requests.

<a id="当前边界"></a>

## Current boundaries

Collect local Agent instances only. Windows 11 x64 is the initial target; macOS/Linux retain CI.
Use Tauri 2/Rust, SQLite, Svelte/TypeScript and on-demand ECharts. Actual manifests/locks determine
dependencies/versions. Every supported capability needs version-specific field references/checks; unknowns stay unknown.

[Adapters](adapters.md) owns current adapters/real-acceptance labels. Sources with verifiable local-format information
in M2–M5/M8 may be implemented; IDEs with unverified local formats remain F1. Junie CLI/built-in Zed are M8;
JetBrains Copilot manual OTel files are M9; JetBrains AI Assistant remains F1. Hosts running supported
external Agents are counted under underlying sources, without claiming verified built-in host agents.
Telemetry, scheduling, pricing and platforms use dedicated designs. Missing real samples do not block
independent core features.

<a id="已授权的本机真实数据验证"></a>

## Authorized real local data validation

1. Use bounded discovery from installation metadata/known candidates, never whole-disk scans.
   F1 checks stay within authorized candidates. Read-only permission does not implicitly initialize Agents
   or create model requests; separately authorized container workflows follow below.
2. Review minimal extractors before reading a few recent samples covering additions, cache, model switches
   and auxiliary calls. Fill absent scenarios with clearly labeled synthetic data; empty sessions do not
   verify usage formats.
3. Parse JSON/JSONL locally, returning only allowlisted fields, anonymous relational IDs, versions/schemas,
   numbers and aggregate differences. SQLite selects required columns, never SELECT *; extract nested usage
   locally and discard bodies.
4. Prompts, responses, tool arguments/output, credentials, emails and real project paths never enter model
   context, logs, Git or CI. Anonymize relational IDs stably, replace needed bodies with constants and preserve
   real token numbers/inclusion relationships. Samples verify only their version/fields/scope;
   absent fields remain unknown.
5. Read active databases consistently/read-only under [storage design](data-contract.md). Never checkpoint,
   migrate, repair or copy bare databases without WAL. Retain unsafe-read limits and continue other sources.
6. Extracts, verification databases and artifacts go under root build/task-name/. Avoid original log copies.
   Necessary consistent staging copies restrict local access and are cleaned up. Keep artifacts apart from
   source directories; never delete/clean Agent sessions.
7. Only reviewed, redacted data enters tests/CI. Parsers pass synthetic/redacted regressions before independent
   same-version real checks of details, totals, timezones, counting units and rescans. Record differences/gaps.

Missing/unreadable sources do not justify remote billing/API replacements. Local files, loopback transport
or matching hostnames do not independently prove original local-instance provenance.

<a id="已授权的容器来源验证"></a>

## Authorized container source validation

Verify official client versions, installation integrity and local-model provider interfaces first.
Pin model/service images by version/digest and verify downloads. Use owned rootless storage, offline
containers, independent ordinary users/fresh HOME, without host sources/credentials/personal authentication.
Offline models listen only on loopback. Specified external Provider tests may enable isolated egress;
pass keys through stdin into intended process environments only. Install/download processes do not inherit
keys. No ordinary configuration key files or raw requests/responses/authentication headers in logs.

After actual client/model calls, compare native per-call usage records, CLI-reported statistics and model-service
counts independently, then verify real application discovery/import/totals/rescans. Never disguise local
model IDs as cloud models; preserve native versions record by record. Failed requests, empty sessions and
successful installations do not verify token formats.

Background calls may be absent from saved sessions. Record known limits using explicit CLI provenance
fields; never subtract main-loop usage from cumulative totals to fabricate events. Controls disabling
background work retain default-scenario conclusions. Only allowlisted data that passed redaction review
may be saved as test samples; original sessions remain in owned build/.
Real Qwen 0.25.0 results: [container acceptance](../../validation/desktop-usage/container-sources.md).

<a id="隔离桌面与后台验收"></a>

## Isolated desktop and background acceptance

`--data-dir <absolute-directory>` controls the application database, task identity and default export
directory; it does not alter Agent discovery. Give subprocesses independent source/configuration environments
as well, avoiding real-source reads/user-configuration writes with only database isolation.

The registry includes observed Kimi Work candidates independent of user paths and Visual Studio TEMP
telemetry roots. Synthetic tests omit HOME/USERPROFILE, set explicit CODEX_HOME and isolated
APPDATA/LOCALAPPDATA/TEMP/TMP, and assert only specified synthetic Agents appear.
OS tasks do not inherit test environment variables. Headless task acceptance persists synthetic manual roots,
enables manual_roots_only and checks nonempty events/tokens through an independent read-only connection
before GUI startup. Later GUI collection cannot establish prior OS collection.

Windows [libuv](https://github.com/libuv/libuv/blob/v1.x/src/win/process.c) adds parent USERPROFILE.
Helpers remove it from the parent only during synchronous spawn, then restore it. Omitting it merely
from the supplied env map does not establish isolation.

npm run test:desktop uses the Windows release client, real WebView2/Tauri IPC and isolated nonempty
synthetic data without IPC mocks. It does not install apps, write real IDE settings or manufacture usage.
System-task round trips register only temporary current-user tasks tied to owned isolated directories,
delete them and verify no leftovers. Native scripts also observe actual minute triggers/exit codes read-only.
Report native UI, system tasks, real Agent samples and CI separately.

<a id="实施与交付检查"></a>

## Implementation and delivery checks

Read Git state, source, configuration, tests and versions before implementation. Preserve user edits;
synchronize affected rules, Skills, designs and Plan.md. Label unimplemented behavior; structural checks
are not runtime records. Record commands, environment, exit codes, test counts, results and gaps.
Do not automatically commit, push or publish.
