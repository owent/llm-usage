# synthetic-no-usage._expectations.md (synthetic)

<a id="synthetic-no-usage_expectationsmdsynthetic"></a>

Wire data contains no usage.record: an error step followed by a retry, both matching
observed native shapes without usage. Expected: status=complete, no events or diagnostics.
Absent usage is valid; neither fill zeros nor report an error.
