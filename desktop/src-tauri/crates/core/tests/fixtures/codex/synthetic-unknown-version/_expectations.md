# synthetic-unknown-version._expectations.md (synthetic)

<a id="synthetic-unknown-version_expectationsmdsynthetic"></a>

**All files in this directory are synthetic test data, not extracts from real sessions.** Under V17, unknown versions use the latest built-in parser by default.
Validated data is imported with compatibility markers. An unregistered version alone
does not cause rejection; an unknown format still does.

<a id="场景与期望"></a>

## Scenario and expectations

- session_meta has `cli_version="0.999.0-synthetic"`, absent from the verified version
  registry. Its shape matches verified records: session_meta has the session ID and
  token_usage_record has all six usage fields.
- detect returns `Supported { format_version: Some("0.999.0-synthetic"), basis: LatestFallback }`.
  File scan status=complete, events=1. One latest_fallback diagnostic says
  "using latest built-in parser; version compatibility unverified".
  source_files.status=active_compat; format_status JSON contains
  { basis: latest_fallback, found_version: 0.999.0-synthetic, compat: unverified };
  usage_events.parse_basis=latest_fallback.
- One call, syn-resp-uv-1: input=10, output=5, total=15.
- Rescanning adds nothing: rescan_added=0.
- parse_basis/compat distinguish this from normal parsing of a verified version;
  diagnostics/status also distinguish compatibility reading from a successful empty scan.
