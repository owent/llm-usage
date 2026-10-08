# synthetic-unknown-version._expectations.md (SYNTHETIC)

<a id="synthetic-unknown-version_expectationsmdsynthetic"></a>

**All samples in this directory are synthetic, rather than extracted native sessions.**
This covers the V30 unknown-session-version policy: unregistered numeric versions use
the latest built-in parser (latest_fallback). Reject only shapes shown incompatible by
fixed-version source. Data follows Pi session JSONL; fixed source has
CURRENT_SESSION_VERSION=3, while v1 has no version field.

<a id="场景与期望"></a>

## Scenario and expectations

- `..._syn-sess-v4.jsonl`: the first session header has version=4 (unregistered);
  subsequent entries have v3 structure.
  detect → Supported(format_version="4", basis=latest_fallback).
- `..._syn-sess-legacy.jsonl`: no version in the first session header (v1 shape).
  Fixed source confirms v1/v2 omitted this field and stored no id/parentId.
  detect → UnsupportedVersion(found=None, with legacy in the reason).

<a id="期望v30-新语义"></a>

## Expectations under the new V30 behavior

- v4: scan imports one syn-v4-1 event with 1/1/0/0/2, parse_basis=latest_fallback,
  schema_version="4" and file state active_compat. One latest_fallback diagnostic;
  repeat scanning adds nothing. See the dedicated synthetic-version-fallback case.
- Legacy: verified incompatible; skip the entire file, import zero events,
  status=unsupported_version and one unsupported_version diagnostic.
- Explicit rejection produces a diagnostic rather than silently reporting success with zero records.
