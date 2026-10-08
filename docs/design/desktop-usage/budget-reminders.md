# Usage and cost alerts

<a id="budget-reminders"></a>

<a id="用量和费用提醒"></a>

<a id="预算提醒"></a>

Reminders default to off. The user chooses a local day or month, known total tokens or
observation-time estimated cost, and either a positive integer token threshold or a monetary
threshold with at most six decimal places. Monetary values use the existing micro-units and
one currency; no monetary reminder is evaluated while cost estimation is disabled. Current
API references and actual bills are kept separate. Reminders appear only in the application
and do not stop an agent or collection.

Queries are scoped to the current statistical user and follow the saved timezone, retention
range and exclusive choice of details versus archives. Unknown fields are not filled with
zero. Restricted source health, unknown usage or partial price coverage produce a coverage
note. A known lower bound reaching the threshold can still trigger a reminder; lack of a
reminder does not establish that complete usage stayed below the threshold.

A reminder's identity consists of user, timezone, local period, metric, currency and threshold
and is persisted in the application database. Each identity is displayed once; restarts,
backfills, repeated reads and multiple windows do not produce another popup. A new period or
an explicit threshold change can create a new reminder. Disabling reminders keeps historical
identities, and clearing usage does not repeat an already shown threshold for the same period.
Settings displays the current value and coverage as observed when the panel loads.

Validation covers default-off behavior, unknown usage, timezone/DST/month transitions,
single-currency amounts, threshold boundaries, duplicates/restarts, configuration changes,
user isolation, transaction failure, native IPC and UI presentation.
