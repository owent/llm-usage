# synthetic-usage-on-tool-result expectations (manually calculated, synthetic)

<a id="synthetic-usage-on-tool-result-期望人工核算全合成样本"></a>

tool_result line 3 contains usageMetadata, permitted only on assistant records. Clear the assistant
event already parsed from line 2. Reject the whole file with usage_on_unexpected_record_type.

<a id="期望"></a>

## Expectations

- files[0].status="pending"; no imported events or cursor advancement; no checkpoint.
- One diagnostic per scan; a second scan remains pending. Diagnostic code=usage_on_unexpected_record_type.
