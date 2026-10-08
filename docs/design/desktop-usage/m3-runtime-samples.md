# Reading native MiMo, Zoo and DSH data

<a id="mimozoo-与-dsh-真实数据读取说明"></a>

On 2026-10-06, official distributions called real local models in isolated rootless
Podman. This file defines reading behavior; completion status belongs to
[Plan.md](../../../Plan.md) and the [three-source record](../../validation/desktop-usage/m3-container-samples.md).
Containers, permitted extraction fields and personal-account limits follow
[implementation preparation](implementation-readiness.md).

## Zoo Code 3.86.0

The [official release](https://github.com/Zoo-Code-Org/Zoo-Code/releases/tag/v3.86.0) maps to
commit 6aa9d0174a9ecae155c6c5db9134bead4b67197d; VSIX digest matches its release asset.
A real VS Code extension task started through the public API. Native ui_messages.json
contains ordinary text, reasoning, recovery prompts and an inline-updated api_req_started.
API, extension callback and file each report input=6,118, output=53. There is no per-request
model/client version; global configuration must not fill these fields.

Accept and skip non-usage messages using the complete ask/say enums in this commit's
packages/types/src/message.ts. Unknown types reject the whole file and retain the cursor.
Preserve legacy finished LIFO merging and auxiliary compression costs. tokensIn includes
cache, whose fields are subsets and are not added again. The actual OpenAI-compatible
writer ignores nested cached_tokens; all four token fields and cost default to zero,
so zeros stay unknown. Positive input/output can derive total tokens. Without complete
cache fields, do not derive uncached input. Verified format fields do not verify unsampled versions.

Parser upgrades recheck unchanged consumed cursors. Correct default zeros and parsing
metadata only when the complete old event summary reconstructed with old rules matches.
Changes to positive values, quality, model, identity, attribution or revision still enter
conflict handling. Test both old-summary versions, parallelism, rollback and same-batch
conflicts while retaining observation times and diagnostic history.

## MiMo Code 0.1.15

The [official release](https://github.com/XiaomiMiMo/MiMo-Code/releases/tag/v0.1.15) maps to
commit 14dfe68a1c121f859544ba810b3c308e8501bfb2; Linux asset integrity was checked.
Actual CLI and session continuation save eight step-finish records in mimocode.db,
matching eight model API usage records individually. CLI exits 0, but all eight finish
reasons are length; this does not verify task completion.

Use this version's session getUsage SDK normalization for input, output, reasoning,
cache and total. Default zeros do not verify absent cache/reasoning. Subtracting SDK
cache from SDK total input produces native input; subtracting SDK reasoning from SDK
total output produces native output. Adding the same components restores positive SDK
totals without establishing default-zero components as reported zero. Keep source total
as a separate comparison; zero cost stays unknown. MiMo mapping remains separate from
OpenCode; ancestry does not infer versions. Read only parts, without adding duplicate
message/session usage. Retain each record's owning-session version; the database's
highest version does not verify other rows. Old-position/complete-old-summary upgrades
must preserve real values, attribution and conflicts.

MIMOCODE_HOME must be a nonempty absolute path, takes precedence over XDG, and uses its
data subdirectory as the base. MIMOCODE_DB accepts absolute/relative database paths;
:memory: has no disk file. Unknown/mixed versions continue per-record latest_fallback;
one 0.1.15 sample does not verify all history.

## DSH 0.2.0-rc.2

The actual distribution is the [official npm package](https://www.npmjs.com/package/@deepseek-ai/dsh).
Its installation lock and compiled persistence, LLM codec and token-meter modules
establish sample field meanings. Current GitHub source differs from the rc.2 distribution
and cannot verify that release.

Two headless/session-continuation runs succeeded. Persistence is session.v4.jsonl.zstd:
a v4 session header followed by seq/time/data events. Two main-loop assistant/message
settlements save usage; embedded stream usage describes the same calls and is not added.
The title API has another call, but only a request marker and no persisted result usage.
Retain the coverage gap rather than inventing an event from a cumulative difference.

The new v4 reader is separate from the earlier documentation-based session log. Check
header, generation version and local origin; limit decompression and check each line.
Test incomplete writes, unknown formats, invalid types, repeated identities and rollback
separately. Use only verified fields from that settlement for usage/model. Imported/seeded
history does not establish local calls. Test default roots, manual roots and registry-wide
physical-file deduplication through actual discovery.

Installed rc.2 token-meter reads assistant/message data.usage. If absent, or for
assistant/attempt, it reads the embedded stream's last usage. For the same turn/step,
keep only the last settlement before retry; retry-started starts a new identity. Stream
copies and context pressure add no usage. Seeded headers require inherited end-seed;
exclude the inherited prefix. Ordinary continuation's empty end-seed is not inheritance.
seq must be consecutive. Reject unknown required types; skip explicitly ignorable future types.

Installed pi-ai 0.87.1 openai-completions normalizes input to
max(0, prompt − cacheRead − cacheWrite), includes reasoning in output and calculates
total itself. Only when the record's own replayState.response explicitly identifies this
API/version 2 and positive input was not restricted to zero may native cache be added
back to recover positive total input. Other protocols do not inherit this rule.
Zero components remain unknown; calculated total is not an independent source total.
Model comes from that message.source; absent values cannot use mutable global routing.

File and decompressed output each have a 64 MiB limit; zstd window=64 MiB, line=4 MiB,
rows=50,000. Only a complete valid snapshot advances the byte cursor. Oversized/incomplete
writes retain the cursor and explicit diagnostics; never silently discard the tail.
Promote default DSH_HOME/manual roots to sessions. Within one directory select the highest
canonical generation version; reject future versions instead of falling back to older
files and hiding the upgrade. Repairing old registry-wide ownership needs a complete
native header and an old source with no detail/daily/period/source-aggregate history.
Transfer file and old checkpoint in one transaction. Retire the old empty instance only
when it has no other files; retain diagnostics and unrelated invalid user files. If a
snapshot deletes existing attempts, retain old values and report snapshot_regressed;
do not infer deletion intent or create offsetting events.
