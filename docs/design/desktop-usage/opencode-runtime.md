# OpenCode v2 local projections

Version 0.3.3 adds a reader for `session_v2` and `session_message`, independently of the
v1 `session/message/part` reader. Local Windows 2.0.32 records were checked against official
v2.0.32 [schemas](https://github.com/anomalyco/opencode/blob/v2.0.32/packages/schema/src/session-message.ts),
[usage normalization](https://github.com/anomalyco/opencode/blob/v2.0.32/packages/core/src/session/usage.ts),
[message updates](https://github.com/anomalyco/opencode/blob/v2.0.32/packages/core/src/session/message-updater.ts),
[projection/fork writes](https://github.com/anomalyco/opencode/blob/v2.0.32/packages/core/src/session/projector.ts)
and [compaction accumulation](https://github.com/anomalyco/opencode/blob/v2.0.32/packages/core/src/session/compaction.ts).
The [0.3.3 record](../../validation/desktop-usage/release-033.md) separates native samples from
synthetic boundaries and Provider/API acceptance.

## Ownership and lifecycle

Verify the product root, SQLite schema and owning `session_v2` row. Retain that session's
version on each message; an installed or maximum database version cannot identify other rows.
Do not infer a session's historical messages from its current model or cumulative totals.
Read only usage, model, identity and time metadata from JSON; bodies never leave SQLite.

| Projection | Contribution |
| --- | --- |
| Completed assistant (`data.time.completed`) | One observed model call, including failed calls without returned usage. |
| Running assistant | No settled contribution yet; reread its stable ID later. |
| Completed/failed compaction | Usage observation. Upstream adds multiple attempts into its projection; do not invent a per-call count. |
| User/system/synthetic/skill/shell/idle and known switch messages | No model usage contribution. |
| Unknown message kind or malformed required metadata | Visible coverage error; preserve other valid messages. |

Fork projection writes copy the settled prefix with new IDs and the original sequence numbers.
Resolve `fork_session_id` and `fork_boundary.messageID` to the original sequence. Exclude
sequences before that boundary, including the boundary for `type=after`. Missing boundary
evidence is an ownership error; copied history must not become new calls.

## Fields

| Native field | Mapping and limits |
| --- | --- |
| `tokens.input` | Uncached input. |
| `tokens.cache.read/write` | Independent cache buckets. |
| `tokens.output` / `tokens.reasoning` | Visible and reasoning output are exclusive. Positive visible output proves the upstream subtraction was not clamped; their sum recovers inclusive output. Zero visible output leaves the inclusive value unknown; retain positive reasoning separately. |
| Zero token components | Unknown: upstream normalization defaults missing/invalid values to zero. No validity marker establishes reported zero. |
| Complete totals | Derive input total only with all three input buckets; derive overall total from known input and recovered inclusive output. Missing reasoning decomposition alone does not invalidate recovered output. Never expose a partial sum as the complete total. |
| `model.id/providerID` | Message's own request identity; compaction may lack it. No inference from model families or protocols. |
| `cost` | Positive client estimate in USD; default zero is unknown, not a bill or free-use proof. |
| `time.completed/created` | Native completion, otherwise compaction start. Positive difference supplies duration; no TTFT inference. |

Reject negative, noninteger and overflowing token shapes. A confirmed completed call with invalid
usage still counts once with unknown tokens and a visible diagnostic. Native session totals and
durable event tables are never added to message contributions. Two nonempty legacy/v2 layouts
in one database remain unverified migration overlap, rather than being combined automatically.

## Missing detail and recoverable output

Open source makes the mapping inspectable; it does not guarantee that local projections
retain original responses. At v2.0.32 the inspected write path is:

1. [OpenAI Chat mapping](https://github.com/anomalyco/opencode/blob/v2.0.32/packages/ai/src/protocols/openai-chat.ts#L913-L935)
   retains inclusive totals and raw usage on `Usage.providerMetadata`. Its finish call passes
   `usage`, without copying that nested metadata to the event's `providerMetadata`.
2. [Lifecycle.finish](https://github.com/anomalyco/opencode/blob/v2.0.32/packages/ai/src/protocols/utils/lifecycle.ts#L98-L116)
   and [LLMEvent.stepFinish](https://github.com/anomalyco/opencode/blob/v2.0.32/packages/ai/src/schema/events.ts#L370-L374)
   keep usage and event metadata separate.
3. [publish-llm-event](https://github.com/anomalyco/opencode/blob/v2.0.32/packages/core/src/session/runner/publish-llm-event.ts#L578-L585)
   saves five normalized buckets and selects provider state only from event metadata.
   Original inclusive input/total and nested raw usage are not copied into this projection.
   `SessionUsage.tokens` replaces absent/invalid components with zero.

The read-only Windows sample has no `providerState` or metadata on any of its seven assistant
messages, and its `event` table is empty. All seven cache-write values are zero; one cache-read
and one reasoning value are zero. These explain incomplete input/detail coverage, not
unrecognized calls or a malformed database. Provider/model names and current configuration
cannot establish each historical request's protocol or original field presence. Do not use
another protocol's defaults or OpenCode's displayed sum to manufacture complete input.
Restoring detail requires the source to retain inclusive totals and field-presence evidence;
rescanning unchanged rows cannot recover it.

The actual [Usage getter](https://github.com/anomalyco/opencode/blob/v2.0.32/packages/ai/src/schema/events.ts#L77-L85)
computes `max(0, (outputTokens ?? 0) - (reasoningTokens ?? 0))`. For valid nonnegative usage,
positive visible output lets the reader invert this subtraction, even with no reasoning
split. Zero visible output cannot exclude absent totals or clamping, so positive reasoning
alone is not complete output. This does not assert that missing reasoning was reported zero.
All seven native rows have positive visible output and recover **7,888 output tokens**;
input and overall usage remain lower bounds.

## Rescans and health

Use stable `opencode:v2:<message-id>` identities and `time_updated` source revisions. Read the
projection in bounded ID pages, at most 50,000 rows or the configured lower budget. After the
last page, begin another cycle so earlier mutable IDs are revisited without assuming unique
timestamps. Large sources require multiple scans; changes behind the position appear next cycle.
The cursor and contributions commit together. Keep previous errors through partial cycles;
replace them only when every row has been revisited. A successful empty page cannot erase errors.

The adapter parser upgrade reevaluates old unsupported registrations through normal discovery,
including parent-directory manual roots. Keep v1 and other products' readers separate. Valid
compatible records retain internal version references without a source warning. Real malformed
records, unknown ownership and unsupported layouts remain visible; no database clearing is needed.

Parser `opencode-session-messages-2` and dispatch 4 replay consumed positions. Same-revision
output corrections reconstruct the complete parser-1 event digest, allowing only verified
output/total coverage changes and parser references. Retain first observation, all diagnostic
history and real conflicts, including earlier conflicts in the same batch. Corrections,
summary recomputation and checkpoints roll back together on failure.
