# Faster daily queries

<a id="日查询加速"></a>

Common daily queries use derived summaries for totals, models and Agents, partitioned
by source, plus daily duration and session membership grouped by source/session.
Queries with no dimension filter or only a provider filter use the appropriate derived
tables; instance allowlists still apply in SQL. Provider session tables are split by
provider. Merge source/session identities before DISTINCT, so one session using several
providers still counts once. Model/Agent filters keep the existing query path and indexes
matching the existing fold_name/model_key expressions. Other queries use the full path.
Across days, merge source/session identities; unknown sessions stay unknown, and daily
DISTINCT counts must not simply be added.

These optional schema-11 tables leave events, original summaries, revisions and source
cursors unchanged. A writer opening an old database rebuilds missing or invalid daily
partitions. Collection and daily recalculation update them in the same transaction.
SQLite AFTER triggers invalidate the affected day on insert/update/delete in original
tables and remove its derived rows and session identities. Read-only queries use the
accelerated tables only when each requested day is valid, or when both original daily
and event tables establish that a missing day is empty. Otherwise they read original
tables. Older applications' writes also invalidate the derived data; rollback reverts
both invalidation and derived rows. Deleted structures can be regenerated and are not
the primary data for exchange or archives. Clearing statistics removes all derived data.
Initial repair handles at most the latest 750 daily partitions; older history retains
the original query path. Integer SUM overflow skips only that day's optional derived
build, without rejecting events whose separate model/source totals are valid. Normal
queries retain exact integer checks and never switch money to floating point.

The derived layout has its own version. Layout or name-normalization changes require
invalidation, rebuilding and updated expression indexes; old results cannot verify new
rules. Opening a writer runs bounded `PRAGMA optimize` to update query-planner statistics.
Combined filters choose indexes by actual selectivity; see
[SQLite PRAGMA optimize](https://www.sqlite.org/pragma.html#pragma_optimize) for its behavior and scope.

Measure old-database rebuilding, complete imports with current indexes, queries without
a cached connection, incremental refresh and all native processes separately. Include
derived storage and write costs. Compare filtering, aliases, unknown values, DST,
archives, instance isolation, external writes and rollback against independent expected
results for the original query path.

Trigger behavior: [SQLite CREATE TRIGGER](https://www.sqlite.org/lang_createtrigger.html).
Integer summation: [SQLite aggregates](https://www.sqlite.org/lang_aggfunc.html).
Both page bodies were checked on 2026-10-04.
