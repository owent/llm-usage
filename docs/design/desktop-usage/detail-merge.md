# Detail exchange and merge

<a id="明细交换与合并"></a>

Detail exchange uses the independent version `llm-usage-details-1`. It preserves complete
normalized events, per-field quality, original host/source identity, source revisions,
conflict flags and independent cumulative snapshots. Aggregate exchange remains compatible.
Settings offers separate aggregate/detail exports; import detects the package format and
previews scope/counts before execution. Exports contain only the statistical allowlist,
excluding conversation bodies, credentials, additional original-file path fields and collection
cursors. Stable source keys remain unchanged. Both import and export use a 64 MiB limit so
an exported package is not too large to import again.

Detail packages include sealed day/hour/period partitions. Source days with retained details
are recomputed from events without adding exported summaries. Details and sealed snapshots
for the same source day cannot overwrite each other; incomplete details cannot unseal an old
snapshot. Cumulative snapshots retain exclusive/duplicate/overlap_unknown and are not expanded
into individual calls. Exports validate complete source days; a target source day containing
only aggregates cannot be partially reconstructed. Original event costs stay with details;
when estimation is enabled, affected unsealed days are recomputed in the same transaction.
Database backups preserve price snapshots, application cost caches and system configuration;
a details package is not a whole-database backup.

Import preserves source IDs; matching paths on different hosts do not merge. Sources default
to disabled and the default user. Explicit import does not change original attribution.
Per-request records retain source keys and source revisions. When both records have revision
numbers, compare those numbers; otherwise compare lifecycle order (`partial < final < corrected`).
A higher rank replaces the existing record, a lower rank keeps it, and equal ranks with different
content retain the existing record and conflict history. Identical records are skipped.
An absent package record has no deletion meaning.
Incomplete per-request exports from old formats can only be imported as aggregate packages;
field quality is not fabricated.

Source registration, events, cumulative data, archives and summaries commit in one SQLite
transaction. Any invalid field, attribution conflict or failure rolls back the entire batch.
The retention floor is respected, removed details are not restored, and duplicate imports do
not advance the usage revision. Processing positions/cursors are not imported; subsequent
local collection continues from its own positions.

Validation covers nonempty round trips, unknown fields, duration/cost/quality/model attribution,
cumulative data and archives, exclusive additions, source-revision replacement, conflicts,
repeat reads, cross-host rejection, retention floors, whole-batch rollback and real IPC.
