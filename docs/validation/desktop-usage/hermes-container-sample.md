# Native Hermes container samples and corrected usage statistics

<a id="hermes-真实容器样本与统计修正"></a>

On 2026-10-06, official Hermes Agent 0.21.5 ran in isolated rootless Podman on WSL Debian.
Public CLI and --resume latest each called an actual local model, both exit 0. Native
database/CLI reports/independent model-server usage confirm input_tokens is uncached input;
missing fields/initialized zeros cannot establish reported zero. Parser and old-database
replay are corrected; package checks below cover this stage. Later [OpenClaw-stage](openclaw-container-sample.md)
packages passed Hermes original-database upgrades/rescans again; these hashes/counts retain
their original stage's scope.

<a id="固定来源与运行环境"></a>

## Fixed sources and environment

- [Official v2026.9.24 release](https://github.com/NousResearch/hermes-agent/releases/tag/v2026.9.24),
  actual version 0.21.5, commit f97608f178d1ffeca59860195ab7da295f7c8e5f.
- Official docker.io/nousresearch/hermes-agent amd64 digest
  sha256:2fd023efbb8d3d2b0ce1a73d028b07370cff34f567cfe0e999553e8c327ea283.
  OCI revision/image provenance/version command agree; official client unmodified.
- Public custom/base_url configuration from [custom providers](https://hermes-agent.nousresearch.com/docs/integrations/providers/)
  and [local models](https://hermes-agent.nousresearch.com/docs/user-guide/local-models/).
  No account credentials supplied. Internal SDK keyless placeholder is not a real credential.
- [Qwen3.5-0.8B original model](https://huggingface.co/Qwen/Qwen3.5-0.8B), revision
  2fc06364715b967f1860aea9cf38778875588b17, native context 262144. [ggml-org GGUF](https://huggingface.co/ggml-org/Qwen3.5-0.8B-GGUF/tree/8fea620810c4afa23dd6443f999a48574c1611a3)
  BF16 file 1,557,662,496 bytes, SHA256 9a7bed4041b7975e0f71fa34670d1e9025213bc92905ac0db75d36c4fa3fa623,
  matching official LFS digest. Actual llama-server log: n_slots=1, n_ctx_slot=64000;
  bounded output. Actual inference, without fabricated responses/usage.
- Model container network=none; client shares only its loopback. No host mounts/personal
  settings. CLI UID 10000, CapEff=0, default seccomp=2, empty isolated work directory,
  empty CLI toolsets; memory/compression/title generation disabled. External commands
  limited to 180 seconds each; cleanup only owned containers.
- Transparent proxy records only allowed model/status/stream-options/usage fields and
  forwards requests/responses unchanged. Message text excluded from redacted test data.

<a id="三方用量核对"></a>

## Three-way usage comparison

| Scenario | API total input | API cache read | CLI uncached input | Output | API/CLI total | Source calls |
| --- | --- | --- | --- | --- | --- | --- |
| First CLI | 816 | 0 | 816 | 2 | 818 | 1 |
| Public resume | 845 | 812 | 33 | 2 | 847 | 1 |
| Combined | 1661 | 812 | 849 | 4 | 1665 | 2 |

Native session_model_usage has one cumulative main-task row: uncached input 849, cache
read 812, output 4, api_call_count=2; cache write/reasoning zero. first_seen=1791281502.5680463
and last_seen=1791281505.302139 mark aggregate writes, without individual call times.
Complete original database: 270,336 bytes, SHA256
cfc28012eb3ff398b865a6337d5eadb814f0d5f321cd0a5fd7b6dafd9ae8d32d.

[Redacted test data](../../../desktop/src-tauri/crates/core/tests/fixtures/hermes/real-0.21.5/provenance.json)
preserves native DDL and permitted time/count/route fields; replaces session identities;
excludes messages/FTS/text/settings/paths/credentials. Read-only extraction leaves the
original database hash unchanged. Extracted data, API and CLI sizes/hashes recorded separately in provenance.

<a id="修正合同及真实范围"></a>

<a id="修正后的字段规则及真实样本范围"></a>

## Corrected field rules and native coverage

[Fixed normalization code](https://github.com/NousResearch/hermes-agent/blob/f97608f178d1ffeca59860195ab7da295f7c8e5f/agent/usage_pricing.py)
agrees with original A24 commit ef70b3661cbfcf57e583008ad91dd04d8ba46070: OpenAI prompt
subtracts cache read/write before saving uncached input; reasoning is an output subset.
Missing fields initialize to zero without stored validity markers. Positive fields are
reported; zeros unknown. Derive input/full total only when all required fields are known;
checked_add overflow remains unknown. Zero call count also unknown.

Application must retain uncached 849/cache read 812/output 4/source calls 2. Input total,
full total/cache write/reasoning remain unknown. Complete API values are independent
comparisons and cannot fill native unknowns. This remains an interval aggregate, without
daily totals/events; current data layer lacks model/cost fields for these aggregates.

Parser hermes-session-model-usage-2 replays old processing positions through actual discovery.
Only a reconstructed complete old summary matching the stored one permits correction
of old input_total assignment/default zeros. Stable identity/first import/source revision
retained. Changed positive values/quality/interval/coverage/call counts still undergo normal
conflict handling. Events/aggregates/cursors share a transaction; failure can replay.
Malformed typed rows get individual diagnostics while other valid cumulative rows remain readable.

Actual database schema=30. [Online storage documentation](https://hermes-agent.nousresearch.com/docs/developer-guide/session-storage/)
has evolved to other schemas, unsuitable for this fixed version. Database migration version
does not identify each row's client version: registry remains empty/latest_fallback, without
verifying other historical sessions. Gateway/auxiliary/compression/subagents/mixed models or
versions/v20 historical backfill/absolute writes have no native acceptance; synthetic checks retained.

<a id="验证及首次失败"></a>

## Validation and first failures

- Initial 32K model exited 1 at the client's minimum 64000-context check; no HTTP calls/usage
  rows. hermes-real-1791281141 preserves the failure. Switched to an actual model with enough
  native context, without bypassing the check.
- Single-call hermes-real-1791281409 exited 0. Final two public commands in
  hermes-real-1791281489 both exited 0.
- Initial format tests: 13 passed/1 failed because the new test used nonexistent app column
  detection_json. Corrected to actual format_status; all 14 Hermes tests passed. Five new
  tests cover native full-registry/rescan, old positions/parallelism/rollback, protected-field
  conflicts, malformed/negative rows and default zero. Added mapping-zero/overflow unit checks.
- First full verification exited 101 compiling unit tests through Clippy: TokenQuality lacks
  PartialEq, so whole-object equality did not compile. Asserted unknown per field, preserving
  domain types.
- Later regression found accepting any aggregate weakened Codex rejection without individual
  usage records. Restricted acceptance to validated exclusive cumulative rows. duplicate/
  overlap_unknown comparison snapshots cannot establish compatible format. Retained Codex
  rejection and Hermes mixed valid/bad-row regressions.
- Full verification exit 0: Windows Rust 967, core 866/app 101, 8 ignored; frontend 21/scripts
  4/Markdown 193; clean Svelte/fmt/Clippy -D warnings/build. Debian Rust 964, core 866/app 98,
  9 ignored, plus deb/AppImage builds exit 0.
- hermes-readback-1791283098: installed new package against untouched original database.
  Set old package's consumed position beyond sample times, upgraded, and confirmed automatic
  full replay. Uncached 849/cache read 812/output 4/source calls 2; unknown fields stay unknown.
  Old identity/interval/revision/first import retained; rule updates 1, actual conflicts 0,
  old audit history 1, cursor 1. Repeat scan adds no duplicates; original bytes unchanged;
  health=ok/latest_fallback.
- Current Windows NSIS 12 checks exit 0, root 1791283166963; headless 11 checks/3 events/75
  tokens exit 0, 1791283234990; receiver 8 checks/zero owned credentials exit 0, 1791283236566.
  Native desktop 17 checks, 20 first screens P95 745.6 ms exit 0, 1791283243856.
- Current Linux deb lifecycle/FUSE/GTK/WebKit/Orca: 9 groups/47 checks exit 0, 1791283121749.
  Retained five-page navigation/six focus names/actual speech/FUSE release-on-exit results;
  same rootless ordinary GUI user/default seccomp/no external network or host mounts as prior stage.
- Current deb readback/rescan of ten M8 original sources and Continue/gajae-code/AtomCode/
  Junie/Xum/Roo old-DB upgrades exit 0. Qwen native/SDK/retained-archive partition selection
  and OpenCode default/comparison also pass. Values/coverage unchanged; no expanded product
  scenarios. Readback roots: Continue 1791283134, AtomCode 1791283136, gajae-code/Roo
  1791283138, Junie 1791283179, Goose/Crush 1791283180, Xum 1791283182, jcode 1791283211,
  Aider 1791283212. OpenCode default/comparison 1791283211/1791283217; Qwen logs retained separately.
- Redaction audit: 50 files/129 JSON objects, zero credential keys/private paths; affected
  Markdown local links pass. Skill static check initially failed under GBK, then passed
  Python UTF-8 mode. Description unchanged; no model-routing assessment. All temporary
  commands/logs/raw databases/first failures under root build/plan-final-push/.

<a id="本阶段受测包"></a>

## Packages tested at this stage

| Artifact | Bytes | SHA256 |
| --- | --- | --- |
| Windows release | 10008064 | 0496e86a003823df11da72def5e63b8041baf35f5c7b9d7d1c9c646ccea7275f |
| Windows NSIS 0.2.1 | 3951219 | f96e564e2128df12fae896baa48b01540c37715eed382e96a57912fae83358b5 |
| Debian 0.2.1 | 5477020 | 2ba79f090ef269aa48613f395278ade570fc1df6382e0437608e96554669cbb8 |
| AppImage 0.2.1 | 111151608 | a75bf3bc27d9518c750c79e005de9e7befdd745f24297d827111c68a3b9d80ba |

Old Hermes comparison uses prior deb a7a607943723547e10c8e2e05cf42b176f38e55e76ab35e9ae0fc39d2d5e76e6.
Windows/Linux lifecycle checks each use saved 0.2.0 packages. Uncommitted tree; tested packages
do not establish published artifacts or remote CI results.
