# synthetic-version-fallback._expectations.md (SYNTHETIC)

<a id="synthetic-version-fallback_expectationsmdsynthetic"></a>

**All samples in this directory are synthetic, rather than extracted native sessions.**
This tests the V30 latest_fallback for an unregistered session version. The header has
version=4; subsequent entries match v3: id/parentId/timestamp plus assistant message.usage.

<a id="场景与期望"></a>

## Scenario and expectations

Three lines: session header (syn-sess-vf, version=4), assistant syn-vf-1 (10/5/0/0/15),
and assistant syn-vf-2 (20/10/0/0/30).

- detect → Supported(format_version="4", basis=latest_fallback); registration absence
  alone does not reject the file.
- Successful import: two primary events with parse_basis=latest_fallback,
  schema_version="4" (source declaration), and session_id=syn-sess-vf.
- Summary (2026-01-05): call_count=2, input_total_known=30 (10+20), cache_read_known=0,
  cache_write_known=0, output_total_known=15 (5+10), total_tokens_known=45 (15+30).
- File state active_compat; format_status JSON has basis=latest_fallback,
  found_version="4", compat=unverified. One latest_fallback diagnostic on version
  makes the compatibility attempt visible.
- Both usage records have all five fields and consistent totalTokens: no usage_shape_deviation.
- Repeated scans stop on unchanged content with added=0. Compatibility markers do not
  cause duplicate imports.
