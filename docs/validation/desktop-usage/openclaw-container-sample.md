# OpenClaw native container sample and runtime database

<a id="openclaw-真实容器样本与运行库读取"></a>

Date: 2026-10-06; uncommitted 0.2.1 worktree. Official OpenClaw 2026.9.8 CLI made two
real local-model calls through public agent --local in isolated rootless Podman; both
exited 0. Schema 24 hot-transcript read-only parsing and full-registry discovery are
implemented. Old-database upgrades, two Windows executable reads and package lifecycle
checks passed. Other protocols, cold archives and gateway paths retain separate limits.
Behavior: [runtime database rules](../../design/desktop-usage/openclaw-runtime.md).

<a id="分发物与依据"></a>

## Distribution and references

- Official [npm metadata](https://registry.npmjs.org/openclaw/2026.9.8) and
  [tarball](https://registry.npmjs.org/openclaw/-/openclaw-2026.9.8.tgz): dist SHA1
  7246c389c7134d9c15622082772912c7082f68af, 13,320 files, unpacked 388,955,824 bytes.
  npm integrity:
  sha512-G+JkNUhtpDE3cXR4AEi2NyyG9fqI/T2WUSl8ZnR8AATH8Dh1kC3qYFL7wwPoZtgHiP/cszA86PEiE0PDysxb9Q==.
- [GitHub v2026.9.8](https://github.com/openclaw/openclaw/releases/tag/v2026.9.8)
  released 2026-10-03, tag commit fc23bc864e4553c2d215e479eeec47b67a0bf943.
  Actual CLI: OpenClaw 2026.9.8 (fc23bc8).
- [Public npm provenance](https://registry.npmjs.org/-/npm/v1/attestations/openclaw@2026.9.8)
  claims build commit aa6008ad198ef99c43f9d89dbd01694708712974 and workflow
  .github/workflows/openclaw-npm-release.yml;
  [build run](https://github.com/openclaw/openclaw/actions/runs/37087759921/attempts/1).
  Decoded statement and subject SHA512 match dist integrity. Signature/transparency-log
  trust was not independently verified.
- Actual dist/package-update-activation-recovery.mjs SHA256:
  e08dfc1fb3ba7962f9e01d7a6770117c6cd77406b3fa4f7ba94788a670030ba0.
  Its OpenAI normalizer matches fc23 and lacks aa6008 contextUsage. Parse using actual
  installed code/schema/responses without applying undeployed fields from the claimed commit.
  Mutable database app_version does not identify historical record versions.

WSL Debian, Node 24.21.0; official engines >=24.16.0 <25 || >=26.1.0.
Image localhost/llm-usage-openclaw:2026.9.8 ID:
4ba2f82099166a8ac7bcf60b4c60c1488786bf1961b063f86957ec04e84b9bcb.
Initial --omit=optional installation exited 1 due to Koffi/CMake. Preserve that log.
Installing cmake/build-essential and restoring optional packages produced exit 0. Keep npm's
unapproved-install-script notices; tests did not bypass approval. Client source/distribution
were unchanged.

<a id="真实运行与独立核对"></a>

## Real runs and independent comparison

Successful WSL root: build/plan-final-push/openclaw-real-1791288304. Ordinary UID 1000,
CapEff=0, default seccomp, no network/host mounts; owned containers reclaimed.
Custom OpenAI-compatible provider connects only to the container loopback model service.
Public local marker ollama-local contains no personal credentials. Configuration validation
exited 0 first. All tools disabled, thinking off and telemetry off; no gateway/delivery
or host-IDE changes.

Two runs used the same owned session through the public command:

```sh
openclaw agent --local --agent main --session-id <owned-session-id> \
  --model local-llama/qwen3.5-0.8b-local \
  --message 'Reply with the word OK. Do not call tools or read or change files.' \
  --thinking off --json --timeout 120
```

Model: [Hermes-tested model](hermes-container-sample.md), official Qwen3.5-0.8B BF16 GGUF,
SHA256 9a7bed4041b7975e0f71fa34670d1e9025213bc92905ac0db75d36c4fa3fa623.
llama-server used one slot, context 64,000, eight threads and maximum output 128.
A transparent proxy recorded actual SSE usage and forwarded it unchanged. The client itself
requested include_usage; no responses/usage were fabricated.

| Run | API full prompt | API completion | API cache read | Native input | Native output | Native cacheRead |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 6,105 | 47 | 0 | 6,105 | 47 | 0 |
| 2 | 6,153 | 107 | 6,101 | 52 | 107 | 6,101 |
| Total | 12,258 | 154 | 6,101 | 6,157 | 154 | Known subtotal 6,101 |

Native input excludes cache. cacheWrite defaults to zero, totalTokens is client-computed,
and cost initializes to zero. Store two usage observations, uncached input 6,157, output 154
and known cache-read subtotal 6,101. Complete input/total/source total, cache write,
reasoning, cost and underlying call count remain unknown. The API had two actual requests;
two observations still do not establish general model-call semantics. Occurrence time uses
message.timestamp request start, not DB write time.

Native agents/main/agent/openclaw-agent.sqlite: 733,184 bytes, SHA256
446b41b0eb2bbb105e553a5b0670c757f0d9ce8f86fa70af5e25afe94070b42e.
user_version 24, primary role agent, owner main. Ten transcript events contain assistant
usage at seq 5/8. session_entry_provenance=1, acp_owned=0, empty plugin/hook, harness
openclaw, and matching session_key/agent. Global store schema 19, quarantine schema 2 and
auxiliary files are excluded. No cold archive exists in this sample; it does not verify
cold-archive reading.

Initial root 1791288181 used maximum output 16, receiving HTTP 200 and actual
6,107/16/6,123 usage, but CLI exited 1 with length/incomplete_turn. Retain the original
error. Increasing output and using an independent container yielded two exit-0 runs.
Failed tasks were not classified as zero usage or successful tasks.

<a id="实施与回归"></a>

## Implementation and regressions

Repair discovery at agents/&lt;agent&gt;/agent. Full-registry checks covered environment
precedence and six manual-root forms. Register each physical file once. Recheck old seq
under live WAL instead of treating an unchanged file fingerprint as an unchanged database.
Read-only transactions check schema/provenance, cold-archive manifests and hot rows together.
Bound pagination/cancellation and TEXT/zstd to 4 MiB, isolate invalid rows, and commit
cursors/events together. Isolate other schemas, external CLI copies, migrated sources,
plugin/hook/acp and other protocols. Expose archive gaps; disappearing hot rows do not erase history.

openclaw_runtime_contract 11 and three prior format tests passed: permitted native
record/API fields; discovery/environment precedence; unknown schema/owner; origin
isolation; invalid buckets/computed contradictions; zstd/invalid types/bounded reading;
pagination/WAL/mutable DB versions; concurrency/rollback; archive gaps/history; unknown
default-zero buckets; duplicate identities/real conflicts. zstd/cold boundary samples
are explicitly synthetic, without expanding real-source acceptance. Preserve the initial
compilation failure from an incorrect parallel API and the full-check failure for
Option.is_none_or under MSRV. Correcting actual API usage and MSRV 1.77.2 yielded passing checks.

Redacted test data: core/tests/fixtures/openclaw/real-2026.9.8. Only permitted fields:
four native user/assistant records, API usage and source notes, with replaced IDs.
Bodies/credentials/configuration/original DB are not committed. Redacted-record file SHA256:
68d4db4b60b40b817fd09f853d148ee4299a1506e1c01e8644ec1396e0492d69.
API SHA256: 8edf09c8e6b699385d1466a796ead81511766702d5b5490497826dec236f4a6a.

<a id="当前成品验收"></a>

## Current packaged-build checks

All commands below exited 0; logs/first failures are under root build/plan-final-push:

- Windows npm run verify: Rust 988 (core 887/app 101, eight ignored), frontend 21/scripts
  four. Markdown then covered 197 files; Svelte had no errors/warnings; fmt, Clippy
  -D warnings and frontend build passed.
- Debian workspace: 95 groups/985 passes (core 887/app 98, nine ignored); Clippy and
  release/deb/AppImage builds passed.
- Windows NSIS 12 checks (1791289684773), headless 11 (1791289665186), native WebView2/IPC
  17 (1791289782343), receiver eight (1791290098384). Twenty startup measurements:
  P95 752.4 ms; zero owned credentials/installation-integration leftovers.
- Linux root 1791289583346: nine groups/47 checks, deb round trips, read-only FUSE/GTK GUI/
  release on exit, and [Orca ten-language/five-page](orca-multilang.md) current focus/speech.
  ALSA null does not establish physical audibility; containers do not verify host
  login/logout or complete OS integration.

| Artifact | bytes | SHA256 |
| --- | --- | --- |
| Windows exe | 10,046,976 | 58c7c7cf1484f55645f11c2ff0cc780bb50fb340ae06115ec87442e6732362d3 |
| Windows NSIS | 3,961,563 | f9b43e75ecf67ff92374410b8413aafa38b8a7cceb9a9951395ef512e1d43a73 |
| Linux deb | 5,497,396 | 099676397117791d99c44be53057ded1be44f5808822c7df8dc579a3b415c357 |
| Linux AppImage | 111,163,896 | c10013873e31457a6984fc1366b039794359d3838b648660156e7d2679412b5a |

openclaw-readback-1791289585 upgraded the old DB created by the prior Cline deb
(SHA256 74cb3d8b8d99ac4cdebd47b3c29afd4782c5e909b13eec5c492180fb1f151d4a).
Two old-package reads produced no events and format diagnostics. New-package reads yielded
two observations and one instance/physical file/checkpoint, health ok/latest_fallback;
repeats added nothing, with values above. Main DB/persistent source-file SHA256 stayed
unchanged. SQLite read-only access may update SHM bookkeeping, so the shared-memory sidecar
was excluded from the unchanged assertion.

openclaw-windows-1791289662154 twice read SQLite rebuilt from permitted fields using the
actual Windows executable, with the same values/unknowns. This verifies Windows application
reading, not Windows OpenClaw CLI. Full test-data field audit: 57 files/135 JSON objects,
zero credential keys/private paths.

The same package reread M8's ten sources plus Hermes/Cline/Qwen/OpenCode, all exit 0
with values/unknowns/existing gaps preserved. Aider/jcode/Continue root 1791290118;
AtomCode 1791290116; gajae-code 1791290117; Crush/Hermes/Xum 1791290119;
Goose/Roo/Cline 1791290120; Junie 1791290121. Product prefixes prevent same-second roots
being mixed. OpenCode default/control roots: 1791290118/1791290135.
Qwen SDK main/background and native-first/archived-copy selection stayed at two calls/
16,423 tokens, cache read 3. Retesting did not expand other products' acceptance.
