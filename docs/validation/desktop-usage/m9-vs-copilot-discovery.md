# Visual Studio Copilot version and directory discovery correction

<a id="visual-studio-copilot-版本与目录发现修复"></a>

2026-10-07. Windows 11 x64, PowerShell 7, llm-usage 0.2.1.
Enterprise users reported missing collection, but the affected machine's complete
VS/Copilot versions and file state were not supplied. These results distinguish source
defects, official component checks and native local samples; they do not establish the
affected machine's sole cause.

<a id="已确认原因与修复"></a>

## Confirmed cause and correction

The old adapter had no Community, year or installation-directory filter. It selected
one root from TEMP, ignored TMP whenever TEMP was nonempty, and never checked the user's
default temporary directory before launcher environment changes. Verified VS 18
FileOutputResolver.DefaultBaseLogPath uses
`Path.Combine(Path.GetTempPath(), "VSGitHubCopilotLogs")`.
InstrumentationServiceBuilder.BuildWithDefaultExporters writes
`traces/<short-telemetry-session-id>_VSGitHubCopilot_traces.jsonl`.
[.NET Path.GetTempPath](https://learn.microsoft.com/en-us/dotnet/api/system.io.path.gettemppath?view=net-10.0)
calls the Windows temporary-directory API. For ordinary users, the
[Win32 resolution order](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-gettemppathw)
is TMP, TEMP, USERPROFILE. Different TMP/TEMP values can therefore send discovery and
the VS writer to different directories.

The correction checks TMP, TEMP and the observed LOCALAPPDATA/Temp candidate. When both
temporary variables are absent, use the current USERPROFILE/supplied home fallback.
Do not inspect other users or Windows/SystemTemp. Candidates come only from the supplied
discovery context, preserving manual_roots_only isolation. Manual roots also accept the
temporary-directory parent and share physical-root identity with log/traces directories
and single files. Deduplicate normalized physical directories and use the framework's
bounded single-directory enumeration. Parsing, event identity, cost and history are unchanged.

<a id="跨版本和-sku-依据"></a>

## Version and edition references

| Scope | Copilot installation reference | Verified files/directories and limits |
| --- | --- | --- |
| VS 2026 / 18.x: Community, Professional, Enterprise | Setup Configuration queries installed instances; telemetry discovery does not filter editions | Installed local Community Core 18.10.1203+60f4a0a576 has FileOutputResolver/JsonlOtlpTraceExporter; the temporary-directory writer has no edition/year branch. Native record versions are separately 18.10.1197+4b9e241b86 and 18.10.12217.157; installed versions cannot overwrite history. No local native Professional/Enterprise sample |
| Built-in Copilot in VS 2022 / 17.10+ | [Microsoft installation requirements](https://learn.microsoft.com/en-us/visualstudio/ide/visual-studio-github-copilot-install-and-states?view=vs-2022) | Official VSIX 17.14.1713.63837 Core/Service lacks VS 18 Instrumentation/JSONL exporters. TokenTelemetryHelpers reports VS telemetry. TokenCacheCounting.WriteTokenCountsToCsv defines a temporary VSGitHubCopilotLogs/UsageDetails writer, but inspected Service IL has no call to it. A definition does not establish default disk output. CSV lacks event time and defaults missing fields to zero, so it is excluded. This does not apply to every 17.x release or establish that other local formats are absent |
| Separate Chat/Completions extensions in VS 2022 / 17.8–17.9 | [Microsoft announcement](https://devblogs.microsoft.com/visualstudio/introducing-the-new-copilot-experience-in-visual-studio/), [Chat Marketplace](https://marketplace.visualstudio.com/items?itemName=VisualStudioExptTeam.VSGitHubCopilot) | Actual official Chat VSIX 0.2.765.20217 manifest targets [17.8,17.10), Community, amd64/arm64. Inspected Core/Shared/Vsix types lack that JSONL exporter; persistent sessions use copilot-chat. VS 18's writer cannot verify this separate distribution. No native usage sample in this batch |
| Earlier VS 2022 Completions | [Official legacy extension page](https://marketplace.visualstudio.com/items?itemName=GitHub.copilotvs) retains historical instructions using 1.84.0.1 for 17.4.4–17.5.4 | Earlier completions extension, distinct from built-in Chat starting at 17.10; per-call usage files unverified |
| VS 2019 and earlier | Current official Copilot documentation requires VS 2022; enumeration by vswhere does not establish Copilot support | No verified local Copilot token format; do not fill zeros or construct imagined directories |

The VS 2022 VSIX manifest has InstallationTarget=Microsoft.VisualStudio.Community,
range [17.14,18.0), amd64/arm64. This does not exclude Enterprise:
[official VSIX schema](https://learn.microsoft.com/en-us/visualstudio/extensibility/vsix-extension-schema-2-0-reference?view=visualstudio)
permits lower-edition targets on higher editions, so Community targets also apply to
Professional/Enterprise. Installation eligibility does not replace native usage verification.

Both inspected CopilotSessionProvider generations use IVsWorkingFolders.GetFolder(1,false,true)
for the solution working folder. PersistedSessionFolderProvider appends
`copilot-chat/<account-hash>/sessions`. VS 2022's MessagePack repository persists sessions
and interactions, rather than usage files in the installation directory. Do not assume
all sessions share one LOCALAPPDATA/Microsoft/VisualStudio/17.0_* root or scan every
project. Local VS 18 historical session/log limits are in the [native record](m9-vs-copilot-local.md).

<a id="安装目录获取与只读核查"></a>

## Finding installation directories and read-only inspection

[Microsoft locating guidance](https://learn.microsoft.com/en-us/visualstudio/extensibility/locating-visual-studio?view=visualstudio)
states that VS 2017 onward has no single environment variable/registry value identifying
all instances. Use Setup Configuration API; vswhere is its official native client.
Users can run:

```powershell
pwsh -NoProfile -File desktop/scripts/inspect-vs-copilot.ps1
```

The [script](../../../desktop/scripts/inspect-vs-copilot.ps1) first checks PATH, then the
official Installer tool location, and accepts -VsWherePath. Instance arguments are
`-all -prerelease -products * -utf8 -format json`, without -latest or year filtering.
Component lookup uses official `-find **\Microsoft.VisualStudio.Copilot.Core.dll`,
associating returned components with actual installation roots. Custom locations and
parallel instances are supported without constructing `2022/2026/18/<edition>` paths.
Missing tools/query failures are explicit; no automatic tool installation.

Inspection reads only PE metadata, component versions and temporary-file counts/sizes/
latest write times. It neither loads/executes components nor reads bodies, accounts,
credentials or remote bills. Exporter types establish static capability, without
verifying current nonempty output. Temporary candidates report current .NET resolution,
process/user TMP/TEMP and system-known LocalApplicationData/Temp. Custom roots differing
from the collector environment can be added manually. Distinguish absent directories,
no JSONL and unreadable directories.

Local vswhere returned one instance: 18.10.12224.181,
Microsoft.VisualStudio.Product.Community, at
`C:\Program Files\Microsoft Visual Studio\18\Community`.
This is an observed local path, without claiming every VS 2026 installation uses it.
No other edition/VS 2022 was installed or launched; no Copilot request was sent.

<a id="官方包与首次异常"></a>

## Official packages and first anomalies

VS 17 release channel references catalog 17.14.37710.0, whose downloaded body reports
17.14.41 (September 2026). Invoke-WebRequest and curl.exe returned the same catalog,
but its SHA-256/size differed from channel declarations. Cause is unconfirmed; catalog
integrity is not verified. Actual SHA-256:
`F0A50EA157222C29ABD5EA6FF01BFC3C33B04E011C5E45EE2CA38EF0778E5643`.
Channel declaration:
`6e470016e4324c84c255ffd0beb3767d17ec89cc8561e9409ee3e1f6d29400f5`.

The separately downloaded [Microsoft Copilot VSIX](https://download.visualstudio.microsoft.com/download/pr/bc92e2cb-33de-4a0c-995d-efa817f16b16/b1b9eb236af78de99b4ece7f40525eca0f704c3f498c15ffb46e9963301f5d9d/VisualStudio.GitHub.Copilot.vsix)
matches its payload-declared SHA-256:
`B1B9EB236AF78DE99B4ECE7F40525ECA0F704C3F498C15FFB46E9963301F5D9D`.
Package size still differs from the catalog; retain this anomaly rather than claiming
consistency across the complete distribution process. Extracted Service Core DLL
ProductVersion=17.14.1713-rc+f95dadab4e.RR, Windows Authenticode=Valid, SHA-256:
`FFFB9D471DAB8D08DCE49B4B969A665D794748CE3105A88DA8176E51190E174E`.
ILSpy 11.1.0.9782 was installed only in the task directory to read code, without executing
the VSIX. Static conclusions apply only to this actual package, not other catalog versions.

The official Marketplace publisher's latest/vspackage also supplied the legacy Chat
extension, manifest version 0.2.765.20217, SHA-256:
`CC7BA4862AD8E7C1D7701803ABAB89EE861AB0606233E174A73FDB92781E1529`.
This identifies the download; no independent expected hash means no claimed integrity
comparison. Inspected Core/Shared/Vsix DLLs have
ProductVersion=0.2.765-beta+4ef9459926.RR and Windows Authenticode=Valid; Core signer is
Microsoft Corporation. Only manifest, metadata and IL were inspected; no extension installation.

<a id="验证与剩余范围"></a>

## Validation and remaining scope

Temporary results are under build/vs-copilot-discovery/, without copying original sessions.

| Command/check | Actual result |
| --- | --- |
| cargo test --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core --test vs_copilot_discovery | Exit 0; four passed: both temporary roots, old cursors/whole registry/repeat scans, default user temporary root/deduplication, manual parents/isolation. Old source first reproduced missing default-user/parent discovery. Initial tests also had checkpoint-column/run-ID reuse errors; corrected tests were rerun without calling those errors product defects |
| node --test desktop/scripts/inspect-vs-copilot.test.mjs | Exit 0; synthetic vswhere covers Community/Professional/Enterprise, 17/18, preview and custom paths; does not verify native edition formats |
| desktop/scripts/inspect-vs-copilot.ps1 | Exit 0; all-instance local query and PE metadata succeeded; one traces JSONL, no warnings |
| `cargo run ... --example real_verify_vs_copilot -- <local-traces> <isolated-comparison-directory>` | Exit 0; four calls, input=77,418/output=356/cache_read=57,297, zero diagnostics; independent Python selected-field span sums match, repeat scan adds zero. Retains each record's two actual service.version values; other token fields unknown |
| npm run verify | Exit 0; Rust 1,032 passed/eight conditional tests ignored; frontend 22, scripts five; types, fmt/Clippy, assets and frontend build passed. Documents added afterward had separate Markdown checks |
| npm run build:desktop | Exit 0; Windows x64 release executable/NSIS build succeeded; distinct from installation/GUI/release acceptance |
| npm run test:headless | Exit 0; actual executable/SQLite with isolated synthetic sources, 11 passed |
| New release --scan-once with both temporary roots | Exit 0; isolated synthetic TMP/TEMP each has one nonempty VS trace. Actual default discovery yields two sources/two calls, input=200/output=20; both scans match, no parsing diagnostics/other sources. Initial comparison incorrectly counted normal scan_completed activity as parsing diagnostics; explicit classification fixed the check without product changes |
| npm run lint:md / git diff --check | 210 documents/no issues; diff check exit 0 |

Without affected-machine logs or native Enterprise/Professional/VS 2022 calls, recovery
on that machine is unverified. If valid traces exist, corrected discovery can read them
or accept their directory manually. If the component has no verifiable local per-call
usage, installation path, conversation counts, premium quota and model context limits
cannot substitute for tokens. Other files/older extensions remain unverified; no remote
API access or invented default zeros.
