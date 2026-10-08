# synthetic-bad-shapes._expectations.md (synthetic)

<a id="synthetic-bad-shapes_expectationsmdsynthetic"></a>

Use the same rules as kimi-code. One valid turn {200,60,800,10} imports with
input_total=1010 and total=1070. Negative usage produces usage_shape_deviation;
usageScope="lifetime" produces usage_scope_unknown; seconds-like time=1767225600
is skipped with timestamp_unparseable, without guessing by multiplying by 1000.
Expected: one event, three diagnostics, degraded.
