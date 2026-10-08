# Latest implementation and acceptance results

<a id="最新实施与验收"></a>

Latest full application round: 2026-10-07, version 0.2.1; portable draft publication:
2026-10-08, version 0.2.2. This is a result index; active
scope/removed conditions are in [Plan](../../../Plan.md), criteria in
[V01–V31](../../design/desktop-usage/validation.md). Detailed records retain complete
commands/counts/environments/first failures/recovery; this page does not repeat every stage.

Local environment: Windows 11 Pro x64 10.0.26300, Ryzen 9 9950X3D, about 125 GiB RAM;
Node 24.21.0, Rust 1.98, Tauri CLI 2.12.0, WebView2 154.0.4258.53, locked dependencies.
Linux uses isolated rootless Podman on WSL/Debian. CI runners/commits are recorded separately.

<a id="当前结果"></a>

## Current results

| Check | Verified result | Record and scope |
| --- | --- | --- |
| Local unified/browser checks | verify exit 0: Rust 1,032, frontend 22, scripts 5; 8 platform tests ignored; clean types. Six Claude tests also pass, overlapping unified counts; earlier Edge alert-display/deduplication checks retained | [VS discovery](m9-vs-copilot-discovery.md), [Codex](codex-today-recovery.md), [round](plan-20261007.md), [Claude](claude-container-sample.md); browser not rerun in this repair; existing simulated IPC/[three-source](m3-container-samples.md)/[M8](m8-container-samples.md) results retained |
| Windows executable | 0.2.1 application release/11 headless cases, local four-call VS check/rescan, Codex old-DB backfill/rescan, Claude new/old-DB correction pass; earlier 19 native WebView2/IPC cases including detail Merge/alerts, 20 startup checks and 8 receiver cases retained | [VS](m9-vs-copilot-discovery.md), [Codex](codex-today-recovery.md), [Claude](claude-container-sample.md), [round](plan-20261007.md), [three sources](m3-container-samples.md); GUI/IPC not rerun in that repair; synthetic DB/source isolation distinct from native samples |
| Windows installation | 12 NSIS upgrade/rollback/uninstall/reinstall/failure-abort cases pass | [Lifecycle](installation-lifecycle.md); owned tasks/startup entries only, other values retained |
| Linux executable/install | Debian workspace 1,004 tests; deb lifecycle and actual AppImage FUSE/GTK/Orca: 9 groups/47 cases pass; earlier 40 checks each at GTK scales 1/2 retained separately | [Three sources](m3-container-samples.md), [lifecycle](installation-lifecycle.md), [ten-language Orca](orca-multilang.md); actual read-only mounts/release on exit checked |
| Native credentials | Two Windows sets of 100 parallel rounds after handling confirmation; six isolated Linux D-Bus/keyring cases and two macOS CI Keychain/HTTP cases pass | [Credentials](platform-auth-continuation.md), [CI](ci-plan-validation.md); earlier Windows revoke anomaly remains unexplained, first failure/residual checks retained |
| Batch remote CI/downloads | Two runs, eight successful jobs each; three archives match API digests/CRC, four packages match SHA-256/size/report revision | [CI 37486581699](https://github.com/owent/llm-usage/actions/runs/37486581699), source 61223e591815a4369a85a00fa23ff1ab2819d6d5; details in [CI record](ci-plan-validation.md) |
| Portable release 0.2.2 | Six native Windows/Linux/macOS x64/arm64 .tar.zst archives; all 12 tag jobs pass after artifact-upload recovery. Six extracted headless checks, both Linux GUI checks and local Windows x64 19 native GUI checks pass; 13 uploaded assets match sizes/digests and overwrite succeeds | [Tag CI 37761919575](https://github.com/owent/llm-usage/actions/runs/37761919575), source 39e74d09fa2b4e300dca3f7d0ba2edf4b6e95ba2; [CI record](ci-plan-validation.md#portable-release-v022). No new installer lifecycle, Windows arm64/macOS GUI or signing/notarization acceptance |
| Scale/resources | Uncached million/ten-million query P95 47.05/133.86 ms; million-event GUI import peak 371.09 MiB; ten-minute idle mean/peak 299.90/363.30 MiB meet revised limits | [Scale](plan-execution.md), [incremental/native](plan-finalization.md); development machine, synthetic sources, whole-process-tree private bytes; original measurements/limit changes retained |

Local Windows/Linux packages and CI downloads are checked separately; CI archive integrity
does not establish local installation. The 2026-10-07 application round retained user-reported
main merge/signing/notarization/Release completion without independently reverifying those
external results. The 2026-10-08 portable draft publication is verified separately above;
it includes no new signing/notarization. Later record-only commits are not another source CI run.

<a id="来源与功能记录"></a>

## Source and feature records

| Scope | Records |
| --- | --- |
| Core storage/source identity/aggregate exchange | [M1](m1-core.md), [M1a](m1a-provenance.md) |
| Complete normalized detail Merge, alerts, Zed external provider | [Round](plan-20261007.md); transactional conflict/replay, single currency/unknown/deduplication and Zed 1.22.0 two nonempty model samples separately; endpoint is Coding Plan |
| Initial Codex/Pi/OMP/Claude/Gemini/Qwen reads/version selection | [Codex](m2a-codex.md), [M2 recovery](m2bc-resumed.md), [version directories](m2d-layout-versions.md), [CLI install scope](wsl-agent-installs.md); initial sample state retained |
| Missing current-day Codex collection | [Rotating reads/old-DB recovery](codex-today-recovery.md); large history no longer postpones unvisited files; local per-record checks/rescan; newer versions remain compatible-only |
| Claude Code 2.1.197 | [Domestic download/native samples](claude-container-sample.md); Zhipu two-model main loop, per-row version, block deduplication, initialized-zero unknown and old DB; Anthropic/other cases require separate checks |
| Qwen 0.25.0 native/SDK and OpenCode 1.18.34 | [Container sources](container-sources.md); choose per-call/native/sealed partitions; title/background coverage separate |
| Cline SDK/Hermes/OpenClaw schema 24/MiMo/Zoo/DSH | [Cline](cline-container-sample.md), [Hermes](hermes-container-sample.md), [OpenClaw](openclaw-container-sample.md), [three sources](m3-container-samples.md); actual nonempty records and current executable upgrades/rescans checked |
| Kilo/Kimi/ZCode/CodeBuddy/WorkBuddy | [M3/M4](m34-kilo-zcode-kimi.md), [Buddy](m4-buddy-local.md), [Kilo health/range](trend-range-kilo.md); CLI/extension/comparison/trace scope distinct |
| M8 second batch | [18 adapters](m8-second-batch.md), [historical ten sources](m8-container-samples.md), [Zed/current container limits](plan-20261007.md); unavailable account/protocol/distribution cases removed from active work; Qoder detection only |
| Copilot four interfaces/quota/telemetry | [Review](m9-copilot-review.md), [VS Code](m9-copilot-chat-local.md), [VS samples](m9-vs-copilot-local.md), [VS cross-version discovery](m9-vs-copilot-discovery.md), [JetBrains inspection](m9-jb-copilot-analysis.md), [configuration](telemetry-setup-ui.md) |
| Dashboard/scheduling/cancellation/concurrency/old-rule recovery | [Native/scale](plan-finalization.md), [queries/background](plan-execution.md), [complete old summaries](parser-conflict-fix.md), [source rules](source-policy-upgrades.md) |
| Costs/online refresh/current API references/archive corrections | [Engine](f2-cost-engine.md), [refresh](f2-online-refresh.md), [references](dashboard-reference.md), [archives](pricing-archive-repair.md); native occurrence-time estimates still require known channels |

<a id="验收边界"></a>

## Acceptance scope

- Native source results apply only to tested versions/formats/nonempty scenarios. Preserve
  unknown fields, unsaved calls and cumulative intervals without zero filling or expanded
  product claims. See [adapter matrix](../../design/desktop-usage/adapters.md).
- DPI 144/UI Automation names, GTK scales 1/2 and Orca ten-language five-page navigation
  are independently valid. ALSA null does not verify physical audio/all controls; control
  names do not establish complete Narrator/NVDA use.
- Linux containers use normal application users/default seccomp, no network/host mounts;
  FUSE adds SYS_ADMIN explicitly only to owned containers. Results do not verify host
  login/logout/full desktop/other distributions.
- macOS native credential/build CI passes; desktop/specific hardware not required this round.
- Synthetic scale/GUI import does not verify other native sources. App scheduling is not
  OS wake; warm empty pages do not establish fresh-WebView minimum overhead; cooperative
  interruption cannot force-cancel OS-blocked reads.
- Observed WebView requests without external HTTP are not whole-process outbound auditing;
  receiver/credential checks do not verify other real exporters.
- User removed more DPI/full screen-reader/host login/logout/OS-wake work. Environment-blocked
  source extensions/F1/system investigation conditions remain in round records; removal is not passing.

Temporary logs/check DBs/raw records stay in ignored task directories under root build/.
Public records contain minimal redacted results; owned receiver credentials, disposable
keyrings and daemons cleaned per acceptance run. User-requested Zed provider keys remain
in Windows credential storage, absent from normal settings.json.
