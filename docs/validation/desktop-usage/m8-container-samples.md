# M8 real container-source samples

<a id="m8-容器真实来源样本"></a>

Date: 2026-10-06, uncommitted 0.2.1 working tree. Task artifacts are under repository-root
build/plan-final-push; WSL copy: /home/owent/llm-usage-platform-auth-20261005. Retain only
selected fields under [readiness rules](../../design/desktop-usage/implementation-readiness.md).
Container clients called a real local model, without personal accounts or paid cloud requests.

Later OpenClaw builds reread all ten original sources/old databases plus Hermes/Cline/Qwen/
OpenCode with identical values/coverage. Windows 12/17/8 checks and Linux 47 passed again;
latest digests/commands are in [the OpenClaw record](openclaw-container-sample.md). Earlier
digests below retain their historical scope. The later 2026-10-07 Zed native external-provider
acceptance is separately recorded in [that round](plan-20261007.md); it supersedes the historical
hosted-only limitation in the remaining-route table without changing these ten-source results.

## Aider 0.86.2

[Official Docker installation](https://aider.chat/docs/install/docker.html) names paulgauthier/
aider; [compatible API configuration](https://aider.chat/docs/llms/openai-compat.html) supports
the local endpoint. The 0.86.2 image tag did not exist: first pull exit 125. The documented
image subsequently reported aider --version 0.86.2 and was pinned before execution:

- Image: docker.io/paulgauthier/aider@sha256:764924922f1f9a47e1185ebaa72e6435027af657deaff3febf0f20a176403c1e.
- Image ID: 4596c7c132b3d8f9594e81324c46cbc588b95847929325478d60b37d6d63b647.
- Original analytics.jsonl SHA-256: 93fa009a0c7e92544d54cb15a5488245c69c398018c24fa007d6e6c4c3e2924c.
- Sample: core/tests/fixtures/aider/real-0.86.2, redacted analytics, independent API usage and provenance.

Client/model ran as ordinary UID 1000 in rootless Podman, without external networking or host
mounts, using loopback within their isolated shared network. Real llama.cpp/Qwen 2.5 0.5B
inference retained model identity qwen2.5-0.5b-local. Ask mode, nonstreaming, Git/autocommit/
update checks/network analytics disabled, explicit --analytics-log. Zero local-model metadata
rates do not establish cloud prices.

Installed analytics.py guards/write paths show logfile writes local events even with network
analytics disabled; base_coder.py writes message_send tokens/cost after sending. Six native
lines are launched, no-repo, auto_commits, message_send_starting, message_send and exit; only
message_send has usage. Logs lack product version. 0.86.2 describes sampling provenance;
aider-analytics-doc-1 stays a format reference without verifying other record versions.

Independent model response prompt/completion/total 95/3/98 matches CLI “Tokens: 95 sent,
3 received.” and application one call/95 input/3 output/98 total. API cached_tokens=0 was
not written to analytics: cache/reasoning components remain unknown. Local cost=0 remains
Estimated; cumulative total_cost is not added.

First root aider-1791265417 had no usage: the test proxy incorrectly required explicit
stream:false, while the client omitted this optional field. Proxy AssertionError caused
retries. Correcting the nonstreaming default under [API documentation](https://developers.openai.com/api/reference/resources/chat)
did not fabricate responses. The first save lacked its target directory; cleanup also tried
to remove shared-network containers in the wrong order. Logs retained, directory created,
client removed before model; successful root aider-1791265647.

The accepted package reread the original log in a separate offline container: complete scan/
repeat one call/98 tokens, aider health=ok, root aider-readback-1791265745. The manual file
also triggered Zed format diagnostics as another degraded source; this is not Aider parser
failure. New aider_contract verifies full-registry results, unknown components, Estimated
cost and repeat, exit 0. Scope: one successful ask call; cloud/edit loops/multiple sends,
failure/retry usage, branches/cache/other versions remain unaccepted. Capability text updated;
later package rechecks are recorded separately.

## Goose 1.53.0

[Official release](https://github.com/aaif-goose/goose/releases/tag/v1.53.0) API supplies
goose-x86_64-unknown-linux-gnu.tar.gz, matching official SHA-256
deb2191a6b75acc0a20232fc5c52655ea2f9cc8fa2f5dffc8622e8d378a915dc.
Binary --version 1.53.0. Fixed cli.rs/openai.rs/session_manager.rs verify --no-profile/
--max-turns, custom unauthenticated nonstreaming endpoint and post-response usage_ledger.
Container-only custom_local follows [official configuration](https://github.com/aaif-goose/goose/blob/v1.53.0/documentation/docs/getting-started/providers.md),
without host settings/credentials changes.

Offline rootless ordinary-user execution, no host mounts, same real model, extensions disabled,
one turn/named session. API/per-request DB match one call/320 input/2 output/322 total.
cache_read_tokens=0; cache_write_tokens/cost/cost_source NULL. Session cumulative cache_write=0
comes from unwrap_or(0) and cannot replace unknown ledger values. --stats contained no token
numbers; no CLI-statistics three-way match is claimed.

Original sessions.db SHA-256: f1c1d714b195aa1795c50ed2db1795613d868a07340cbf0b1a563384fdfebdcf.
Success goose-real-1791266205; new deb default discovery/repeat one call/322, only goose
health=ok/no diagnostics, goose-readback-1791266290. Real schema/selected rows:
core/tests/fixtures/goose/real-1.53.0. New goose_contract verifies full-registry discovery,
exclusive ledger/cumulative selection, unknown cost/cache-write/uncached input, unchanged
original bytes and repeat, exit 0. Client version is provenance; goose-usage-ledger-1 stays
the format reference. Initial archive check expected only goose but found ./ and ./goose,
exit 1. Retain failure, strictly allow those actual entries without bypassing checks/redownloading
verified archives. Desktop/legacy DB/rewind/fork/subagent/compaction/other versions remain unaccepted.

## Continue CLI 1.5.47

[Official YAML configuration](https://docs.continue.dev/cli/configuration) and
[OpenAI-compatible endpoint](https://docs.continue.dev/customize/model-providers/top-level/openai)
connect the real local model. Official @continuedev/cli@1.5.47 integrity matches the lock:
sha512-gtpewV3RoIOD9dyTtKIBi1SY0VOHRu3Ehe7C/mmnswm+j34MPyrcQhQaWj/m+jdfGO4fNIKdrgGIlLso1ULDFw==.
Installed isolated image ec002ed485dbcc28879a695c09da7f33ef3a334dc97bcdfb01aefd13897e4dde.
Dependencies downloaded during preparation only; runtime offline/no host mounts, ordinary
acceptance user, read-only mode/independent CONTINUE_GLOBAL_DIR, real model name preserved.

Forward real SSE unchanged. Client requests include_usage=true; proxy retains model/status/
final usage without request/response bodies. API one call/1,471 input/2 output/1,473 total;
CLI answers OK, native cumulative usage 1,471/2. Only session aggregates persist: no original
total/per-request identity/model/interval start. App invents neither total nor calls and does
not turn mtime endpoints into complete daily ownership. Daily totals stay unknown; independent
session aggregate retained. totalCost=0.001475 is not imported as provider cost.

Installed session.ts:74–81/130–137 initializes cache buckets to zero; 149–177 increments
only nonzero responses. API cached_tokens=0/cache write absent nevertheless writes both
cache fields zero. The format cannot establish API-reported zeros. continue-session-usage-2
keeps zero cache unknown, positive native cumulative values, unchanged input/output and no
complete total inferred across mixed providers. Automatically reread unchanged old cursors;
complete old aggregate summaries permit only both 0/reported → NULL/unknown at the same
source revision. Other tokens/quality/coverage/higher old revisions use existing conflict
handling. Aggregates/fingerprints/cursors commit together; retain old diagnostics/new upgrade
diagnostic without editing sources/sealed history.

Original session SHA-256 eb93d5dba3db44af18952e9e2a5dadf80d223a53ddec3e9575bc4c01e7fa023f,
mtime=1791266584000; successful inference continue-real-1791266577. First extraction treated
sessions.json index array as a session object, exit 1. Save complete files, distinguish objects,
extract read-only without repeating inference. Redacted core/tests/fixtures/continue/real-1.5.47.
Three continue_contract checks exit 0: real cumulative/unknown components; unchanged cursors/
complete old-summary repair/parallel wrapper/rollback; token-quality-coverage conflicts/higher
old revision protection. First test used the wrong parallel API, compile exit 101. Later
assertion wrongly assigned an unknown-start aggregate to today; corrected under interval
rules to independent aggregate/unknown day. Neither was a product defect. M8 19, parallel 3,
rollup 4 checks passed. Cloud/versions/multimodel/resume/fork/GUI unaccepted.

Real old/new deb round trip continue-readback-1791267437 exit 0: consumed unchanged bytes/mtime
upgraded cache 0/0 to NULL/NULL without clearing. Input/output 1,471/2 and source revision
1791266584000 unchanged; continue-session-usage-2/health=ok. Old complete hash 190f4216e8675ec3
corrected, one upgrade/no real-conflict diagnostics. Repeat adds none; calls/complete total
unknown. Old package restored from saved earlier readback archive, SHA-256
69abe4b2463459d2969bf582d874d1e82796b14e77e5823f4ea5f38dc1adbd46.

## jcode 0.91.0

[Official release](https://github.com/1jehuang/jcode/releases/tag/v0.91.0) jcode-linux-x86_64.tar.gz
matches digest 74e9426b0a8e26c5fcff7803d6a5716f8294d1158fa63feb1b429450ac3f705e.
It contains a jcode-linux-x86_64 launcher and adjacent .bin; actual jcode v0.91.0 (439a243bb).
Initial preparation expected one jcode file, StopIteration exit 1. Retain failure, strictly
check both ordinary files/launcher and install their actual layout.

Fixed [OAUTH.md](https://github.com/1jehuang/jcode/blob/v0.91.0/OAUTH.md)/actual CLI verify
provider add local endpoint/--no-api-key/--context-window. Use --provider-profile local-model,
--tool-profile none, --no-update/--no-selfdev, telemetry disable first. Real client/model
share an offline/no-host-mount rootless ordinary-user container; unchanged SSE, no fabricated usage.

API/CLI/session snapshot input/output/cache-read 460/2/0, API total 462. Custom provider_key=
local-model does not imply openai. Zero cache read cannot imply zero cache write/reasoning.
Complete total/uncached/cache-write/cost/reasoning remain unknown. Creation-time env_snapshots
v0.91.0 (439a243bb) cannot verify every mixed-session message. Keep jcode-session-1 reference,
without broader product support. Retain native prompt_tokens, without adding to input_tokens.

Original snapshot SHA-256 9b80f14e29be44cb94cd2b5355bfabaaa5acc2c15b40e80e4a08a945581de8c7;
success jcode-real-1791268010. Redacted session/CLI/API/provenance:
core/tests/fixtures/jcode/real-0.91.0. jcode_contract 1/M8 19/M8 fixes 27 exit 0: full registry
isolated JCODE_HOME, one valid source/call, 460/2, unknown buckets, unchanged bytes/repeat.
Initial home_dir test discovered existing Kimi Work default candidates, source-count failure
exit 101. Explicit JCODE_HOME/no default home fixed test isolation without changing discovery
rules. Only successful single-turn snapshot accepted; journal/cloud/branch/rewind/multimodel/
subagent/failure/retry/cache-hit/other versions unaccepted. Executable readback recorded later.

## gajae-code 0.18.7

[Official release](https://github.com/Yeachan-Heo/gajae-code/releases/tag/v0.18.7) gjc-linux-x64,
161,092,808 bytes, matches SHA-256 c7d745845ac975a500a836c2e362e74791e6f430cffd760c0d186bc5e1cb8486.
Actual gjc/0.18.7. First download exceeded 240 seconds, exit 1, retaining 85,983,232 bytes.
Resume only after exact Range 206/Content-Range, fully validate before execution; partial
files were never counted as installations.

Fixed [models.md](https://github.com/Yeachan-Heo/gajae-code/blob/v0.18.7/docs/models.md)
verifies local-model baseUrl/auth:none/api:openai-completions/explicit real model. Independent
GJC_CODING_AGENT_DIR, --print/--mode json, tools/LSP/MCP/rules/title disabled, thinking off,
plain test system prompt. Rootless ordinary-user offline/no-host-mount execution calls the
same real model. Client requests SSE usage; proxy forwards unchanged.

API/CLI message_end/session v5 assistant usage match input 412/output 2/total 414. Ten native
lines include new configured_model_chain; fixed
[session-manager.ts](https://github.com/Yeachan-Heo/gajae-code/blob/v0.18.7/packages/coding-agent/src/session/session-manager.ts)
identifies persisted/replayed fallback configuration, without usage. Skip only this nonusage
type; other unknown types still stop the whole file and preserve cursors. Version names
cannot justify skipping unknown content.

Fixed [parseChunkUsage](https://github.com/Yeachan-Heo/gajae-code/blob/v0.18.7/packages/ai/src/providers/openai-completions.ts)
1991–2033 defaults absent cache/input/output to zero; input=prompt−cacheRead−cacheWrite,
total=max(reported, component sum). Cache/default initialization zeros cannot establish
API presence. gjc-session-2 leaves this API's zero buckets unknown and uncached unknown
without both cache buckets; inverse normalization recovers input 412, output 2/total 414,
independent source total and Estimated cost 0. Independent API cached_tokens=0 is not injected
into native references; other APIs remain separate. All-zero controls retain observed calls
with unknown tokens.

openai-completions.ts:609–614 creates output before connection;
[createInitialResponsesAssistantMessage](https://github.com/Yeachan-Heo/gajae-code/blob/v0.18.7/packages/ai/src/providers/openai-responses-shared.ts)
1166–1183 uses Date.now(). Native message.timestamp=1791268562127 precedes entry persistence
by 288 ms: source_start, unchanged numeric time, without claiming completion time.

Original JSONL SHA-256 be9f3e72bc71e3d58a81ef23fd7cf68e0c41fe30d0fb5a1c812ed1dfb002affb;
gajae-real-1791268555. Ten redacted lines/CLI/API/provenance in
core/tests/fixtures/gajae-code/real-0.18.7; product version differs from session format 5.
Five gajae_contract checks exit 0: full-registry isolation/repeats, complete canonical/legacy
old summaries/consumed cursors, parallel wrapper/checkpoint rollback, other token/quality/
model/cost/revision conflicts, same-batch real conflicts, all-zero correction and unknown-type
stop. Explicit corrections permit only stated fields, retaining observation/revision/conflicts/
diagnostics and committing unsealed summaries/events/checkpoints together. Old control data
removes the new nonusage line only before first scan, without usage edits, and is reported
separately from untouched native logs.

First exit 101 wrongly expected no conflict when old revision existed but new revision was
absent. Existing rules require comparable complete revisions and already retained the old
summary; fix test assertions without weakening product handling. Keep gajae-target.log and
gajae-target-final/gajae-zero-final.log separately. One successful local call only; cloud,
other APIs/versions/failure/retry/cache hits/multimodel/subagents/branches/rewind unaccepted.
Real old/new packages recorded separately.

<a id="五个客户端完成后的成品复验"></a>

## Package rechecks after five clients

Windows/Debian builds exit 0. Unified checks: Rust 938 (core 837/app 101, eight ignored),
frontend 21/scripts 4, Markdown 192, Svelte zero errors/warnings, fmt, Clippy -D warnings and
frontend build. Debian workspace actually passed 935 (core 837/app 98, nine ignored); earlier
six independent credential checks retain their original stage.

| Tested artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| Windows exe | 9,994,752 | a8bdaff6fb9a9d4bd5db31d1224d5eba6019fd050dbc4822069135503c7cde13 |
| Windows NSIS | 3,944,393 | a451f70d1b6b1e04bdbddca05d78cc83059c153feeaf11d94fb396ed7fe1a2b1 |
| Debian deb | 5,470,798 | e901252da13179ab3cbeca98906f5e93d72149ea97635ea2d1f28f42a256733e |
| Linux AppImage | 111,143,416 | 85d839e6db240c1bd7071f312dca30aa81f52ba69a519c899d68dce60731b8bf |

These actual artifacts read original sources without repeating inference or changing bytes.
Windows cwd repository root; Linux cwd isolated WSL copy. All commands exit 0:

| Acceptance | Results and output directory |
| --- | --- |
| test:install:windows | 12 checks, build/install-lifecycle/windows/1791269838899; real NSIS upgrade/rollback/uninstall/reinstall/writer-failure abort, zero owned integration remnants. |
| test:install:linux --screen-reader --appimage-mode fuse | Nine groups/47 checks, WSL build/install-lifecycle/linux/1791269789954; real deb round trip, UID 1000/CapEff=0/Seccomp=2 GTK/read-only FUSE/release on exit, Orca five-page navigation/speech/branding; owned containers reclaimed. |
| test:headless | 11 checks, three synthetic events/75 tokens; build/plan-completion/headless/1791269785308. |
| test:receiver | Eight checks, zero owned credentials; build/plan-continuation/native-receiver/1791269789134, real IPC/HTTP without real-exporter claim. |
| test:desktop | 17 checks, 20 small-data first paints P95 736.4 ms; build/plan-completion/native/1791270091144, real WebView2/IPC. |
| Aider original analytics/repeat | One call/95 input/3 output/98 total, health=ok, aider-readback-1791269832; other-format Zed diagnostics retained. |
| Goose original DB/default discovery/repeat | One call/320/2/322, health=ok/no diagnostics, goose-readback-1791269839; cache write/cost unknown. |
| Continue old DB/new package/repeat | 1,471/2, cache 0/0 → NULL/NULL, unchanged source revision, total/calls unknown, health=ok, continue-readback-1791270088; one upgrade/zero conflicts. |
| jcode original snapshot/default discovery/repeat | One call/460/2, cache-read=0/complete total unknown, health=ok/no diagnostics, jcode-readback-1791269789. |
| gajae original JSONL old DB/new package | Old reader stops at configured_model_chain, zero rows; new one call/412/2/414, unknown cache/uncached/source_start/health=ok, gajae-readback-1791269788; old format diagnostic retained. |
| gajae old-rule control DB/new package/repeat | Before first read only new nonusage entry removed. Complete old fnv1a64:40c2834cd0e94759 and consumed cursor reevaluated: cache 0/0/uncached 412 → unknown, completion → start. Numeric time/revision unchanged, one upgrade/zero conflicts; separately listed at the same result root. |

Continue old SHA-256 69abe4b2463459d2969bf582d874d1e82796b14e77e5823f4ea5f38dc1adbd46;
gajae old 6faa4f4a93fca930b197000e5db32b99feb38a5ad1c69485747c7cf52fc1bcf6.
Its modified control SHA-256 31f066bc087dfb6c7df12a2f18baba8a436aa759582d4962fe173599d03ce40d
is separate from original JSONL. Actual GUI, synthetic lifecycle and native-source readback
have separate results; package version cannot replace digests, and installed source clients
cannot verify other scenarios.

## AtomCode 5.2.1

[Official installation](https://atomcode.atomgit.com/docs/en/getting-started.html) specifies
@atomgit.com/atomcode npm. 5.2.1-linux-x64 SHA-512 matches registry dist.integrity:
ncC/rqORQeDLMPH39NpjchZEoFCBmC4uosaWaKe8spbRv3Ek51/HvRQuJMMaC6amPrkRdfjSRZYRxP8ktz2oTg==.
Archive contains package/bin/atomcode and package/package.json; binary atomcode 5.2.1 (unknown).
GitHub mirror releases/latest returned 404; inspect official installation script/npm metadata
without guessing URLs or running the host script. Mirror source 45e05cb14775f070f3867539e1f21c9849b3484b
also declares Cargo 5.2.1, but unknown binary build ID cannot establish that commit. Keep
atomcode-meta-turns-1 reference.

[Official configuration](https://atomcode.atomgit.com/docs/en/configuration.html)/actual CLI
use local OpenAI, headless --no-tools/--dev/--no-telemetry/--output-format jsonl, independent
ATOMCODE_HOME, nonaccount local authentication placeholder, offline/no host mounts/UID 1000.
Old documented --max-turns/--disable-tools were not used; actual help/fixed source establish
--no-tools/coding.max_rounds. Real local model/SSE responses unchanged.

atomcode-real-1791270565 API/CLI usage/turn.completed/.meta v1 match input 6,176/output 2/
total 6,178/round_count=1/tool_call_count=0. created_at=1791270569650/updated_at=1791270575895.
TurnStat lacks occurrence time: retain session interval/source call aggregate, without per-call
events or extra last-request total. Original .meta SHA-256
3e5db1220bad2e44993054bb4257ea70c6580cfdec99f3aedf8d674c58e2f0d7.
core/tests/fixtures/atomcode/real-5.2.1 retains redacted .meta, selected CLI/API/provenance,
removing names/owner/origin/import/fork/body/workspace paths without changing numbers/times.

Fixed openai_compat.rs:1785–1803/manager.rs:680–715 default missing API fields/TokenBreakdown
deserialization to zero. Cumulative cache zeros cannot establish API presence; cache-read/
uncached remain unknown. Valid normalized buckets recover positive input 6,176/output 2/
total 6,178. All-zero control retains reported round=1 with unknown tokens. Old reading also
accumulated absent/invalid buckets as zero; now keep unknown/bounded diagnostics on invalid
buckets/overflow while importing other valid models. atomcode-meta-turns-2 same-revision
upgrade requires complete old summary reconstructable only from default zeros/old derived
buckets; other tokens/quality/calls/interval/coverage/higher revisions retain normal conflicts.
Aggregates/checkpoints commit together.

Actual .ui.json v1 entries/.rewind.json version 2 points are auxiliary state under fixed
manager.rs:1069–1077/3253–3308. Exclude only complete verified shapes with valid same-name
.meta/identity, bounded to 64 KiB; other formats/versions remain diagnostic. Manual malformed/
usage JSON cannot be hidden by names; manual entry is a directory. Initial control wrongly
used rewind version 1; native values establish version 2, while version 99 remains diagnosed.
Six checks exit 0. First test used a manual file with directory-only discovery, exit 101;
correct test to directory without expanding product entry points.

Six AtomCode checks cover full registry/native rows/repeat, consumed cursors/complete old hash,
parallel wrappers/checkpoint rollback, protected fields/higher revisions, zeros/missing/mistyped/
overflow buckets and auxiliary/manual errors. Initial nonexistent parallel_scan target exit
101 corrected to parallel_scans, with separate logs. Unified checks first failed unformatted
diagnostics, exit 1; is_none_or then exceeded MSRV 1.77.2, exit 101. Use compatible syntax
without raising MSRV. Only successful single local turn; cloud/versions/resume/branches/
subagent/background/cache hits remain unverified.

<a id="六个客户端完成后的成品复验"></a>

## Package rechecks after six clients

Windows unified exit 0: Rust 944 (core 843/app 101, eight ignored), frontend 21/scripts 4,
Svelte zero errors/warnings. Six AtomCode checks verify final rewind version 2; full frozen
verification repeats with identical counts. Debian workspace 941 (core 843/app 98, nine
ignored), exit 0. Build Windows release/NSIS and Debian deb/AppImage from unchanged source;
the five-client artifact table retains historical acceptance only.

| Tested artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| Windows exe | 10,000,896 | fc68aa7b60c8a0a65435663566712d70d177949f38b0335207eef537198fa953 |
| Windows NSIS | 3,945,809 | 397b2425c76a397c22e88eaa010a056b435f7e142d1530dbddc950851d0d0c03 |
| Debian deb | 5,472,434 | 07970ce598fc6fbe10e08af769d3a544c902acb85e470d8ed653ba279cda8ec6 |
| Linux AppImage | 111,143,416 | 50f3b106c478b01b8e93679d8cc7189edcc93d834dc502ad70e1c98a563deb1f |

All commands exit 0, Windows repository-root cwd/Linux isolated WSL cwd. Read six original
sources without new inference; digests unchanged:

| Acceptance | Results and output directory |
| --- | --- |
| test:install:windows | 12 checks, build/install-lifecycle/windows/1791271903027; NSIS upgrade/rollback/uninstall/reinstall/failure abort, zero owned remnants. |
| test:install:linux --screen-reader --appimage-mode fuse | Nine groups/47 checks, WSL build/install-lifecycle/linux/1791272024234; deb round trip/GTK/IPC/read-only FUSE/release, Orca five-page names/keyboard/speech; UID 1000/CapEff=0/Seccomp=2, offline/no host mounts, containers reclaimed. |
| test:headless | 11 checks/three synthetic events/75 tokens; build/plan-completion/headless/1791272017297. |
| test:receiver | Eight checks/zero owned credentials; build/plan-continuation/native-receiver/1791272022051, real IPC/HTTP, synthetic exporter configuration. |
| test:desktop | 17 checks/20 first paints P95 766.9 ms; build/plan-completion/native/1791272022393. |
| Aider original/repeat | One call/95/3/98, health=ok, aider-readback-1791272167; other manual-format diagnostics retained. |
| Goose original/repeat | One call/320/2/322, health=ok/no diagnostics, goose-readback-1791272171; cache write/cost unknown. |
| Continue old DB/current/repeat | 1,471/2, cache 0/0 → unknown, unchanged revision/health=ok, continue-readback-1791272170; one upgrade/zero conflicts/unknown total/calls. |
| jcode original/repeat | One call/460/2, cache-read=0/total unknown, health=ok/no diagnostics; jcode-readback-1791272171. |
| gajae original/control old DB/current | Native configuration chain recognized; complete old control hash corrects cache/uncached/time basis, unchanged numeric time/revision, health=ok; gajae-readback-1791272171, one call/412/2/414/one upgrade/zero conflicts. |
| AtomCode complete original directory old DB/current/repeat | atomcode-readback-1791271970, one source-reported call/6,176/2/6,178, revision 1791270575895/interval unchanged; default cache/uncached → unknown, degraded → ok, one upgrade/zero conflicts/zero per-call events/repeat unchanged. |

AtomCode old package is prior e901252d…; original .meta/UI v1/rewind v2 unchanged. Complete
old fnv1a64:e3f4f4038bc2d9f0/consumed checkpoint reevaluated to fnv1a64:4a44cb7e34542f28.
Four old unknown_format diagnostics stay; no new false auxiliary-file diagnoses. First
atomcode-readback-1791271915 product results were correct, but a script expected unchanged
total diagnostics despite new scan_completed, exit 1. Enforce monotonic history/no added
unknown_format, then repeat with a fresh isolated DB. No database clearing concealed failures.

<a id="command-code-1743-准备与认证限制"></a>

## Command Code 1.74.3 preparation and authentication limits

[Official BYOK](https://commandcode.ai/docs/byok) supports providers.json keyless local
OpenAI/--local-only/CMD_LOCAL_ONLY. Official command-code@1.74.3 SHA-512 integrity verified:
K0eLCJtkWDvdR7JegxANKe5CUdsujO7+VtGeH/SLyjTWRnXLPoR5poBKiedgoRdn0NNtEQ/+/tE/3VqjAsmtQA==.
Actual 1.74.3, offline image f1ff5b83dcc302e1f1d6e7fa62c8a370d90ca12c81370274a3bf75a1b98dca7f.
Dependencies installed during preparation only; runtime offline/no host mounts/UID 1000/fresh
HOME. Real model started but received no API requests.

commandcode-real-1791272678 uses --local-only/--no-auto-update/--no-skills/--skip-onboarding,
--permission-mode plan/--model local-model/qwen2.5-0.5b-local/--max-turns 1/-p/--output-format
json. Exit 3, “Not authenticated”, requires cmd login; no projects sessions. Four zero output
buckets are failure initialization, rather than observed usage. resolvePrintAuthentication
requires isAuthenticated=true; isAuthenticated reads a Command account key without a local-only
exception. No injected test flags, fabricated credentials, modified client or personal login.
Installation/real-launch restriction only, excluded from six real-usage samples.

## Crush 0.97.1

[Official v0.97.1](https://github.com/charmbracelet/crush/releases/tag/v0.97.1) Linux x86_64
tar, 26,726,787 bytes, matches release SHA-256
1b7cbe0600a3797538a74dc00dd8c4bac54ac4b8f4455ba5ff2312b29c7bd598.
Actual crush version v0.97.1. Tagged source verifies new crushrc configuration/all 29 tool
names; deny each explicitly without guessed wildcards. Provider autoupdate/metrics disabled,
ordinary UID 1000/offline/no-host-mount rootless execution with real model.

crush-real-1791273563 main loop/automatic title produce two API calls, answer OK. API input
5,059/output 4/total 5,063/cache read 4; native root-session/CLI statistics instead contain
context snapshot 4,899/2/4,901, whose SUM cannot represent cumulative usage.
[agent.go:2036–2086](https://github.com/charmbracelet/crush/blob/v0.97.1/internal/agent/agent.go)
shows additive costs but token SET overwrite; child cost rolls into parent. Keep tokens/calls/
model unknown, root-session Estimated cost only. crush-sessions-cost-1 does not verify
product versions.

Explicit manual acceptance rates: USD 1/million for input/output, zero cache read/write.
These are neither local-model prices nor bills. Independent API prompt−cached+output gives
5,059 micro-USD; native/CLI HTML total_cost=0.005059. Original DB SHA-256
443b94f91fa1995f70af99123807c660d3b4088113d5f8d3f4519c4dd4ccc911, integrity passed.
core/tests/fixtures/crush/real-0.97.1 retains real schemas/numbers/time/redacted identity/
selected CLI/API fields, without bodies/project paths. Original parts are two text/two finish,
no tools. One crush_contract exit 0: full discovery, one cost-only observation, unknown tokens,
read-only/repeat. Current executable readback recorded below.

Initial inference succeeded, but stats --json was invalid, overall exit 1. Source/help says
HTML; reread saved DB without repeating inference. First parsing incorrectly treated output
path notification as JSON; another copy lacked destination, exit 125. Separate failure logs
retained; embedded HTML statistics finally read with exit 0. Statistics-page browser, cloud,
compaction/children/other versions unaccepted.

<a id="junie-26922341929"></a>

## Junie 26.9.22 (3419.29)

[Official custom models](https://junie.jetbrains.com/docs/custom-llm-models.html)/actual help
verify complete Chat Completions URL, no key, independent JUNIE_HOME. Website installation
script download returned 403; read official GitHub commit
472d75becf96aa4a37733797f37309ca2a42f980 without running host installer. Official 3419.29
Linux amd64 ZIP, 337,552,368 bytes, matches SHA-256
7ac5d675d90305c65207f9ddaf4219a1bf78c34630b8e39423833d2716ce7b5e.
Stop only precisely matched owned slow-download child processes, retaining 59,768,832 bytes;
validate each remaining 206/Content-Range, size and whole digest before extraction. Partial
packages never counted as installed. Actual Junie 26.9.22 (3419.29), offline image
ae1b5f9b69f8e03bf7c634776f29630081c5da78f61de65bc38e1302f801414f.

First explicit chat mode exit 1: stable distribution requires Nightly, no API calls. Stable
classic mode junie-real-1791273837 makes seven real local calls; 43 events.jsonl lines/seven
LlmResponseMetadataEvent records, final exit 1 from small-model response format mismatch.
API prompt 64,074/cache read 53,102/output 98; CLI/native inputTokens=10,972 excluding cache.
Task failure retains incurred usage without verifying successful tasks.

Distribution LlmMetadata.onLlmEvent constructs ModelUsage from AIAnswer.usage. UsageTokens
construction/deserialization defaults absent numbers to zero. OpenAIChatRequest input is
prompt_tokens−cached_tokens/cache write default zero; Responses/Anthropic/Google converters
also normalize independent buckets. Events omit API/provider/product version: models cannot
establish complete input/totals. Positive input maps input_uncached with independent positive
cache read/output. Missing/zero components unknown; AIAnswer.time default zero does not
establish duration. calcTokenCost uses ModelCapabilities per-million rates for four buckets/
Web Search. Custom model has no rate, cost=0 unknown. Positive costs Estimated; USD follows
earlier documented reference, without verifying paid channel/bill.

Native events SHA-256 529330c708ec839b3c840e54503ba6cc7bf158c0f6150e762eed45d9df061580.
fixtures/junie/real-3419.29 preserves all 43 positions/types/seven usage events/timestamps,
removing other payload/environment/paths/body. API/CLI final result.errorCode arrays retain
native usage fields. Uncached 10,972/cache read 53,102/output 98 match independent API;
API full input/total 64,074/64,172 are reconciliation only, not missing-field substitutes.

junie-events-doc1 → 2 preserves event keys composed from original buckets/cost and revisions,
rereading old cursors. At most 128 absent/default-zero candidates reconstruct only permitted
old differences and match complete canonical/legacy summaries. Input location/cost quality/
zero-duration correction cannot conceal changed models/positive usage/other quality/revision.
Observation/real-conflict/history retained; events/summaries/cursors commit together. Add
verified JUNIE_HOME discovery. Five junie_contract checks pass: native/API/CLI/full registry/
read-only/repeats, old cursors/two summaries/parallel/checkpoint rollback, other content/
same-batch conflicts, zero/missing fields, invalid entries/positive Estimated cost. First
test lacked manifest; later wrong private helper/ScanTarget/column caused compile failures.
Correct actual definitions, exit 0, retain logs. Full verification/current-deb original-source
readback and old-cursor migration passed, as below.

First bundled javap attempt exit 127; jdk.jdeps absent, exit 1. Under existing user authorization,
install official openjdk-21-jdk-headless 21.0.12.1+1-1~deb13u1 in WSL Debian; javap reads pass
without modifying official JARs. Other versions/APIs, streaming/paid channels, complete hidden
calls and IDE remain unaccepted.

<a id="八源阶段成品复验"></a>

## Eight-source package rechecks

Frozen business source: root npm run verify exit 0, Rust 950 (core 849/app 101, eight ignored),
frontend 21/scripts 4/Markdown 192, Svelte zero errors/warnings, fmt, Clippy -D warnings/build.
Debian workspace 947 (core 849/app 98, nine ignored), exit 0. Eight-product checks total 23;
previous stages retain their original scope.

| Tested artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| Windows LLMUsage.exe | 10,002,944 | 07521361b500982a9cadb485fb64acbf3eb2f03a14a01bf07731ffe8f617c603 |
| Windows NSIS 0.2.1 | 3,947,611 | 969b39b97f7511a34d55464dde9a1b935d8f721b333090b019606a0d53d065d5 |
| Debian 0.2.1 | 5,472,992 | c50e4bb168eee805b295fd6141184e8a6a14bd601766b1f337cd0cedf1523e78 |
| AppImage 0.2.1 | 111,147,512 | 806cfe2dace748ad83de4d1742058039b4874386e7fa276ed31ae30d1082bd73 |

Windows actual NSIS round trip: build/install-lifecycle/windows/1791275512662, 12 checks,
exit 0/zero owned integrations. Headless build/plan-completion/headless/1791275588209:
11 checks/three synthetic events/75 tokens. Receiver build/plan-continuation/native-receiver/
1791275585960: eight checks/zero owned credentials. Native WebView2 build/plan-completion/
native/1791275584776: 17 checks/20 first paints P95 776.3 ms, exit 0.

Debian build/install-lifecycle/linux/1791275588714: nine groups/47, exit 0, actual old/new
deb install/upgrade/rollback/uninstall/reinstall/purge, read-only AppImage FUSE/GUI/release,
Orca five-page native Tab/Enter/AT-SPI names/speech. Same pinned Orca image, ordinary UID 1000,
CapEff=0/default seccomp/network none/no host mounts. Only owned rootless FUSE containers
add SYS_ADMIN. Host logout/other DPI/distributions/assistive tools/physical audio unverified.

This stage's c50e4bb... deb rereads saved native sources/repeats without changed bytes.
Roots relative to WSL build/plan-final-push; OpenCode relative to build/install-lifecycle.

| Source | Executable result | Output root |
| --- | --- | --- |
| Aider | One call, 95/3/98, unknown cache, target health=ok. | aider-readback-1791275681 |
| Goose | One call, 320/2/322, cache read 0, other fields unknown. | goose-readback-1791275676 |
| Continue | 1,471/2, old cache 0 → NULL, same revision/unknown total/calls. | continue-readback-1791275679 |
| jcode | One call, 460/2, cache read 0/total unknown. | jcode-readback-1791275678 |
| gajae-code | One call, 412/2/414; native configuration chain restored, complete control-summary correction/history retained. | gajae-readback-1791275679 |
| AtomCode | 6,176/2/6,178, reported calls 1; cache/uncached → NULL, same revision/four old unknown_format diagnoses retained. | atomcode-readback-1791275679 |
| Crush | One cost-only observation, Estimated 5,059 micro-USD; tokens/model/calls unknown. | crush-readback-1791275588 |
| Junie | Seven original records, uncached 10,972/cache read 53,102/output 98; input/total/default-zero cost/duration unknown. | junie-readback-1791275627 |
| OpenCode | Default/controlled one call each, 298/1/299, cache read 3/0, known_version/coverage unchanged. | fuse-continuation-opencode-default-1791275761 / controlled-1791275765 |

Qwen native-first/SDK-copy selection recheck exit 0: two effective calls/input 15,865/output
558/total 16,423/cache read 3. One native/two SDK copies excluded, target Qwen/OTel healthy;
other manual-format diagnostics retained. First Junie old-package read set JUNIE_HOME, an
unsupported old discovery entry, yielding zero sources/exit 1. Retry shared isolated HOME/
.junie layout without editing events. Archived old deb 07970ce... had seven input_total
values summing 10,972 and default-zero cache/cost/time. Upgrade preserves keys/first observations,
seven parser_policy_updated/no conflicts. Old cache-inclusion diagnostic counts remain at five;
repeat adds none. First/final logs separate; all owned acceptance containers stopped/removed,
cached images retained.

## Xum 0.30.0

[Official npm metadata](https://registry.npmjs.org/@coder/xum/0.30.0) gitHead
81b0b744db6e27a4416f3596d70bf88529171caf, 32,673,885-byte tarball, verified SHA-512
`dPdpIxj8o0gZw+Kt5xWe93QMMIZhKC3DKDzqW7JDLEUs/n67lGuNnkexNFQwjz9xejsBhzuGyt/ZexwyhleSRQ==`.
Actual v0.30.0-dirty (81b0b744d), retained without attributing dirty to this work. Dependencies
installed ignore-scripts only inside owned Podman; image
c97123cc6297913ea9aacc8269b49a54ae5c0c3a67e2b41425172a31d1a534ac.

[CLI](https://xum.coder.com/reference/cli), [providers](https://xum.coder.com/config/providers)
and fixed source support keyless custom openai-compatible provider. Real inference: network
none/UID 1000/no host mounts, XUM_DISABLE_TELEMETRY=1, --no-mcp-config, thinking off,
max_output_tokens=16, without account/fake key. runSessionRoot.ts supports XUM_RUN_SESSION_ROOT/
MUX_RUN_SESSION_ROOT to retain native sessions; default temporary configuration is deleted
on exit. paths.ts verifies XUM_ROOT/MUX_ROOT/new .xum/old .mux. Both native chats have two
text parts/no CLI tool events; real responses unchanged.

First xum-real-1791276725 uses default gateway/Plan mode: one API call/HTTP 200, but client
omits stream_options and model returns no usage. Official SDK includeUsage defaults false;
providerModelFactory.ts:1574–1580 omits it and SDK overwrites the request parameter. byModel/
lastRequest five buckets zero, chat.metadata.usage={}. Plan mode exits 1 without a plan;
neither successful task nor reported zero verified. Native 788 bytes, SHA-256
bbbbe31cfa7d2216f441a9dc74d7bf31a0b03f0974d68a30ade4fcd4eaf69af6.
Initial inspection used unavailable container rg, log retained; existing Python reads original
distribution, without installing/modifying SDK.

Independent xum-real-1791276921 Exec control leaves client/provider unchanged. Only the local
gateway adds stream_options.include_usage=true to real upstream llama.cpp, forwarding every
actual SSE byte without creating/overwriting usage. API one call/input 13,721/output 2/total
13,723/cached_tokens=0 matches CLI run-complete/chat.metadata.usage, task exit 0. Native
byModel.input=13,721/output=2, other buckets zero, no cost_usd. CLI cost_usd=0 does not verify
cost. Gateway control is not a default-client capability. Native 796 bytes, SHA-256
b5adf0dac5bf304a2852c0f13b5817b2502825e534e7f91ff8538fc234a094f1.

Fixed displayUsage.ts:117–147 subtracts cache from input/reasoning from output, defaults missing
fields to zero and enforces historical lower bounds. sessionUsageService.ts:173–195 accumulates
by model; lastRequest is final call only. xum-session-usage-2 maps positive input_uncached,
all zero buckets unknown; positive text + known reasoning derived, or text lower bound with
coverage notice/healthy source if reasoning unknown. Complete input/total/calls/cost unknown.
At most 32 complete-old-summary candidates recover permitted old buckets/default zeros;
protect revision/interval/identity/quality/coverage/other positive values. Default missing-usage
five-zero history becomes one entirely unknown native cumulative range, without call/complete
usage claims.

core/tests/fixtures/xum/real-0.30.0 native usage contains only model/numbers/time and needs
no edits; API retains selected fields. Six checks pass: two real scenarios/full registry/repeat,
input/known reasoning, invalid/overflow buckets, unchanged old cursors/parallel/rollback,
other fields/newer revision protection, four official environment entries/unknown-schema
latest_fallback. First two type/structure compile failures and wrong unknown-version expectation
are retained; correct against actual definitions/existing compatibility policy.

xum-readback-1791277675 old accepted c50e4bb.../current 4dde217... deb: both originals/auxiliary
SHA-256 unchanged, old cursors/keys/revisions/created_at retained. Five zeros → all unknown;
positive input_total 13,721 → input_uncached, output lower bound 2 retained. Two
aggregate_parser_policy_upgrade, zero new conflicts, one healthy-source output-coverage notice;
repeat adds nothing, health=ok. Other versions/cloud/reasoning models/cache hits/subagents/
rebuild/cross-day cumulative decreases unaccepted; schema v1 does not verify every version.

<a id="九源阶段成品复验"></a>

## Nine-source package rechecks

Windows unified Rust 956 (core 855/app 101), frontend 21/scripts 4; Debian Rust 953 (core
855/app 98, nine ignored), exit 0. Nine-source checks total 29. Same frozen source; previous
eight-source digests/first failures retained.

| Tested artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| Windows release exe | 10,006,528 | d185adde19096b6207ed14233bba9f7211a9474d83ebd19a6b85f0a03e46a73b |
| Windows NSIS | 3,949,453 | ac25b4a1bbfd36eba38be965941fce953bc91fdab0a2395b862acfba9da4c8a4 |
| Debian | 5,475,376 | 4dde217007fd7dc24b99ea6074babd2ae6f07f4a0bc72c2e50c883dca79fa4a9 |
| AppImage | 111,147,512 | 7e67071f7e357c714198876c57b69b6c41205b18505eef572bcef0c9c164f06e |

Windows NSIS 12 (1791277785564), headless 11 (1791277904563), real receiver eight
(1791277901389) exit 0, zero owned credentials. Linux nine groups/47 (1791277727107),
exit 0, real deb round trips/AppImage FUSE/GTK/WebKit/Orca five-page navigation/speech.
Windows WebView2 build/plan-completion/native/1791277903313: 17 checks/20 first paints
P95 729.7 ms, exit 0. Current deb nine-source repeats exit 0, preserving earlier values/
upgrade protection:

| Source | Results/root under WSL build/plan-final-push |
| --- | --- |
| Aider | One call/95/3/98, aider-readback-1791277771; unrelated manual Zed format diagnostics retained. |
| Goose | One call/320/2/322, goose-readback-1791277765; cache write/cost unknown. |
| Continue CLI | 1,471/2, default cache zero → unknown, unchanged revision, continue-readback-1791277766. |
| jcode | One call/460/2, total unknown, jcode-readback-1791277768. |
| gajae-code | One call/412/2/414, original/control complete old-summary protection, gajae-readback-1791277768. |
| AtomCode | 6,176/2/6,178, cache/uncached unknown, source calls 1/old diagnostics retained, atomcode-readback-1791277767. |
| Crush | Cost-only Estimated 5,059 micro-USD, unchanged original DB/registry, crush-readback-1791277772. |
| Junie | Seven records uncached 10,972/cache read 53,102/output 98, old keys/diagnostics/observations retained, junie-readback-1791277767. |
| Xum | Default unknown/control uncached 13,721/text lower bound 2, two old-summary upgrades, xum-readback-1791277675. |

39 sample files/118 JSON objects passed, zero secret keys/personal paths. Task artifacts remain
only under ignored build. Final document/Skill formatting/links reviewed separately; stage
builds do not verify publication/remote CI.

<a id="剩余本地采样路线核查"></a>

## Remaining local-sampling routes

Droid [public BYOK](https://docs.factory.com/model-independence/byok) supports local models;
[account-free airgap](https://docs.factory.com/enterprise/airgapped-deployment) uses an
undistributed enterprise package. Public headless installation still requires Factory API
key. Only documentation/distribution limits verified; no real call. Public BYOK cannot imply
account-free operation. Roo announced extension shutdown on 2026-05-15; its historical official
VSIX/native calls were independently checked below, without substituting archived source/Zoo.

Other routes checked against official documentation on 2026-10-06. Installation/BYOK support
alone does not verify native usage:

| Product | Official reference and limits at that date |
| --- | --- |
| Amp | [Model Routing](https://ampcode.com/docs/customize/model-routing) supports Custom URL/multiple protocols with account-maintained personal/workspace connections. Old “no BYOK” assumptions invalid; public account-free local route unverified. |
| Qoder | [Authentication](https://docs.qoder.com/cli/authentication) requires login/PAT. [Custom Models](https://docs.qoder.com/cli/custom-models) Individual BYOK uses account-visible catalog/wizard, explicitly forbidding manual settings.json. Native usage unverified. |
| Antigravity | [Actual CLI installation/authentication](https://antigravity.google/docs/cli/install/) requires Google login/Gemini API key; custom endpoints Gemini-compatible. [SDK local models](https://antigravity.google/docs/sdk/local-models/) has separate account-free OpenAI route, without verifying IDE protobuf; check interfaces separately. |
| Zed | [Providers](https://zed.dev/docs/ai/llm-providers) local models differed from the then-checked hosted zed.dev format. Empty DB did not verify usage; local BYOK could not verify hosted behavior. Later 2026-10-07 1.22.0/DbThread 0.3.0 specified external-provider native samples are independently accepted in [the next record](plan-20261007.md). |
| Kiro | [Authentication](https://kiro.dev/docs/getting-started/authentication/) public/enterprise routes require credentials; account-free local route unverified, without constructed account state. |
| Grok Build | x.ai/grok-build retrieval failed; only that failure recorded. Official distribution/local rules unverified; similarly named third-party CLIs cannot substitute. |

Only Command Code has an actual account-free headless rejection result here; distinguish
other documentation limits from runtime results.

## Roo 3.54.0

[Official release](https://github.com/RooCodeInc/Roo-Code/releases/tag/v3.54.0), commit
27001b2b5aa47b65e8a6ba1914e0f4216be0ebb0, published 2026-05-15T17:52:24Z.
roo-cline-3.54.0.vsix 30,837,353 bytes matches official SHA-256
615b7e30ab456c51fe2e1f413a6e05fe4bd370f2350e14de574ec92e25366b4b.
Manifest RooVeterinaryInc.roo-cline/3.54.0/./dist/extension.js/VS Code ^1.84.0.
[Shutdown announcement](https://roocodeinc.github.io/Roo-Code/) does not establish historical
BYOK cannot run.

Official Microsoft Linux VS Code 1.140.0/07f806f999227108933c2e30515b26eecc1fda74 tar,
348,920,118 bytes, SHA-256 d32031e9e213d59532af3cf32fcb8b357a1cdd10417967b4f5b5ba30436dc0dc,
matches official metadata. Validate both archives' entries/extracted size/path/link boundaries.
Independent image 6ed04799198831354cad3bde113717d908f3f93c3420b0ff89f1bba9f6377ddd adds
only ldd-missing libnspr4/libnss3. Runtime offline/no host mounts/ordinary user, independent
Xvfb/D-Bus/native Electron GUI. --no-sandbox/--disable-gpu are acceptance-environment flags.

Fixed src/extension/api.ts/official vscode-e2e activates unmodified extension, waits isReady,
uses public setConfiguration/startNewTask/cancelCurrentTask and taskTokenUsageUpdated.
provider=openai/loopback URL/qwen2.5-0.5b-local/max 16 output tokens. Disable MCP/tool autoapproval,
checkpoints/autocompaction, VS Code telemetry/updates; isolated user data/extensions. No key
configured; official SDK handler supplies not-provided placeholder, without account credentials.
Extension requests include_usage=true; proxy changes neither request nor SSE, saving selected usage.

| Scenario | Real model API | Native records/public extension statistics |
| --- | --- | --- |
| Default request limit, roo-real-1791279141 | Three calls: 6,772/2/6,774, 7,095/2/7,097, 7,418/2/7,420; total 21,285/6/21,291, nested cache read 13,869. | Two persisted api_req_started, 13,867/4/13,871. Public cancellation removed the third; coverage gap retained. |
| Request limit 2, roo-real-1791279262 | One call, 6,772/2/6,774, cache read 0. | Matching one call; next placeholder blocked by public limit and removed after cancellation. |

Model answered OK without completion tool; both public-API cancellations do not verify
successful task completion. Default native 767 bytes/SHA-256
b9c8ca855050f224281ec50038efa5f5aa4674e0726cce9a98b8c09204862238;
controlled 556 bytes/SHA-256 f5fa87fcc6bd6bccdcc529aec41a737d35f5ae81252aff08d81c124eedb5e2cc.
Redacted original records/API/extension stats in fixtures/roo/real-3.54.0 preserve actual times,
buckets/order/types, removing body/request text/task IDs/personal paths under readiness rules.

Fixed Task.ts/cost.ts initialize cache/cost zeros. OpenAiHandler.processUsageMetrics reads
only top-level cache_read_input_tokens/cache_creation_input_tokens, ignoring actual positive
prompt_tokens_details.cached_tokens. Native zero establishes neither zero cache nor free
usage. roo-ui-messages-doc2 retains positive input/output, unknown zero cache/cost, no invented
uncached input. Known total input/output derive complete total. Empty placeholders do not
count; explicit zero usage markers retain calls. Absent model/provider unknown; positive
condense cost Estimated.

Old upgrades require complete roo-ui-messages-doc1 canonical/legacy summaries, at most 32
old-zero/cost candidates with old-derived uncached/total reconstructed under old algorithm.
Other token/quality/model/ownership/time/revision/identity changes use normal conflict handling.
Retain event keys/first observations/diagnostics/real conflicts; events/cursors/fingerprints/
unsealed summaries commit together. APPDATA default discovery is independent of HOME;
acceptance excludes other host sources.

Five roo_contract/M8 19 checks exit 0: real full-registry/API/repeats, complete/legacy summaries/
unchanged consumed cursors/parallel/rollback, protected fields/same-batch conflicts, zero
markers/placeholders, invalid rows/other valid records/positive estimates. Real old/current
deb round trip roo-readback-1791280179 exit 0: two sources/three native calls, input 20,639/
output 6/total 20,645; do not invent the fourth service call. Three upgrades/zero conflicts/
two checkpoints/health=ok; bytes/keys/first observations/revisions unchanged. Retained old
diagnostic survives; repeat adds none.

First failures retained: download limit below official VS Code size raised after metadata
verification; missing NSS/official WSL startup notice resolved. Premature launcher exit had
no API/native files; use official-test native Electron with strict result checks. Limit 1
counts issued placeholders and yielded no request, excluded as sample. Initial sample tests
discovered other default sources/misread quality partitions: correct isolation/assertions
without changing product quality. First readback script syntax/source-format filter/expired
historical-marker assertions logged separately; final checks use agent/actual retention.
Official ripgrep-location warning/offline model-directory download failure retained. Tools,
other versions/cloud unaccepted.

<a id="十源阶段成品复验"></a>

## Ten-source package rechecks

This stage follows Roo changes; five/six/eight/nine-source digests/logs remain historical.
npm run verify exit 0: Rust 961 (core 860/app 101), frontend 21/scripts 4/Markdown 192,
Svelte zero errors/warnings, fmt/Clippy -D warnings/build. Debian workspace Rust 958
(core 860/app 98, nine ignored), 34 real-source checks.

| Tested artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| Windows release | 10,008,064 | d56c2ba2580dcd870ca05f5e190285bbd090c76a7588276bef1c7c87d8753e49 |
| Windows NSIS | 3,949,669 | 16de6498284e3830a29b99552f9a8b80fec7589a603dd0aa4f0eb099d5e3ecf1 |
| Debian deb | 5,476,298 | a7a607943723547e10c8e2e05cf42b176f38e55e76ab35e9ae0fc39d2d5e76e6 |
| AppImage | 111,151,608 | b9decd976bb69068e3d2cff539b25252443895501bff7f592f5e52382a90eb11 |

NSIS 12/root 1791280147976, headless 11/root 1791280229639, receiver eight/root
1791280228175/zero owned credentials, Windows native 17/root 1791280276230 all exit 0;
20 first paints P95 753.6 ms. Linux nine groups/47/root 1791280082989 exit 0: real deb
round trip/read-only AppImage FUSE/release/GTK/IPC/Orca five pages/branding speech. Ordinary
user/CapEff=0/default seccomp/offline/no host mounts; only owned FUSE containers add SYS_ADMIN.
Physical audio is not verified.

| Current deb original-source readback/repeat | Actual root |
| --- | --- |
| Aider one call/95 input/3 output/98 total | aider-readback-1791280325 |
| Goose one call/320/2/322, cache read 0/write-cost unknown | goose-readback-1791280328 |
| Continue cumulative 1,471/2, old cache zeros unknown/revision retained | continue-readback-1791280273 |
| jcode one call/460/2, complete total unknown | jcode-readback-1791280329 |
| gajae one call/412/2/414, complete old-summary correction | gajae-readback-1791280275 |
| AtomCode 6,176/2/6,178/reported calls 1, uncertain interval, old source health restored | atomcode-readback-1791280273 |
| Crush Estimated 5,059 micro-USD only, manual rates do not verify bills | crush-readback-1791280327 |
| Junie seven records/uncached 10,972/cache read 53,102/output 98, keys/diagnostics/observations retained | junie-readback-1791280229 |
| Xum default unknown/control uncached 13,721/text lower bound 2, two old-summary upgrades | xum-readback-1791280229 |
| Roo three native calls/20,645 total, cache/cost unknown/three old-summary upgrades | roo-readback-1791280179 |

46 sample files/125 JSON objects pass, zero secret keys/personal paths. Original bytes unchanged;
rule upgrades preserve diagnostic/conflict history. These are local unpublished artifacts,
rather than remote CI or release acceptance.
