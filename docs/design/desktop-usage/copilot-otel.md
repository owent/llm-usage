# Copilot supplemental collection and OTel configuration

<a id="copilot-补充采集与-otel-配置设计"></a>

Distinguish missing configuration, configured but waiting for data, verified samples and
configuration conflicts. Overview shows a compact summary. Details check actual output
read-only every 30 seconds. When indistinguishable profiles share an output, do not claim
each profile's settings took effect. Existing data does not remove policy conflicts or
establish complete history. JSONC merges preserve formatting without repeated blank lines;
see [interactions](dashboard-polish.md) and
[validation](../../validation/desktop-usage/dashboard-polish.md).

Asynchronous user-configuration checks, overview notices and merged settings controls are
implemented. Trace SQLite parsing is not. Configuration development acceptance is in the
[implementation record](../../validation/desktop-usage/telemetry-setup-ui.md); native-format
results are in the [M9 review](../../validation/desktop-usage/m9-copilot-review.md).

Automatic collection of application-configured VS Code Copilot file output is implemented
and verified using copies of actual exports; see [dashboard rules](dashboard-repair.md)
and [results](../../validation/desktop-usage/dashboard-repair.md).
CLIENT chat &lt;model&gt; spans represent individual calls. For the same host/user/session/local
day, OTel replaces the native contribution while retaining native records. Rescans do not
add both; archived partitions retain native contributions and exclude overlapping new spans.
Never infer call identity from close timestamps or equal tokens. The enabling day may lack
earlier calls; exports do not establish complete history. New CLI/JetBrains versions require
independent native checks; other isolated exports do not automatically become statistical sources.

<a id="补充路线与取舍"></a>

## Supplemental collection options

| Option | Additional data | Verified scope and limits |
| --- | --- | --- |
| VS Code Copilot Chat OTel file | Per-call input/output/cache/reasoning and subagent calls | Fixed source and 30 local CLIENT spans accepted; discover application-managed output; absent fields stay unknown |
| VS Code Agent Host OTel file | Native runtime per-call records | Separate configuration namespace; SDK service.name cannot alone establish an independent CLI |
| VS Code local trace SQLite | IDE-retained spans and existing history exports | Official documentation/schema checked; no adapter implemented; opt-in with IDE retention limits |
| Copilot CLI OTel file | Future per-call tokens absent from latest chronicle | Official variables checked; native row format still needs verification; cannot recover already-lost historical tokens |
| Visual Studio automatic traces | Verified VS 18 calls/cache reads | Does not verify all VS 2022/older extensions; check TMP/TEMP/default user temp; more frequent collection reduces cleanup loss; [cross-version analysis/tool](../../validation/desktop-usage/m9-vs-copilot-discovery.md); never invent exporter settings |
| JetBrains Copilot debug File Logging | Plugin OTel outfile | Static checks for 1.18.0-261 only; manual setup without native IDE acceptance; no direct plugin XML edits |

Prefer local files: IDEs/CLIs may keep writing after this application exits, for later scanning.
Local OTLP/HTTP requires the application to run. Support JSON/protobuf /v1/traces and
/v1/logs plus isolated /v1/traces/supplemental output. Authenticate each source first.
With native credential storage, automatically configure Claude/Codex logs and isolated
CodeBuddy CLI 2.98.0 traces verified by the adjacent npm manifest. Protocol/rate-limit tests
do not verify every sender/version's authentication configuration; see
[authentication requirements](receiver-auth.md). No gRPC/metrics support; this is not a general Collector.

<a id="已实施的用户配置入口"></a>

## Implemented configuration controls

After startup, bounded background discovery checks installation manifests and agent launchers
on PATH. User directories locate settings without establishing installation; leftover directories
after uninstall do not appear. Checks are read-only. Overview shows only a short notice,
Enable all and View details. Details opens Settings → Local telemetry, listing installed
agents supported by these checks, with batch configuration, individual preview/apply/revoke
and recheck. Do not show templates for uninstalled agents.

Pages share results without blocking statistical queries, periodically writing settings or
executing agent launchers. Opening the settings panel refreshes installation/configuration
read-only. CLI detection currently requires a PATH launcher; a nonstandard installation
outside PATH is not inferred from its user directory.

The batch button processes currently missing, automatically configurable items one by one:
create a plan and immediately merge/apply it. Preparing all plans beforehand would make later
shared-file concurrency checks stale after earlier writes. Skip existing settings/manual items;
one failure does not stop later items. Report success/failure/manual results separately.
Batch state and revocation remain across pages; overview omits paths/keys/per-item results.
Users must run generated CLI launchers or reload file-configured IDEs/agents themselves;
the application does not start them.

| Client | Automatic changes | Output and verification limits |
| --- | --- | --- |
| VS Code / Insiders Copilot | Default user and existing profiles; check OTel keys declared by installed manifests; file export with content/identity disabled | Inspect Windows version directories and normal extension directories; merge sync exclusions; explicit sync conflicts require manual setup |
| VS Code Agent Host | Separate chat.agentHost.otel when installed app manifest is verified 1.140.0 | Separate file per profile; other versions pending; extension keys cannot establish host support |
| Copilot CLI | Dedicated PowerShell/sh launcher inside application storage, setting official file variables only for its process | No config.json/settings.json/global-environment/shell-profile edits or automatic execution |
| Gemini CLI | User settings.json telemetry enabled/local/outfile, logPrompts=false | Append .gemini after GEMINI_CLI_HOME replaces home; actual new-version output not accepted |
| Qwen Code | User settings.json telemetry enabled/outfile; prompts/sensitive spans disabled | QWEN_HOME is the config directory; QWEN_RUNTIME_DIR does not replace it; 0.25.0 consecutive multiline SDK JSON supports per-call/native partition selection; discover/check managed output read-only; isolate other versions; see source-upgrade record |
| Claude Code | User env with dedicated HTTP/JSON logs endpoint and authentication header; content disabled | Restricted logs remain isolated; per-source credentials in current-user system storage; actual exporter still pending |
| Codex | User TOML HTTP/JSON logs, separate authentication header, log_user_prompt=false | Preserve other tables/provider configuration/headers/comments; logs never automatically add to native usage |
| CodeBuddy | First PATH launcher's adjacent npm manifest must be 2.98.0; generic headers with Bearer space encoded %20; content disabled | Isolated supplemental traces only; other versions, absent manifest or existing traces-specific headers require manual setup; local JSONL remains independent |

Verified VS 18 components export automatic traces; check VS 2022 components/actual files
independently without inventing user exporter settings. JetBrains remains manual.
Do not automatically write portable/custom user-data, other IDE or remote-host configuration;
follow official instructions. Preserve enabled output destinations. An existing destination
with export disabled requires manual review. Known environment overrides, managed files,
disabled telemetry, sync conflicts or configuration errors block automatic changes.
User settings do not establish effective enterprise/workspace/other-launch-environment settings.

Individual preview shows paths, exact keys/values, output location and receiver effects before
apply. Merge JSONC using AST value ranges and TOML with a format-preserving editor. Check
BOM/comments/duplicate keys/size limits/links/read-only state/concurrent edits. Back up original
bytes first, then replace through a temporary file in the same directory. If receiver binding
fails, do not write agent settings. Configuration failure restores this operation's receiver state.

HTTP apply generates per-source tokens, reads back system storage and reclaims newly owned
credentials on failure. Previews show placeholders without returning existing header secrets.
Revoke credentials before conditionally restoring user keys; other sources keep working.
Profile sync exclusions belong in the default user file: preview both files and check both
versions before applying. If the profile write fails, conditionally roll back default settings
without overwriting intervening user edits. Revoke only values still equal to this feature's
writes; retain later user edits and report conflicts. Preview/revoke tokens last only within
the current app process; original backups remain local after restart. Revoking one client
does not automatically disable the shared receiver or delete collected files/history.

OTel adds only actually exported records after enabling. Check cache/reasoning/failure/auxiliary/
retry/inline coverage individually; do not promise tokens for all Copilot requests. Never add
parent invoke_agent, child chat, same-call events and token histograms. Premium quotas/AIU
stay separate, without token conversion.

<a id="手工配置"></a>

## Manual configuration

These examples generate files; they do not establish complete end-to-end acceptance.
Create a dedicated local directory and use actual absolute paths without assuming outfile
expands environment variables or ~. Retain native statistics first; after checking the new
file, select the statistical source rather than immediately mixing totals.

<a id="vs-code-扩展宿主"></a>

### VS Code extension host

Merge these keys in user Settings JSON. Replace the username and create the parent directory:

```json
{
  "github.copilot.chat.otel.enabled": true,
  "github.copilot.chat.otel.exporterType": "file",
  "github.copilot.chat.otel.outfile": "C:/Users/<username>/AppData/Local/llm-usage/copilot-otel/vscode.jsonl",
  "github.copilot.chat.otel.captureContent": false,
  "github.copilot.chat.otel.captureIdentity": false
}
```

Reload the window and check the file. After the user's next normal Copilot usage, verify chat
span identity/time/usage. Configuration checks do not send test prompts. User-selected files
use manual roots; automatically configured Copilot files are discovered. Inspect coverage
diagnostics/independent comparison first; handle overlap using the rules below.

### VS Code Agent Host

Use the host's own namespace and a separate file when using Agent Host sessions:

```json
{
  "chat.agentHost.otel.enabled": true,
  "chat.agentHost.otel.exporterType": "file",
  "chat.agentHost.otel.outfile": "C:/Users/<username>/AppData/Local/llm-usage/copilot-otel/agent-host.jsonl",
  "chat.agentHost.otel.captureContent": false
}
```

Personal settings bind at host startup; restart the host/window. Include only local runtimes.
Do not integrate remote Agent Host, SSH or cloud sessions. WSL/containers use separately
authorized instance boundaries.

### Copilot CLI

Set process variables in the PowerShell terminal that will launch the CLI:

```powershell
$copilotOtelDir = Join-Path $env:LOCALAPPDATA 'llm-usage/copilot-otel'
New-Item -ItemType Directory -Path $copilotOtelDir -Force | Out-Null
$env:COPILOT_OTEL_ENABLED = 'true'
$env:COPILOT_OTEL_EXPORTER_TYPE = 'file'
$env:COPILOT_OTEL_FILE_EXPORTER_PATH = Join-Path $copilotOtelDir 'cli.jsonl'
$env:OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT = 'false'
```

Use the CLI normally in that terminal; existing processes do not inherit new variables.
Closing the terminal ends this process configuration. Automatic setup provides a dedicated
launcher that restores PowerShell's previous process variables when it exits.

<a id="http-接收方案"></a>

### HTTP receiver

Claude/Codex and verified CodeBuddy 2.98.0 setup check native credential storage/binding,
immediately enable the receiver and create independent local-instance authentication.
CodeBuddy always uses isolated supplemental output. Backend keys are otel_receiver_enabled
and otel_receiver_port, default off/4318; they never bypass authentication. Existing
unauthenticated application HTTP settings need another preview/confirmation, without
automatic reuse. Copilot per-source HTTP setup remains unverified; use file export above.
Unverified CodeBuddy versions receive no automatic setup. Pointing an endpoint at the
application alone does not establish integration; do not configure remote Collectors.

Environment overrides, enterprise policy and VS Code telemetry switches may prevent settings
taking effect. Show conflicts without overwriting policy. HTTP exporters may send metrics:
the receiver rejects them and stores logs independently.

<a id="后续接入设计"></a>

## Additional integration design

1. Add source-page supplemental Copilot collection by client/version/host, showing coverage gaps, official instructions and copyable settings,
   including not-installed/unsupported versions. Prefer file then HTTP.
2. Preview paths/keys/output/source effects. Users choose installation/profile/user-data directories; check JSONC/environment/policy without
   scanning or printing credentials.
3. After Apply, back up original bytes and modify only listed keys while preserving comments/other settings. Compare file versions before
   writing; concurrent edits need another preview. Restore this operation on failure and report actual results.
4. Distinguish configuration written, reload required, waiting for records and parseable calls received. Check only allowed identity/time/token
   fields and rescans; file existence does not establish acceptance.
5. Revoke only changed keys still matching this feature's writes; preserve later user edits with conflict notices. Backups stay local, out of
   logs/exports. Never automatically restart IDEs or create model requests.

The implemented table above owns current scope. Initial acceptance is Windows. Isolated
WSL checked Linux native credentials/HTTP; macOS credential code passed cross-platform
type checking only. Actual desktop/exporter checks remain separate on both platforms.
Modify user settings only, never project .vscode/settings.json. Merge sync exclusions without
enabling Settings Sync. JetBrains stays manual; Visual Studio uses verified existing files.
Managed VS Code Copilot file discovery, session-range source selection and actual existing
exports were verified; see [dashboard repair](../../validation/desktop-usage/dashboard-repair.md).
New CLI/JetBrains exports, trace SQLite and more real revocation scenarios remain unverified.
Extensions without current installations/files left this round's plan with limits preserved.
Session-store does not substitute for a trace database.

<a id="统计选择前置条件"></a>

## Prerequisites for statistical source selection

Route managed Copilot telemetry directories only to OTel, not every adapter's manual roots.
Native vscode-copilot-chat still reads chatSessions. These are distinct formats, selected
under the rules below. One physical file has one active adapter owner. Old incorrect sources
without usage stop appearing in the source list. A recognized format may take over an
unrecognized file with no committed cursor/usage, preserving the DB, diagnostics and disabled
settings. Verify recovery through real registry discovery, including adapters promoting
roots to parents; matching only the supplied directory is insufficient. Rebuild discovery
for application-managed inputs only. Never hide sources with usage/period history.
Metrics/logs may appear first; collect only verified call spans, not metrics as per-call usage.

set_source_enabled stops scanning; load_daily_rows still includes disabled-source history.
Disabling native collection then adding OTel cannot retract already stored overlap.
VS Code file parsing uses trace+span identity. Copies across files/receiver contribute once
within the same original host/user scope. Native turns and OTel calls lack common call IDs:
equal tokens/close times cannot pair them. A mixed-agent receiver cannot be disabled wholesale
to switch Copilot.

Verified VS Code file selection uses original host/user/session/local day. Establish
selection only with valid OTel calls. Retain native records but exclude their contribution
from selected partitions on later rescans. Other sessions/days remain native; archived native
partitions cannot add new overlapping telemetry. Selection applies to daily/hourly queries,
charts and exports. The enabling day may lack earlier exports; expose coverage without
promising lossless combination. Missing session identity cannot replace native contributions;
never split a whole turn's consumption using its timestamp at the switch boundary.

Implemented corrections count chat calls without usage, use trace+span identity and strictly
validate integer tokens; skip mixed logs/metrics and distinguish SDK CLIENT from OTLP enums.
Local results verify VS Code's embedded host. Subagent/other-runtime classification and
actual records require separate checks. Configurable service.name cannot establish host
identity. Prefer explicit relationships among same-origin file/receiver/SQLite records;
without verified relationships, select one statistical source.

<a id="验收与回滚"></a>

## Acceptance and rollback

Configuration regressions cover JSONC comments/BOM, existing profiles, known environment/
policy conflicts, absent directories, read-only files, concurrent edits, repeated apply,
failure recovery and revocation after user edits. Compare independent expectations with
details/summaries for failed calls without tokens, mixed agents, parent/child spans,
retransmission, same-origin file/HTTP/SQLite, source switching and archived contributions
after cleanup.

Portable/custom user-data auto-discovery is not implemented. This round read only actual
VS Code output from the user's earlier automatic configuration, without rewriting settings,
reloading hosts or issuing model requests. Do not report those operations as tested this round.
Actual acceptance should record settings writes/reload/normal-user-call output; synthetic
data cannot represent real calls.

Future trace SQLite uses read-only connections or IDE export while respecting WAL. Copying
bare .db files may lose uncheckpointed data. Query necessary fields only, without exporting
bodies/span_events. Unknown schemas remain limited. Trace DB, chronicle and session-store
are distinct databases. Revocation preserves historical statistics, stops this feature's
export/reading, and permits independent selection back to the native source.

<a id="依据与本轮验证"></a>

## Sources and this round's checks

Common metadata: verified_at=2026-10-01, owner=repository maintainers. Recheck before related
work, upgrades or configuration/export/policy changes. Installed versions come from M9,
without claiming fresh installation checks. C01 native VS Code file acceptance is dated
2026-10-02 and does not expand other product interfaces.

| ID / scope | source_url / source_version | method / status / impact |
| --- | --- | --- |
| C01 Extension calls, file and trace DB | [Official monitoring](https://code.visualstudio.com/docs/agents/guides/monitoring-agents), 2026-09-30 page | Official text; 30 actual VS Code file chat calls accepted 2026-10-02; trace DB unimplemented |
| C02 CLI variables and CLIENT chat | [Official CLI reference](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-command-reference#opentelemetry-monitoring), rolling | Official text informs launchers/fields; actual new export not accepted |
| C03 Independent Agent Host, file/DB and startup binding | [Fixed source](https://github.com/microsoft/vscode/blob/dc546cc3c9979a19adafccd439889d7b64298def/src/vs/platform/agentHost/OTEL.md) | Fixed commit; informs host selection; main snapshot is not installed-version verification |
| C04 File and SQLite formats | [fileExporters](https://github.com/microsoft/vscode/blob/dc546cc3c9979a19adafccd439889d7b64298def/extensions/copilot/src/platform/otel/node/fileExporters.ts), [otelSqliteStore](https://github.com/microsoft/vscode/blob/dc546cc3c9979a19adafccd439889d7b64298def/extensions/copilot/src/platform/otel/node/sqlite/otelSqliteStore.ts) | Fixed source: one span per file line; DB schema_version/spans/span_attributes; references for future parsing |
| C05 Policy/environment and telemetry switch | [otelConfig](https://github.com/microsoft/vscode/blob/dc546cc3c9979a19adafccd439889d7b64298def/extensions/copilot/src/platform/otel/common/otelConfig.ts), [official policy](https://code.visualstudio.com/docs/enterprise/manage-ai-settings#configure-telemetry-export-with-opentelemetry) | Descriptions differ on precedence; use installed-version implementation without forcing overrides |
| C06 JetBrains 1.18.0-261 | [Existing analysis](../../validation/desktop-usage/m9-jb-copilot-analysis.md) | Static checks without a native IDE environment; version-limited manual instructions |

Public snapshots/SHA256 are under ignored build/copilot-otel-setup/. During the earlier
documentation stage, root npm run lint:md (162 files), python build/copilot-otel-setup/check-links.py
(4 documents/77 relative links) and git diff --check all exited 0. That stage changed no
product code, reran no Rust/browser tests, enabled no OTel and made no model requests.
Current implementation/source results are linked above.
