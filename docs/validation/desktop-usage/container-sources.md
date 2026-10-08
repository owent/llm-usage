# Official clients and real local-model samples in containers

<a id="容器内官方客户端与真实本地模型样本"></a>

2026-10-06 update: the Qwen 0.25.0 consecutive-JSON SDK reader, per-call spans and exclusive
selection of span/native partitions are implemented: two real calls, 16,423 tokens. OpenCode
1.18.34 is registered by each record's actual version; upgrading old processing positions
has passed. Routing manual roots containing real first Qwen records is also corrected.
See [source-rule upgrades](source-policy-upgrades.md) for current implementation/package
read-back and remaining limits, [M8 container samples](m8-container-samples.md) for eight
clients' later real samples/package checks, and [three-source acceptance](m3-container-samples.md)
for later MiMo/Zoo/DSH native files and packages. The rest preserves the original 2026-10-05
checks and package results at that time.

Date: 2026-10-05; cwd is this task's isolated WSL Debian checkout. Preparation follows
[real-data validation](../../design/desktop-usage/implementation-readiness.md).
The user authorized Podman testing. Runs used a local model without personal account login,
personal keys or paid cloud requests. Software installation and actual model-reported tokens
are recorded separately.

<a id="已核验环境"></a>

## Verified environment

Rootless Podman 5.4.2; derived from the Debian GUI image before CJK fonts were added in
[installation acceptance](installation-lifecycle.md), image ID
311d072b80b15c3a57a9afd8135deed530931aec6e4afac81361f35f6f19793e.
Client/model user UID 1000, fresh HOME, no host mounts, --network=none. CPU llama-server
listens only on container 127.0.0.1:8080 to generate real local records; it does not connect
to remote usage/billing APIs. Image builds and public package/model downloads used networking;
execution was offline.

- Official npm Qwen Code 0.25.0, Node 24.21.0. Registry manifest requires Node >=22;
  dist.integrity is
  sha512-XlqtxN7UKEkXLpuKioorDTBoIR/rWMdNHMtO/aPuQb7HDsTswMPqkEMYsVY56wuF3xSaI9PKgh9Q2NU/CSTkAQ==.
  The [npm registry entry](https://registry.npmjs.org/@qwen-code%2fqwen-code/0.25.0)
  and actual --version determine installation; old versions in search summaries do not.
- [Qwen's official model-provider documentation](https://qwenlm.github.io/qwen-code-docs/en/users/configuration/model-providers/)
  confirms an OpenAI-compatible local provider. Actual CLI --help verified auth/base-url/model/
  plan/max-session-turns/output-format. The container uses fresh configuration and a local
  authentication placeholder without account significance.
- Official [Qwen2.5-0.5B-Instruct-GGUF](https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/tree/9217f5db79a29953eb74d5343926648285ec7e67),
  q4_k_m SHA-256: 74a4da8c9fdbcd15bd1f6d01d621410d31c6fc00986f5eb687824e7b93d7a9db.
- Official llama.cpp server, fixed amd64 manifest:
  ghcr.io/ggml-org/llama.cpp@sha256:559ac229adefe0f7e2d4e32f5222f26927b6bb8ba44b9db08e2544afebf41984.
  Actual version 0.5.0-dev build 11382 / commit 11fe02151; derived image ID
  c66a5d2978b14379e7c29493fd48f12e3c4ef9c13ebb7e3cbbcf61d20e1d9060.

Qwen ran in plan mode with one turn and JSON output; actual tool calls: zero. The application
used the current real 0.2.1 deb's --scan-once with isolated application data and source
environment. Model responses and token values were not fabricated.

<a id="默认场景及对照"></a>

## Default and comparison scenarios

| Scenario | CLI-reported calls / total tokens | ChatRecord / application | Independent comparison |
| --- | --- | --- | --- |
| Default automatic memory | 2 / 16,082; bySource.main=10,228, managed-auto-memory-extractor=5,854 | Main loop only: 1 record, input 10,226, output 2, cache read 0, total 10,228 | Main loop matches model-server timing; no per-call session record for background work; coverage gap retained |
| Background memory disabled | 1 / 8,905; main only | 1 record, input 8,903, output 2, cache read 0, total 8,905 | CLI, model server, native file and actual SQLite agree; four checks exited 0 |

Installed 0.25.0 chunks verified the comparison configuration: schema defaults true; loader
reads settings.memory?.enableManagedAutoMemory and enableManagedAutoDream; the forked agent
is named managed-auto-memory-extractor. Only the new container sets both false. Main-loop
checks cannot establish that every default request is collected; do not manufacture events
by subtracting session usage from CLI totals. Background CLI explicitly reports prompt=5,639,
cached=3, total=5,854; server timing reports prompt=5,636, total=5,851. Timing counts differ
from API usage and cannot replace API totals. That background work did not enter application events.

Each application run discovered one qwen-code source, health=ok; first import added one record,
repeat added zero, diagnostics=0. Events retain schema_version=0.25.0 and raw local model
qwen2.5-0.5b-local; canonical/provider remain unknown without cloud-billing inference.
Complete known input/output does not establish coverage of every client call.

The formal comparison script exited 0. The initial default offline audit exited 1 due to a
wrong SQL column; subsequent read-only checks used the actual schema. Model/client runs and
both collections had succeeded. The complete default native file was not successfully copied
out of the container; retained permitted fields and SQLite/CLI statistics do not establish
preservation of that complete original session. The comparison native file was retained under
the dedicated WSL build/; Windows received only permitted fields and configuration source references.

Results: build/install-lifecycle/qwen-evidence/ and WSL real-qwen-default-results/ /
real-qwen-controlled-results/. Only redacted selected fields enter tests/fixtures/qwen/real-0.25.0-local-*:
remove bodies/project paths, anonymize IDs, shift timestamps within the same day, preserve tokens.
Regression tests check both samples' fields, unknowns, call counts and unchanged revisions on repeats.
Three Qwen tests on each of Windows/Debian exited 0. Final capabilities replace the earlier
no-real-sample statement and describe default background-memory coverage limits. A final deb
read the saved comparison file in another offline container: first import one, repeat zero,
8,905 tokens, health=ok; SQLite capability text updated, exit 0. This read-back made no further
model requests; see build/install-lifecycle/final-qwen-readback.json.

<a id="qwen-0250-真实-file-遥测"></a>

## Real Qwen 0.25.0 file telemetry

Another isolated offline container with fresh HOME/local model/default background memory enabled
telemetry.outfile, with logPrompts/includeSensitiveSpanAttributes=false and usageStatisticsEnabled=false.
The [fixed v0.25.0 SDK implementation](https://github.com/QwenLM/qwen-code/blob/6788c035698a0ada471c958d1e789e96c6cddd9b/packages/core/src/telemetry/sdk-impl.ts)
shows outfile suppresses the OTLP exporter.
[file-exporters](https://github.com/QwenLM/qwen-code/blob/6788c035698a0ada471c958d1e789e96c6cddd9b/packages/core/src/telemetry/file-exporters.ts)
serializes SDK objects as consecutive multiline JSON, rather than JSONL or OTLP JSON envelopes.

Actual export: 25 objects, including two qwen-code.api_response logs and two qwen-code.llm_request
spans. Spans have kind=0 (SDK INTERNAL), private `_spanContext` identity, and service.name/version
qwen-code/0.25.0 in `resource._rawAttributes`. The main-loop span has parentSpanContext; background
memory has none. A shared session cannot establish a parent/child relationship. Do not add logs,
spans, HTTP spans and metrics together.

| Source | Independently compared API-response log / LLM span / CLI bySource | Native session / application |
| --- | --- | --- |
| main | input 10,226, output 2, cache read 0, total 10,228 | 1 record / 10,228 tokens |
| managed-auto-memory-extractor | input 5,639, output 556, cache read 3, total 6,195 | Call absent from native session; no fabricated event |
| Total | CLI 2 requests, 16,423 tokens, zero tool calls; fields agree with two independent logs | Repeat application scan still 1 record, health=ok, diagnostics=0 |

Five independent checks exited 0. Background cache read 3 is directly reported by API/telemetry;
main-loop cache hits and other providers/versions remained unverified. The real file supplied
background per-call records, but the then-current OTel JSONL/Copilot reader did not cover this
shape. Changing a suffix/span name could not establish compatibility. At this historical stage
the reader and authoritative native/span partition selection were unimplemented, so default
background coverage remained incomplete. The 2026-10-06 update above supersedes that implementation status.

Original file/session/SQLite remain in WSL build/install-lifecycle/real-qwen-telemetry-1791206781/.
Windows retains qwen-telemetry-whitelist.json, qwen-export-audit.json and fixed source only.
Original file SHA-256: 5d0abf2ac630aaf64c13a7f870eb7c264d5ebc91e5b3dc534de7ba10925a4ef5.
This original batch changed neither parsing nor combined statistics from the two record formats.

<a id="opencode-11834-主循环与默认标题"></a>

## OpenCode 1.18.34 main loop and default titles

The same offline rootless environment used the official
[opencode-linux-x64 1.18.34 manifest](https://registry.npmjs.org/opencode-linux-x64/1.18.34),
fixed source [aec0b9a6d8898f68f923aaf08b7306d931fd9d76](https://github.com/anomalyco/opencode/tree/aec0b9a6d8898f68f923aaf08b7306d931fd9d76).
Official dist.integrity:
sha512-RTAMjCve4euxP2QKLuvRmdoW5J5DQK1DiZqt+7slfixyjAEi79QC2Df2oYKogibaAI4IEU8uzenoJeEl3k+UEw==.
After official direct downloads timed out, an npmmirror tarball with the same name/version was
used. Before execution it was checked against that official SHA-512; the mirror's own digest
did not establish authenticity. Actual package: 60,309,530 bytes, SHA-256
b83e8ac66d752d05ead4b6a439d3a2cfa32bcd9817c708825a389b5d5cba4f19; --version=1.18.34.
Only the official binary was extracted; npm lifecycle scripts were not run.

[Official llama.cpp provider documentation](https://opencode.ai/docs/providers/#llama-cpp)
and actual run --help were checked. A new HOME defines a provider/primary agent (steps=1,
all permissions denied). run --pure disables external plugins; model fetching, automatic updates
and sharing are disabled; zero tool calls. Only API-response usage, model and role counts are
saved locally. A container-loopback proxy stores no request bodies/model outputs/authentication
headers and does not connect to remote usage/billing APIs.

| Scenario | Real API usage | CLI / native files / application |
| --- | --- | --- |
| Default title | 2 calls: title input 539, output 10, total 549; main input 298 (including cache read 3), output 1, total 299; combined 848 | Main loop only: 1 record, uncached input 295, cache read 3, write 0, output 1, reasoning 0, total 299 |
| run --title comparison | 1 call: input 298, cache read 0, output 1, total 299 | Same single main-loop record: input 298, cache read/write 0, output 1, reasoning 0, total 299 |

Fixed [ensureTitle](https://github.com/anomalyco/opencode/blob/aec0b9a6d8898f68f923aaf08b7306d931fd9d76/packages/opencode/src/session/prompt.ts)
starts an independent LLM stream for default titles and uses only its text; explicit titles
skip it. The [main-loop processor](https://github.com/anomalyco/opencode/blob/aec0b9a6d8898f68f923aaf08b7306d931fd9d76/packages/opencode/src/session/processor.ts)
writes step usage to a part and assistant message. Session totals match parts and exclude title
usage. A match cannot establish full-client coverage. Do not invent title events from a difference
or add the three native representations. Initial server timing had two inference jobs without
usable HTTP request counts; later checks used real SSE usage. Timing cannot replace API token counts.

Five independent checks per scenario exited 0. Two actual --scan-once runs still leave one
record/299 tokens, health=ok, diagnostics=0. Second-scan events=1 counts parsing in the overlap
window; SQLite adds no event. Raw model qwen2.5-0.5b-local and provider llama.cpp are retained
without verifying cloud prices. Native session/part/message row counts: 1/4/2; the new core
session_message table has zero rows. An empty table does not verify a new storage format;
offline execution does not establish every process's behavior when networking is enabled.

WSL build/install-lifecycle/real-opencode-default-1791208377/ and real-opencode-controlled-1791208422/
retain native files, permitted API fields and application DBs. Windows retains opencode-default-audit.json,
opencode-controlled-audit.json, anonymized selected fields and fixed source. Only three-table DDL,
usage and necessary anonymous fields enter the
[default test sample](../../../desktop/src-tauri/crates/core/tests/fixtures/opencode/real-1.18.34-local-default/_expectations.md)
and [title comparison sample](../../../desktop/src-tauri/crates/core/tests/fixtures/opencode/real-1.18.34-local-controlled/_expectations.md).
Eight OpenCode tests on each of Windows/Debian exited 0: individual fields, cumulative comparison
and unchanged revisions on repeat reads.

At this historical stage data remained latest_fallback: file selection used the database's
highest version. Per-record verification, mixed versions, empty sessions and upgrading unchanged
old cursors were incomplete; registering one real version could not verify every version in the database.
Capabilities described the real scope/title gap/upgrade requirements without changing reading,
calculation or version dispatch. The deb with those descriptions was rebuilt and read both saved
native DBs in new containers: after two scans each retained one record/299 tokens, cache reads 3/0,
health=ok; SQLite capability text real-local, version registry empty, exit 0. Saved Qwen comparison
data still had one record/8,905 tokens. Results:
build/install-lifecycle/fuse-continuation-opencode-{default,controlled}-result.json and
fuse-continuation-qwen-readback.json; no further model requests. The update above records later
1.18.34 per-record and old-cursor acceptance.

<a id="仍待验证"></a>

## Remaining checks

This batch verifies Qwen 0.25.0 main-loop usage with a local compatible provider; it does not
verify Qwen cloud, other versions, real archives or main-loop cache hits. SDK background records
were integrated on 2026-10-06; valid existing records do not require degraded source health.
Gemini's [official authentication documentation](https://geminicli.com/docs/get-started/authentication/)
still lists Google login/API key/Vertex routes. This batch did not verify an official offline
provider or find existing nonempty native records. Third-party forks, simulated Gemini APIs or
Qwen samples do not verify Gemini versions. OpenCode other versions, Windows layout, nonzero cost,
cache writes, reasoning, independent title records and mixed-version records remain unverified;
1.18.34 per-record versions and upgrading old cursors have passed. The local Windows Zed database
was still empty at this stage; other products' real-sample conditions remain in Plan.md.
