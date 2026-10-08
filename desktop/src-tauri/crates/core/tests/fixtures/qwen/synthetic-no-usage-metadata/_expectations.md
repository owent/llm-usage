# synthetic-no-usage-metadata expectations (manually calculated, synthetic)

<a id="synthetic-no-usage-metadata-期望人工核算全合成样本"></a>

An assistant without usageMetadata is valid: the fixed source's tokens argument is
optional. It produces no event and **no diagnostic**; remaining records import normally.

<a id="期望"></a>

## Expectations

- files[0].status="complete", one event (syn-a-1), records_seen=3, diagnostics=0.
- Summary: call_count=1, input_total_known=100, output_total_known=20, total_tokens_known=120.
