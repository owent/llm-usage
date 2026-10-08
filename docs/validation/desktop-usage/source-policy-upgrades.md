# Source-rule upgrades and SDK file integration

<a id="来源规则升级与-sdk-file-接入"></a>

2026-10-06, version 0.2.1; uncommitted worktree based on
c2c8f6f1c44f3dc9e398842d9dca2c93da76680c. Cwd: D:/workspace/projs/github/owent/llm-usage;
no commit/push/release. Requirements: [data rules](../../design/desktop-usage/data-contract.md),
[telemetry configuration](../../design/desktop-usage/copilot-otel.md) and
[authentication](../../design/desktop-usage/receiver-auth.md). Native environments/official
download hashes/independent statistics are in [container sources](container-sources.md);
installation baseline is in [lifecycle acceptance](installation-lifecycle.md).

This preserves tested artifacts/results for Qwen/OpenCode/credentials/Orca. Subsequent native
samples, rule corrections and final Windows/Linux rereads for eight M8 clients are in
[M8 container samples](m8-container-samples.md).

<a id="opencode-逐记录版本与旧处理位置"></a>

## OpenCode per-record versions and old processing positions

Previously, the highest database session.version selected the whole file's parsing basis,
including effects from empty sessions. A 50,000-row limit with a 60-second overlap window
could skip unprocessed old rows. Now only sessions owning step-finish usage affect file status;
each event uses its owning session's version. Only native-verified 1.18.34 is known_version;
others remain latest_fallback. Cursors retain updated time, stable row ID and incomplete-window
start. Only a complete valid scan finishes the rule-update marker. Unknown-version/invalid-row
states persist across incremental windows; SQL type errors do not block other valid records.

Old summaries' parse_basis was omitted from metadata normalization, so parser/basis upgrades
still conflicted. Compare complete canonical/legacy summaries and allow only parsing-reference
changes. Token/model/quality/ownership/revision changes retain ordinary conflict handling;
diagnostic history remains and summaries/checkpoints commit transactionally.

Eleven OpenCode/ten metadata-upgrade tests and shared MiMo reading regressions passed, covering
real registry discovery, unchanged old cursors/complete summaries, mixed versions, empty
sessions, same-millisecond pagination, repeat reads, invalid-row recovery and checkpoint rollback.
Native 1.18.34 main-loop data still lacks default title requests; differences do not create events.

## Qwen 0.25.0 SDK file

Verified fixed official source 6788c035698a0ada471c958d1e789e96c6cddd9b against actual files.
FileExporter writes consecutive pretty JSON objects. `resource._rawAttributes` is a key/value
array; LLM span identity is in `_spanContext`, kind=0. Read only this version's per-call
qwen-code.llm_request, without adding api_response logs, HTTP spans or metrics. Object/byte/
count/time limits apply; incomplete/oversized objects retain their starting position. Invalid
objects do not hide subsequent valid calls.

Native main-loop input/output: 10,226/2; automatic background memory: 5,639/556; cache read: three.
API response, LLM spans and CLI independently totaled two calls/16,423 tokens. SDK supplies no
cache-write value or provider reference establishing whether reasoning is included in output.
Leave cache write unknown; derive total only with known reasoning=0, and keep total unknown
for nonzero reasoning. A shared session does not identify parent/child calls.

Choose SDK or native data by host/user/session/local day; choose trace/span copies within the
same host/user. Retain native/copy records with exclusion reasons. Other SDK versions and
unknown ownership/sessions remain isolated. Retained native source/day partitions block
overlapping SDK records even after detail deletion. Scope settings/exclusions/events/checkpoints/
summaries commit together; clearing removes scope identities. Partial coverage adds historical
coverage notices without degrading valid-source health.

Eight requirement tests cover both import orders, repeat scans/copies, different hosts/local
days, retained partitions, unknown ownership/versions, nonzero reasoning, incomplete/oversized
objects, invalid objects/tokens and rollback. Read-only exporter checks and directed discovery
of verified application-managed directories also passed; other supplemental output stays isolated.

An added new-directory-copy regression first exited 101: retained SDK statistics of two calls/
16,423 became four/32,846. After detail deletion, per-call trace/span identities could no longer
select copies. Now complete retained SDK source partitions are checked by host/user/local day;
new output whose nonoverlap cannot be established receives qwen_sdk_partition_sealed exclusion/
coverage notices without changing retained contributions. Another test executed actual
enforce_retention, reopened the DB and scanned late native/SDK copies: two calls/16,423 remain,
zero details, with the persisted cleanup cutoff enforced. A test's manual native file failed
discovery layout; correcting it to actual .qwen/tmp/project/chats was a test fix. All eight
tests finally exited 0.

A new deb read saved native/SDK files in offline rootless Podman: two calls, input 15,865/
output 558/cache read three/total 16,423. One native record and two SDK copies from another
directory were excluded; repeat scans added no usage, Qwen/OTel health=ok. Other adapters'
format diagnostics on the initial manual-copy directory are separate from SDK parsing failures.

<a id="实际包发现的来源路由缺陷"></a>

## Source routing defects found by the actual package

The actual new package let Claude's shared type detector claim a manual Qwen .qwen root:
12 sources/zero events/eight diagnostics. QWEN_RUNTIME_DIR instead produced one event/10,228.
Allowlisted inspection of the first native user record/fixed ChatRecord source confirmed
Qwen message.parts/usageMetadata/complete identity. This explicit layout/shape now directs
only to Qwen; a directory name/shared type alone does not verify a format. Incorrect ownership
is restored only when the entire old source has no events/daily/period/native summaries,
releasing that file's incorrect checkpoint while retaining diagnostics/enabled state/source
bytes. Sources with historical contributions do not migrate. The repaired package's manual
root found one Qwen source/event and no diagnostics. Full-registry consumed-cursor/invalid-row/
retention-boundary regressions passed.

Goose first opened direct manual SDK JSON as SQLite, then failed its source with NotADatabase.
It now checks the 16-byte SQLite magic first. Non-SQLite still receives unknown_format, allowing
OTel to claim an explicit SDK format without duplicate adapter registration on repeat scans.
Existing M8 SQLite requirement tests and targeted regressions passed.

<a id="windows-原生凭据失败与修正"></a>

## Native Windows credential failures and correction

Reproduced credential_store_unavailable while creating credentials. Diagnostics contain only
operation/numeric error code/missing-different-failed states, without credentials/targets/
authentication headers. CredWriteW succeeded, immediately followed by missing CredReadW;
after 10 ms the complete content matched, still matching at 50/100 ms. Only mutation flows
wait up to 50 ms on missing reads. Different content/read errors reject immediately;
authentication requests still read once. Failed-creation cleanup/revocation compares complete
owned content and confirms deletion. If identical content remains, Windows waits up to 50 ms;
external replacement/errors/exceeded limits fail without another deletion.

After the write correction, two 100-round parallel groups passed. Earlier, a revoked credential
remained across processes; another occurrence after adding deletion confirmation reproduced
at round 60 and still authenticated after 10/50/100 ms. Child exit success did not establish
input processing. Adding --nocapture and explicit handling acknowledgments yielded two
100-round/two-thread parallel groups passing with complete binding processing confirmed.
The earlier revocation failure's root cause remains unknown; later passes do not close it.
Initial/final results remain separately under build/plan-final-push/.

Four exact test directories identified by failure timestamps contained two complete owned
bindings, precisely reclaimed. Another 40 parallel create/revoke operations passed with zero
owned residue; other sources' credentials were left alone. Final checks of five exact
directories, including new failures, found no pending bindings; another 40 round trips passed,
zero owned residue. Official [CredWriteW](https://learn.microsoft.com/en-us/windows/win32/api/wincred/nf-wincred-credwritew)
and [CredReadW](https://learn.microsoft.com/en-us/windows/win32/api/wincred/nf-wincred-credreadw)
specifications do not explain this timing. The observation does not establish system-cache
or thread-safety behavior.

<a id="linux-真实屏幕阅读器"></a>

## Actual Linux screen reader

Added distribution packages Orca 48.1-1+deb13u2, Speech Dispatcher 0.12.0-5/espeak-ng,
pyatspi 2.46.1-1 and xdotool to the fixed Debian GUI image; image ID
4575f4ba7437ce291ce8e1633292f232ad5899d1551c06ed20aced61f3fc212d. Ordinary UID 1000,
independent D-Bus/X11, default seccomp, no network/host mounts, ALSA null audio. Debug options
were checked against [official Orca instructions](https://orca.gnome.org/debugging) and installed commands.

Initial actual focus events had unnamed navigation buttons; Orca spoke only button. A temporary
DOM probe adding explicit names read all five items. The product therefore adds localized
aria-labels to navigation/brand links, retaining names when narrow sidebars hide text. Language
preparation uses/saves the actual settings form, without simulated backend IPC frontend updates.
Tests with unknown language values/overbroad selectors including settings subnavigation remain
recorded as test defects.

Reusable test:install:linux adds --screen-reader as an independent round after reinstalling
the current package. Native Tab/Enter, actual AT-SPI button focus, five page titles and actual
speech output are compared without injected names. Orca 48.1 debug.py buffers output; formal
checks once failed because speech existed before the file flushed. Installed /usr/bin/orca
opens this debug file with open(..., 'w'). The test launcher changes exactly that line to
buffering=1; other event/speech code stays unchanged, and changed upstream text causes rejection.
Wait for actual speech and recheck AT-SPI focus. Confirm PIDs by parent/UID: Orca changes its
process name, so a launch command alone cannot identify ownership. Logs retain normal-stop
timeouts; final owned reader/speech cleanup uses bounded TERM/KILL/wait without claiming Orca
shutdown reliability. Product assistive-technology defaults remain unchanged.

The formal current-package check exited 0: AT-SPI names and speech for Overview, Trends,
Sources, Details, Settings matched actual page titles after native Tab/Enter. Brand link spoke
LLM Usage. Narrow-sidebar screenshots show icons only while explicit names remain complete.
Results/logs/screenshots: build/plan-final-push/; full container lifecycle:
WSL build/install-lifecycle/linux/1791264897354/. This does not verify physical audibility,
all controls, ten-language screen readers or Windows assistive technology.

<a id="本阶段检查与待补边界"></a>

## Stage checks and remaining coverage

Final npm run verify exited 0: 927 Rust tests, core 826/app 101, eight ignored by default;
21 frontend/four script tests; Markdown 191 files, assets, Svelte zero errors/warnings,
fmt/Clippy -D warnings/frontend build passed. Debian workspace: 924 tests, core 826/app 98,
nine ignored by default; six explicit system-credential tests passed, keyring reclaimed.
Current Windows release headless 11/receiver eight/NSIS lifecycle 12 checks passed, zero owned
credential residue; NSIS output: build/install-lifecycle/windows/1791264668246/.
An install prerequisite check correctly blocked a concurrent desktop test before running any
checks; sequential retry after desktop exit passed, without claiming an installation defect.
Final native release desktop: 17 checks/20 startups, P95 747.1 ms, exit 0;
build/plan-completion/native/1791264943840/. Edge simulated-IPC language/theme/selection/filter/
pagination/narrow-layout regressions exited 0. Current Linux deb/FUSE/GTK/Orca lifecycle:
nine groups/47 checks passed. Earlier scaling-factor-two/40-check results remain separate;
this stage did not repeat that scale.

Final deb reread of Qwen native/SDK/direct manual copies yielded two calls/16,423, excluding
one native/two SDK copies; target-source health normal, retaining one initial unrelated
format-detection diagnostic. Two original OpenCode DB rereads each yielded one call/299;
cache read three/zero, health=ok, parser=opencode-step-finish-parts-2, known_version;
repeat scans added no usage.

| Actual artifact at this stage | Bytes | SHA-256 |
| --- | ---: | --- |
| Windows exe | 9,989,120 | 5b3e16b2d9be5d4f25bd4886ea521a109a7d5f06ee6ddd06f2c22b23fb1e3073 |
| Windows NSIS | 3,941,612 | e34aa3eebade224e865dac185a157081634a7035e2244971fda2bae2d3060076 |
| Debian deb | 5,466,678 | 69abe4b2463459d2969bf582d874d1e82796b14e77e5823f4ea5f38dc1adbd46 |
| Linux AppImage | 111,143,416 | 76b33bc5500672ba3dd6f0d115bd874fc453eac69b3283a7552da76b25137457 |

Both Windows/Linux builds exited 0. Old 0.2.0 package/image hashes remain in installation
history. Disposable databases/scripts/raw verification artifacts/logs stay under root build/;
only affected requirements are synchronized to rules/Skill.

Still pending: nonempty native samples for other products/versions, OpenCode title per-call
records, Qwen cloud/main-loop cache hits, other exporter authentication/retries/sampling/
parent-child spans, new Copilot CLI/JetBrains native exports and complete outbound audits.
macOS desktop/specific-hardware requirements were removed. Native macOS credentials, actual
assistive technology, host login/logout and remote CI/release retain separate environment/
authorization dependencies; local builds/container exits do not establish those results.
