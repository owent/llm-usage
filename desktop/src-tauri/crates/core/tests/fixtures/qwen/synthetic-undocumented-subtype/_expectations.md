# synthetic-undocumented-subtype expectations (manually calculated, synthetic)

<a id="synthetic-undocumented-subtype-期望人工核算全合成样本"></a>

Assistant line 2 has subtype="brand_new_subtype", outside the fixed source's 33-value enum. Reject the whole file with undocumented_record_subtype.

<a id="期望"></a>

## Expectations

- files[0].status="pending"; no imported events or cursor advancement; no checkpoint.
- One diagnostic. Diagnostic code=undocumented_record_subtype.
