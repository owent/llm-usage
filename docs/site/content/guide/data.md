---
title: Retention, export and merge
description: Keep useful history, exchange normalized records and recover safely.
sidebar:
  order: 7
---

## Retention tiers

Settings exposes separate retention windows for events, hourly, daily, weekly, monthly
and yearly statistics. Inspect the current counts and cleanup preview before applying
destructive retention or **Clear all data**. Cleanup affects the application database,
not the agent's original files.

Archived totals and retained details are chosen by complete source partition, not added
together. Removing detailed records does not authorize inventing missing token fields,
models or individual calls from a cumulative snapshot. Later rescans respect the saved
retention floor instead of silently rebuilding deliberately removed history.

## Two exchange formats

| Format | What it preserves |
| --- | --- |
| Aggregate exchange | Supported summary partitions and their provenance; it does not reconstruct per-request records. |
| `llm-usage-details-1` | Complete normalized events, field quality, source/host identity, revisions, conflicts, cumulative snapshots and sealed partitions. |

Export in Settings and choose the destination through the system dialog. Imports detect
the format and show a preview before executing a transaction. Detail packages are bounded
to 64 MiB, consistently for import and export. A details package is not a database backup:
it does not contain collection cursors, all system settings or the complete price cache.

Repeated imports skip duplicates. When both records have source revision numbers, compare
those numbers; otherwise compare lifecycle order (`partial < final < corrected`). A higher
rank replaces the existing record, a lower rank keeps it, and equal ranks with different
content retain the existing record and a conflict history. Missing records in an import
do not delete existing ones. An invalid field or identity conflict rolls back the complete
batch rather than committing a partial merge.

## Identity and privacy

Stable original source keys remain intact, including original host identity. The same path
on a different host is not the same source. Imported sources start disabled and assigned
to the default local user. Importing remote records does not make them current local usage.

Exports use statistical fields rather than prompts, responses, credentials or tool output.
Review metadata before sharing: stable source identities can still identify a machine or
local source. See [privacy](/guide/privacy/) and the
[detail merge rules](/reference/design/detail-merge/).
