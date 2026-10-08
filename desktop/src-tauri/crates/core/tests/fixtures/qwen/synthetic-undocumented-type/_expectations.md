# synthetic-undocumented-type expectations (manually calculated, synthetic)

<a id="synthetic-undocumented-type-期望人工核算全合成样本"></a>

Line 2 has type="brand_new_record", outside the four-value ChatRecord enum in fixed source. Clear
the assistant event parsed from line 1. Reject the whole file with undocumented_record_type.

<a id="期望"></a>

## Expectations

- files[0].status="pending"; no imported events or cursor advancement; no checkpoint.
- One diagnostic per scan; a second scan remains pending and adds another diagnostic. Diagnostic code=undocumented_record_type.
