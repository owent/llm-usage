# Cline VS Code SDK container-source acceptance

<a id="cline-vs-code-sdk-容器来源验收"></a>

2026-10-06; [plan](../../../Plan.md), [data rules](../../design/desktop-usage/data-contract.md)
and [current artifact acceptance](current-acceptance.md). This checks the actual run and native
fields, without verifying legacy ui_messages.json, CLI, desktop sidecars or other providers.
The later [OpenClaw stage](openclaw-container-sample.md) rechecked native Cline SDK old-database
upgrades/repeated reads with new packages; the hashes/counts below retain this stage's scope.

<a id="官方分发与源码"></a>

## Official distribution and source

[Official v4.1.22](https://github.com/cline/cline/releases/tag/v4.1.22) was released on 2026-09-30 at
f58bc118bdeef1bd2813cd08e00d98bdcda96475. The original VSIX is 9,072,144 bytes;
its downloaded SHA-256 matches the release manifest:
134b54af94e1e4cc6cd07224a61f6873c40c845d9fba1f9e6aa510dd6e2c5382.
Package manifest: saoudrizwan.claude-dev, 4.1.22, dist/extension.js, VS Code ^1.101.0.
The latest desktop release is a separate client and does not verify this extension version.

Fixed-source checks:

- [Native writer](https://github.com/cline/cline/blob/f58bc118bdeef1bd2813cd08e00d98bdcda96475/sdk/packages/core/src/services/session-data.ts):
  schema 1; only the last assistant carries metrics. Missing
  per-turn metrics may be filled with whole-run usage, and ts may use run endedAt. Metrics
  counts do not identify underlying API-request counts.
- [Codec](https://github.com/cline/cline/blob/f58bc118bdeef1bd2813cd08e00d98bdcda96475/sdk/packages/core/src/runtime/config/agent-message-codec.ts):
  all four token fields may contain default zero. Retain positive values;
  zero does not establish a reported absence of cache usage.
- [Origin
  metadata](https://github.com/cline/cline/blob/f58bc118bdeef1bd2813cd08e00d98bdcda96475/sdk/packages/core/src/session/history-origin.ts):
  session version is writable metadata and cannot identify
  each message's version in mixed history.
- [Paths](https://github.com/cline/cline/blob/f58bc118bdeef1bd2813cd08e00d98bdcda96475/sdk/packages/shared/src/storage/paths.ts): session
  directories resolve through CLINE_SESSION_DATA_DIR,
  CLINE_DATA_DIR and CLINE_DIR/default home.
- [UI translator](https://github.com/cline/cline/blob/f58bc118bdeef1bd2813cd08e00d98bdcda96475/apps/vscode/src/sdk/message-translator.ts): SDK
  input includes cache; cache is subtracted only when
  translating to legacy UI. The legacy UI's four mutually exclusive token buckets do not apply.
- [Legacy-session
  conversion](https://github.com/cline/cline/blob/f58bc118bdeef1bd2813cd08e00d98bdcda96475/apps/vscode/src/sdk/legacy-task-handling.ts): copies
  role/content without legacy UI usage;
  actual recovery of old sessions needs separate acceptance.
- [Retry usage
  merging](https://github.com/cline/cline/blob/f58bc118bdeef1bd2813cd08e00d98bdcda96475/sdk/packages/llms/src/providers/middleware/retry-empty-response.ts):
  discarded attempts' usage may be included in the final
  finish. One metrics record does not imply one API request.

<a id="实际环境与运行"></a>

## Environment and actual run

WSL Debian/rootless Podman used the verified VS Code 1.140.0 GUI dependency image. Only the
official Cline VSIX was copied to independent /opt/cline; extension directories, HOME and
configuration were isolated. Runtime had no host mounts/network/privileged mode/additional
capabilities. GUI/model ran as the acceptance user with Xvfb and independent D-Bus. No account
login or personal configuration was used. The public extension API exposes task operations
such as startNewTask; tests called activate/startNewTask without modifying extension code,
using a private controller or manufacturing native usage.

Following the [official local-model instructions](https://docs.cline.bot/running-models-locally/overview), only public user configuration
under isolated CLINE_DIR was written: lmstudio provider, loopback /v1. The actual service was
llama.cpp with official Qwen3.5-0.8B BF16; LM Studio itself was not claimed installed.
Model/service references follow [the Hermes record](hermes-container-sample.md): GGUF SHA-256
9a7bed4041b7975e0f71fa34670d1e9025213bc92905ac0db75d36c4fa3fa623; native context 262,144.
This server used context 64,000, one slot and 16 output tokens per call. The proxy forwarded
responses unchanged and recorded usage; the client itself requested include_usage.

All automatic approvals were disabled, using plan mode with telemetry/checkpoints/auto-condense/
MCP disabled. Upstream removed legacy maxRequests, so its configured value 1 was not an effective
request limit. A 210-second external deadline and actual model output limits bounded the test.
After three metrics records, the test host exited normally with code 0; this does not verify
task completion, a stop API or Cline's formal cancellation lifecycle. The owned container was
stopped/removed, logs/source files retained and the host IDE left unchanged.

<a id="独立核对与读取结果"></a>

## Independent comparison and reading results

build/plan-final-push/cline-real-1791283911 holds extension-host/model logs, transparent API
ledger, native sources and container inspect. cline-inspect/cline-native-source/cline-fixture
check environment, source provenance and extraction summaries respectively.

| Record | API/native total input | API/native output | API/native cache read | Unified total |
| --- | --- | --- | --- | --- |
| 1 | 2,915 | 16 | 0; native default zero is unknown | 2,931 |
| 2 | 2,974 | 16 | 2,911 | 2,990 |
| 3 | 3,033 | 16 | 2,970 | 3,049 |
| Total | 8,922 | 48 | Known subtotal 5,881 | 8,970 |

Native messages: 9,732 bytes, SHA-256
c1aaa3d3170f1c33a57ea99e98de840c380313689d4be03c916cf86524e5e53f.
Manifest cumulative uncached input 3,041/output 48/cache read 5,881 uses different input rules
from native SDK messages and is not added as another source. sessions.db is registry metadata;
its tokens are not read. Every source modelInfo is qwen3.5-0.8b-local/lmstudio; cache write is
default zero and cost fields are absent.

Added independent cline-sdk-messages-v1 reading. SDK origin version remains latest_fallback;
three usage_observation records leave call counts unknown. Cache write/uncached input/reasoning/
cost are unknown. Empty sessions, non-VS-Code interfaces, import/subagent and unknown schemas
remain isolated. The legacy UI parser is retained independently. Full-registry discovery reads
one physical file once, repeated scans add no usage, and source bytes remain unchanged. Redacted
test data retains allowlisted fields/replaced IDs; message bodies/system prompts/configuration/
credentials/databases were not committed.

<a id="验证与首次失败"></a>

## Checks and first failures

Ten Windows Cline SDK requirement tests passed: native/API/full-registry comparison, default/
environment path priority and five manual-root forms, default-zero/invalid-field isolation,
unknown interfaces/versions, displayOnly/no-usage exclusion, duplicate IDs/real conflicts/
retained disappearance history, cache contradictions, partial-write recovery, checkpoint
rollback/parallel retries and 32 MiB rejection without cursor advancement. Three legacy Cline
tests remain passing. Affected rules/matrix/research/plan were synchronized with artifact acceptance.

Original logs retain the first cargo-helper call missing a manifest (exit 101), initial test
compilation errors in parallel-function arguments/borrowing, and a seven-pass/one-failure
diagnostic-code assertion. Actual framework/ingest code required update_conflict plus the
conflict flag; correcting that assertion did not weaken behavior checks. Final targeted exit: 0.

<a id="当前成品验收"></a>

## Artifact acceptance at this stage

All commands exited 0; logs remain under build/plan-final-push:

- Windows npm run verify: 977 Rust tests, core 876/app 101, eight ignored by default; 21 frontend/
  four script tests; Markdown 195 files. Svelte had no errors/warnings; fmt, Clippy -D warnings
  and frontend build passed.
- Debian cargo test --workspace --locked: 94 groups/974 passed, core 876/app 98, nine ignored
  by default; release/deb/AppImage builds passed.
- Windows NSIS: 12 checks, root 1791285323343; headless: 11, 1791285322853; native WebView2/IPC:
  17, 1791285395704; receiver: eight, 1791285577694. Twenty small-data startups had P95 788.5 ms;
  no owned credentials/installation integrations remained.
- Debian lifecycle root 1791285259847: nine groups/47 checks, actual deb round trips, read-only
  FUSE mounting/GTK GUI/released mounts on exit, and Orca keyboard/AT-SPI names/speech across
  five pages. Application UID 1000, CapEff=0, default seccomp, no runtime network/host mounts.
  ALSA null does not verify physical audibility; containers do not verify host logout/full
  desktop integration.

| Artifact | Bytes | SHA-256 |
| --- | --- | --- |
| Windows exe | 10,024,960 | `9a58a275d376f176ee5f8a7bc35920bd758a21b11af294319fce13305ce4bc72` |
| Windows NSIS | 3,955,414 | `ad7b6d5d6b92defc7a568cab3f501f7ba2371021b4c7d6c9baa669dc40e00367` |
| Linux deb | 5,486,240 | `74cb3d8b8d99ac4cdebd47b3c29afd4782c5e909b13eec5c492180fb1f151d4a` |
| Linux AppImage | 111,155,704 | `41e4a09f7c69791aacbb98f17bf05e787d142c543523cf4a0d892a032c8ea625` |

cline-readback-1791285253 upgraded the old database from the previous deb, SHA-256
2ba79f090ef269aa48613f395278ade570fc1df6382e0437608e96554669cbb8, to the table's current deb.
Two old-package scans found no SDK instances/events; the new package found one instance,
one physical file/checkpoint and three observations. Repeat scans added zero. 8922/48/8970
and known cache read 5881 matched; health was ok/latest_fallback and original source-file
SHA-256 listings remained unchanged. Version fallback does not downgrade confirmed-usage health.

cline-windows-1791285320382 read the allowlisted native test data twice with the actual current
Windows executable, producing the same three observations, values and unknowns without changing
source bytes. This verifies Windows application reading, without claiming Windows Cline
extension execution from the Debian extension run.

The same package reread unchanged sources for ten M8 products plus Hermes/Qwen/OpenCode;
all exited 0 and preserved values/coverage limits. Product prefixes keep same-second roots
independent: Aider/jcode 1791285412, AtomCode 1791285408, Continue 1791285412,
Crush/Goose/Junie/Xum 1791285411, gajae-code 1791285410, Roo 1791285414;
Hermes hermes-readback-1791285322; OpenCode default/comparison 1791285327/1791285330.
Qwen SDK primary/background and native/retained partitions still read two calls/16,423 tokens/
three cache-read tokens. These repeats do not close untested product scenarios.

Untracked allowlisted test data audit: 54 files/132 JSON objects, zero credential keys/private
paths. The initial local-link check's sandbox git EPERM is retained separately; after obtaining
execution permission, the check found and fixed a sample README relative-path depth error.
Actual reference checks passed. Tool failures remain separate from product failures; earlier
stage passes do not substitute for new SDK artifact acceptance.
