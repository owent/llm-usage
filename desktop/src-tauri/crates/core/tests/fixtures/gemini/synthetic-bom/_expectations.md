# synthetic-bom expectations (manually calculated, synthetic)

<a id="synthetic-bom-期望人工核算全合成样本"></a>

A session JSON file begins with a UTF-8 BOM. Both detection and whole-file parsing
remove the BOM and import normally.

<a id="期望"></a>

## Expectations

- detect=Supported; files[0].status="complete"; one event, syn-msg-1.
- Summary: call_count=1, input_total_known=100, output_total_known=20,
  total_tokens_known=120; no diagnostics.
