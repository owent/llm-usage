# synthetic-unknown-version._expectations.md (synthetic)

<a id="synthetic-unknown-version_expectationsmdsynthetic"></a>

protocol_version="1.5" is verified in kimi-code, while kimi-work verifies only 1.4.
The same file must use latest_fallback in kimi-work; A12/A13 registries are independent.
Expect one latest_fallback diagnostic and one event with parse_basis=latest_fallback.
Usage {100,50,400,0} yields input_total=500 and total_tokens=550; count no duplicate repeat.
