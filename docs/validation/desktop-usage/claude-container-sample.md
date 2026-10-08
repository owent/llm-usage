# Claude Code domestic mirror and native usage acceptance

<a id="claude-code-国内镜像与原生用量验收"></a>

2026-10-07; Claude Code 2.1.197, WSL Debian/rootless Podman 5.4.2, normal uid 1000,
Node 24; application 0.2.1 on Windows 11 x64. User-authorized Zhipu Coding Plan credentials
made one minimal request each for glm-5.3-flash and glm-5.3. Downloads/scripts/logs/check
databases remain under root build/claude-cn-20261007/; no commit/publication.

<a id="国内下载与安装"></a>

## Domestic download and installation

Both @anthropic-ai/claude-code and @anthropic-ai/claude-code-linux-x64 fixed to 2.1.197;
actual tarballs downloaded through [npmmirror](https://npmmirror.com/). Package name/version/
SHA-512 integrity match independently read official npm metadata; downloaded bytes pass SHA-512.

| Package | Bytes | SHA-256 |
| --- | --- | --- |
| claude-code | 19,918 | 0481de729ef296a62291f26227f76d47741536a4fd81097237448d7769b83199 |
| claude-code-linux-x64 | 77,071,446 | 42b12aa7a1d57d9f48b49acecca37643a81528ad873629e0fc5f043730621b14 |

Downloads took about 0.22/6.93 s. Verified local packages installed offline without inherited
secrets. Package install.cjs creates native bin/claude.exe, the actual Linux executable,
rather than old cli.js. Version returns 2.1.197 (Claude Code); help/install exit 0. Binary:
245,517,112 bytes, SHA-256 f54e69cbc89b2da61a415700af7ff52a147e862517d4f1b0eecf768448cf7f83,
identical to platform-package original. See [official setup](https://code.claude.com/docs/en/setup).

<a id="真实请求与载体"></a>

<a id="真实请求与保存记录"></a>

## Real requests and saved records

Following [official Zhipu Claude settings](https://docs.bigmodel.cn/cn/coding-plan/tool/claude),
the authorized key reaches only the target process through ANTHROPIC_AUTH_TOKEN. The user's
OpenAI-compatible Coding Plan URL corresponds to Anthropic endpoint
<https://open.bigmodel.cn/api/anthropic>. Isolate HOME/project/config; disable tools, updates
and unnecessary telemetry. Command: claude -p constant-OK-request --model model --max-turns 1
--tools '' --output-format json. No product login or fabricated authentication state.

A one-time local forwarder in the container allows only the two named models/vendor endpoint
and forwards streaming responses unchanged. It saves no request/response bodies, headers
or keys, only path/model/HTTP status/usage. Each call makes one /v1/messages?beta=true request;
both HTTP 200, CLI exit 0, subtype=success, is_error=false, num_turns=1.

| Model | Uncached input | Output | HTTP calls | Native assistant entries |
| --- | --- | --- | --- | --- |
| glm-5.3-flash | 1,339 | 33 | 1 | 2 |
| glm-5.3 | 1,338 | 23 | 1 | 2 |

Final message_delta.usage, CLI and native message.usage positive input/output agree. Each
native session has six lines: queue/user/two assistant/last-prompt. Assistants carry
version=2.1.197; two content blocks share message.id and lack requestId. Distinct UUIDs/
completion times do not split calls. Total: **two calls, 2,677 uncached input, 56 output**.

message_start initially reports input/output zero; final delta has positive values and
omits cache_creation_input_tokens, yet CLI/native writer inserts zero. Native records lack
zero-validity flags, so initialized zero buckets remain unknown. This request's explicit
API cache-read zero cannot verify zeros in other local history. Cache read/write, complete
input/total and reasoning remain unknown; positive buckets usable independently. Records
lack provider/channel; provider remains unknown without protocol/model-name guesses.
CLI total_cost_usd is a client estimate, not bill/occurrence-time usage pricing. Endpoint
is Coding Plan; channel conditions remain.

Public samples extract only permitted native statistics, removing bodies/paths/authentication
data and hashing stable IDs. provenance retains original-file SHA-256. Application executable
readback uses the extraction, without claiming to import every original byte. Actual CLI
establishes native format; no synthetic exporter substitutes for it.

<a id="应用修正与验证"></a>

## Application corrections and checks

Select rules per assistant version: 2.1.197 verified, other versions compatible only;
legacy unversioned documented format remains separate. Positive native buckets reported,
initialized zero unknown; no totals derived with missing components. Reject documented-format
deviations, retain unknown-field diagnostics and exclude bodies.

Old executable first readback: two calls/2,677 input/56 output, but false zero cache read/write,
complete total 2,733 and fixed Anthropic provider. This field-meaning defect was not accepted
as success. New rules replay consumed unchanged processing positions and correct only
matching complete old event summaries. Real token/model/quality/attribution/revision changes
still follow conflict rules. Preserve first observation, original completion, conflicts/history;
recompute unsealed summaries in the same transaction. Resume after read limits; bad snapshots
do not mark rule updates complete.

| Check | Command/entry | Exit/result |
| --- | --- | --- |
| Domestic downloads/native install/two models | Task download.py, run.py/driver.py/launch.py | 0; both integrities match; installation/version/help/two real calls succeed; no containers remain |
| Initial Claude regressions | claude_contract, claude_gaps_synthetic, claude_incremental_v12, claude_native_contract | 0; 28 pass |
| Unified checks | npm run verify | 0; Rust 1,024 pass/8 platform ignores, frontend 22/script 4; clean types/build |
| Complete old summaries/same-batch conflict | claude_native_contract rerun after added conflict case | 0; 6 pass: unchanged cursor, two old summaries, rollback, model/quality/revision conflicts, bounded continuation; overlaps unified totals |
| Final Rust format/static | cargo fmt --all --check; cargo clippy --workspace --all-targets --locked -- -D warnings | 0 |
| Windows release executable | npm run build:desktop -- --no-bundle | 0; no new release installer |
| Executable full registry/SQLite/native extraction | readback.mjs: old DB upgrade/new independent DB, read/repeat | 0; both two calls/2,677 input/56 output, unknown cache/complete total/provider/cost; files owned only by Claude |
| Headless regressions | npm run test:headless | 0; 11 pass, isolated source environment |
| Documents/links/secrets | npm run lint:md, check-public.py, git diff --check | 0; 208 clean Markdown files, 296 local links, no actual-key matches across 63 public changed files |

Old DB upgrade neither rebuilt the database nor edited samples. Original keys/first
observations/completion times match; diagnostics kept, two parser_policy_updated added.
Repeated scans keep data_revision. Independent new DB gets identical summaries/statistics.
Background reads only local extracted data, without extra vendor calls. Executable/SQLite
and headless results add no GUI/IPC or Linux-package lifecycle acceptance.

First failures retained: an earlier official-source attempt timed out within limits and
obtained no usage. This version uses a native platform package; old cli.js is invalid.
First capability-list expectation still listed only the documented format; an API sample
test read the wrong wrapper. Both corrected/rerun without counting failures as passes.

<a id="受测边界"></a>

## Tested scope

This verifies Linux x64 native Claude CLI's Zhipu-compatible main loop and permitted-field
reading. Anthropic models, positive cache buckets, subagents/auxiliary work, retries/failures/
cancellations, migration and other versions lack corresponding nonempty native samples and
are not inferred from this success. No Windows-native Claude installation/GUI acceptance.
Owned container stopped/deleted; label lookup found none remaining.
