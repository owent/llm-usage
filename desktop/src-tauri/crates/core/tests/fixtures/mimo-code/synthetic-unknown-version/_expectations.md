# synthetic-unknown-version expectations (synthetic)

<a id="synthetic-unknown-version-期望全合成"></a>

Unregistered session.version="9.9.9-syn-uncollected" uses latest_fallback under V17/V30,
retaining compatibility markers instead of rejecting the version alone.

Manually calculated expectations:

- detect=Supported, format_version="9.9.9-syn-uncollected", basis=latest_fallback;
  format_status has compat=unverified.
- One latest_fallback diagnostic.
- One imported usage_events record, mimo-code:part:part_syn_future_1:
  input_total=9500, output_total=250, total_tokens=9750, source_total=9750.
  2026-06-13 summary: call_count=1, input_total=9500, total_tokens=9750.
- reconciliations is empty.
