# synthetic-usage-on-user._expectations.md (synthetic)

<a id="synthetic-usage-on-user_expectationsmdsynthetic"></a>

**All files in this directory are synthetic test data, not extracts from real sessions.** A user record contains usage although this record type has no usage fields
in the specified format. V17 rejects the whole file, clears current events and does not
commit a checkpoint. The next scan rejects it again; do not infer another format.

<a id="场景与期望人工核算"></a>

## Scenario and manually calculated expectations

Three lines:

- Line 1: valid user, used for detection; first-line type=user returns Supported.
- Line 2: assistant syn-req-would has usage 10/5/0/0 and would produce one event,
  which is cleared on rejection.
- Line 3: user has a top-level usage key, triggering usage_on_unexpected_record_type
  and whole-file rejection.

Each scan must produce:

- files[0]: status=pending (ScanStatus::Pending), lines_read=3, records_seen=3, events=0.
- Zero usage_events and ingestion_checkpoints rows; the cursor never advances.
- One new usage_on_unexpected_record_type diagnostic, field=type, position="line 3";
  source_files.status="degraded".
- A second scan remains pending with zero events and one further diagnostic (two total).
