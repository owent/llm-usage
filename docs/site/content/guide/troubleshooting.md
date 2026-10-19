---
title: Troubleshooting
description: Diagnose empty totals, stale collection, parser limits and background failures.
sidebar:
  order: 9
---

## No usage or incomplete values

Check the selected user and date range, including the statistical timezone. Inspect Sources
for enablement, discovered paths, actual nonempty files/databases, record versions and diagnostics.
An empty session has no usage to collect, and a known model cannot supply missing token fields.

Known components still appear when the source cannot provide complete totals. A value marked
≥ is a lower bound; hover over a table value for coverage details. Charts and shares include
this known usage, and shares describe only the displayed amounts. Missing components are
never filled with zero. OpenCode v2 default-zero cache buckets can cause this limited coverage
even when its completed calls have been collected successfully.

OpenCode being open source does not mean every response field is saved locally. In the
checked v2.0.32 path, missing values and reported zeros become identical stored zeros, and
original usage may be discarded before the session is saved. This affects field coverage,
not call recognition. Positive visible output can recover inclusive output independently of
missing input details. See the [OpenCode field rules](/reference/design/opencode-runtime/).
Rescanning cannot restore detail that the source never saved; keep the existing statistics.

If a client version is newer than the verified matrix, compatibility parsing can preserve
valid observations and internal version references. A complete, valid expected shape produces no
compatibility warning; actual parsing and coverage errors remain visible. Preserve the exact version and file/database
shape when reporting the problem; do not label the whole product unsupported from one
missing field.

## Today's Codex usage is missing

Update to the [latest Release](https://github.com/owent/llm-usage/releases/latest), refresh
and inspect collection progress. Large histories may take several scans to backfill;
the reader prioritizes files it has not visited so today's records can be collected.
Keep the existing database. See the [recovery details](/reference/evidence/codex-today-recovery/)
if the problem persists.

## Visual Studio Enterprise or VS 2022 is empty

The adapter does not restrict Community or a year-specific install directory. Check the
process/user `TMP` and `TEMP` trace locations and use the read-only `vswhere`-based tool in
the [client guide](/guide/clients/). TEMP traces can be cleaned before collection.

The investigated VS 2022/older extension artifacts do not contain the verified VS 18
JSONL exporter. That result requires a different verified local file/database format, not guessed Enterprise
or `2022` path patterns. Include exact installation/product/component versions and whether
trace files exist, without sharing conversations or credentials.

## Values look duplicated or change after an upgrade

Do not add native and supplemental OTel totals manually. The application selects eligible
contributions and preserves native history. Upgrades can reread existing records to correct
parsing without clearing the database. Inspect conflicts, field quality and source revisions
if values still differ; conflicting content remains in the history.

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
