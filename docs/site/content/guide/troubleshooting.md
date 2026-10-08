---
title: Troubleshooting
description: Diagnose empty totals, stale collection, parser limits and background failures.
sidebar:
  order: 9
---

## No usage or incomplete values

Check the selected user and date range, including the statistical timezone. Inspect Sources
for enablement, discovered paths, actual nonempty files/databases, record versions and diagnostics.
An empty session proves no token format, and a known model does not supply missing usage.

If a client version is newer than the verified matrix, compatibility parsing can preserve
valid observations with an unverified-version note. Preserve the exact version and file/database
shape when reporting the problem; do not label the whole product unsupported from one
missing field.

## Today's Codex usage is missing

Historical backfill can require multiple bounded windows. The current Codex reader gives
unvisited files priority and rotates confirmed visits across scans, preserving unfinished
cursors so that old large files do not indefinitely delay today's files. Refresh, inspect
progress and use the corrected executable; do not clear the database.

The [recovery record](/reference/evidence/codex-today-recovery/) documents the actual local
old-database repair, independent record reconciliation and the remaining backfill boundary.

## Visual Studio Enterprise or VS 2022 is empty

The adapter does not restrict Community or a year-specific install directory. Check the
process/user `TMP` and `TEMP` trace locations and use the read-only `vswhere`-based tool in
the [client guide](/guide/clients/). TEMP traces can be cleaned before collection.

The investigated VS 2022/older extension artifacts do not contain the verified VS 18
JSONL exporter. That result requires a different verified local file/database format, not guessed Enterprise
or `2022` path patterns. Include exact installation/product/component versions and whether
trace files exist, without sharing conversations or credentials.

## Values look duplicated or change after an upgrade

Do not add native and supplemental OTel totals manually. The application uses verified
contribution scopes and preserves native history. A parser correction reevaluates consumed
cursors and compares complete old event summaries; other content changes still undergo
normal conflict handling. Inspect conflicts, field quality and source revisions.

## Collection is stale or a task does not run

Check automatic interval, independent source rules, battery-saver pause and cancellation.
Manual refresh remains available when automatic collection is off. On Windows compare the
saved intent with the actual task definition and executable path. Sleep/logout, an exited
process and a hidden window are different states.

## Report a useful issue

Include application version, OS, exact client version, file/database format, selected range,
expected versus observed values, source status and a minimal redacted record containing
only relevant usage fields. Keep the first failure and later retry results separate.
Exclude secrets, full chat logs and account reports. Use
[GitHub issues](https://github.com/owent/llm-usage/issues).
