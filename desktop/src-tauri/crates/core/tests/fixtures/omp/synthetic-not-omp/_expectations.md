# synthetic-not-omp._expectations.md (synthetic)

<a id="synthetic-not-omp_expectationsmdsynthetic"></a>

**All files are synthetic, not native session extracts.** V17 rejects unknown formats
instead of reporting successful empty output.

<a id="场景与期望"></a>

## Scenario and expectations

- First line type="event_msg" is neither an OMP title nor a session header. A valid
  session v3 header on line 2 cannot override first-line detection.
- detect=UnknownFormat{reason: `first record type "event_msg" is neither title nor session`}.
- Run: status="unknown_format", no events, one unknown_format diagnostic,
  empty usage_events, source_files.status="unsupported".
