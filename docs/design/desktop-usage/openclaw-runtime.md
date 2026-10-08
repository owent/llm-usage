# Reading local OpenClaw schema 24

<a id="openclaw-schema-24-本地读取说明"></a>

This specification uses installed npm openclaw@2026.9.8 code, the fixed official schema
and this batch's nonempty native local-model sample. Public npm provenance commit aa6008ad,
CLI-displayed fc23bc8 and actual compiled code were checked separately. contextUsage was
added in the provenance commit but is absent from installed normalization code; do not
invent historical fields from that commit. Source and execution details are in the
batch's validation record; [Plan.md](../../../Plan.md) maintains status.

Each agents/Agent/agent/openclaw-agent.sqlite is a separate instance; exclude the global
state database. Detection checks user_version=24, the main schema_meta row's
role=agent/schema_version/agent_id, and required tables/columns. Mutable database-wide
schema_meta.app_version does not verify historical records' client versions. Latest
compatibility reading remains latest_fallback; unknown schemas are rejected. Legacy
sessions JSON/JSONL remains migration input and is excluded. Leave sources unchanged
and run no source maintenance/migration commands.

Read transcript_events bodies individually as TEXT or zstd, decoding read-only.
Both original and decoded bodies have a 4 MiB limit and must match event_utf8_bytes.
Navigation indexes are not usage bodies. Retain diagnostics for oversized/invalid rows
and continue valid rows. Reading, schema/ownership checks and archive checks share one
read-only SQLite snapshot. Scan at most 50,000 rows per run, paging by session_id/seq;
after the final page, recheck mutable history from the beginning. WAL data cannot be
skipped solely on byte offsets. Events and continuation position commit in the unified
batch transaction; rollback advances neither.

Include only saved local OpenClaw sessions with session_entry_provenance=1, acp_owned=0,
and no plugin_owner_id or hook_external_content_source. Missing/external provenance
stays isolated. Each assistant message must use the verified openai-completions API;
other protocols do not inherit its token rules. Identity uses session_id plus entry id.
Other messages, system/custom records and snapshots add no duplicate usage. Use only
each record's provider/model, rather than the session's latest model. Call category and
underlying call count remain unknown until all attempt attribution is verified;
store usage_observation.

Installed code's input is uncached input after subtracting cache read/write; output is
total output. Missing cacheRead/cacheWrite defaults to zero. Retain valid positive
values; zeros/missing values stay unknown. Do not derive complete input/total from
calculated totalTokens or default cache values, or infer a bill from default cost=0.
Invalid types, negatives, limits and contradictions stay visible; valid other fields
can still be saved. Use actual timestamp field semantics rather than database-write
time as an invented request completion time.

Do not add active transcripts to archives. No nonempty native archive sample is available;
when archives exist, explicitly report limited coverage and retain saved history. An
empty active table does not establish complete history. Gateway, auxiliary/nested/
imported/legacy cases and other transports need separate verification; two local CLI
calls do not expand that scope.
