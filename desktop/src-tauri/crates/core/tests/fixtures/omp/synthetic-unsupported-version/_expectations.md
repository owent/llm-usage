# synthetic-unsupported-version._expectations.md (SYNTHETIC)

<a id="synthetic-unsupported-version_expectationsmdsynthetic"></a>

**All samples in this directory are synthetic, rather than extracted native sessions.**
The historical unsupported-version directory name is retained from V17 rejection tests.
From V30, OMP unknown versions attempt latest_fallback; this document and assertions
describe that newer behavior.

<a id="场景与期望v30未收录缺失-version-都回退最新内置解析器尝试"></a>

## Scenario and expectations (V30: unregistered or absent version attempts the latest parser)

Older OMP disk formats are unverified, without established incompatibility. Pi v1/v2,
by contrast, were proven incompatible by fixed source and remain rejected. For OMP,
both unregistered numeric versions and absent version use LatestFallback with session_v3.
V30 scanning determines structural incompatibility: records read, zero events and
structural diagnostics retain previous results.

- `...d020.jsonl`, three lines: session version=4 (only 3 locally verified).
  detect returns Supported{format=omp-session-jsonl, format_version=Some("4"),
  basis=latest_fallback}.
- `...d021.jsonl`, three lines: session has no version; legacy-shaped entry id=syn-l1
  differs from d020's syn-m1. V17 used syn-m1 in both because no events were imported;
  compatibility fallback would create a false same-key/different-content conflict, so
  these now have distinct IDs. detect returns Supported{format_version=None, basis=latest_fallback}.
- Both scan successfully: file status="complete", one event each with 100/10/0/0/110.
  Detail includes `latest_fallback: version compatibility unverified (found: 4|missing)`.
  Two usage_events have parse_basis=latest_fallback, schema_version="4" and "unknown".
  Both source_files have status="active_compat" and format_status basis=latest_fallback,
  compat=unverified, found_version="4" and null. Two latest_fallback diagnostics, one per file.
- Summary (2026-01-05 UTC): call_count=2, input_total=200
  (derived input+cacheRead+cacheWrite, 100 each), output=20, total_tokens=220.
- Repeat scanning adds nothing after the cursor is consumed: added=0.
