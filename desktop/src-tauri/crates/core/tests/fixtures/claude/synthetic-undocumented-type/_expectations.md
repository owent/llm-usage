# synthetic-undocumented-type._expectations.md (synthetic)

<a id="synthetic-undocumented-type_expectationsmdsynthetic"></a>

**All files in this directory are synthetic test data, not extracts from real sessions.** An undocumented record type, file-history-snapshot, triggers V17 rejection
of the whole file. Clear the current scan's events and do not advance its cursor.
Detection checks only the first line: type=user returns Supported; rejection occurs
during scanning. Do not infer an unknown format.

<a id="场景与期望人工核算"></a>

## Scenario and manually calculated expectations

Three lines:

- Line 1: valid user, detect Supported.
- Line 2: assistant syn-req-would has usage 10/5/0/0 and would produce one event,
  which is cleared when the file is rejected.
- Line 3: type="file-history-snapshot" triggers undocumented_record_type.

Expected results:

- Direct detect: Supported(format=claude-transcript-jsonl, version=transcript-doc-1).
- files[0]: status=pending, lines_read=3, records_seen=3, events=0.
- No usage_events or ingestion_checkpoints rows.
- One undocumented_record_type diagnostic, field=type, position="line 3";
  source_files.status="degraded".
