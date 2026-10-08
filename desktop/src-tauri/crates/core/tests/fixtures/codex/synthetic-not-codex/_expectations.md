# synthetic-not-codex._expectations.md (synthetic)

<a id="synthetic-not-codex_expectationsmdsynthetic"></a>

**All files in this directory are synthetic test data, not extracts from real sessions.** V17 requires rejection of unknown formats, with an explicit status instead
of a successful empty result.

<a id="场景与期望"></a>

## Scenario and expectations

- The first line is not session_meta; it resembles a Kimi wire usage.record.
- detect=UnknownFormat; file scan status=unknown_format; events=0;
  diagnostics contains unknown_format; source_files.status=unsupported.
