# synthetic-negative-usage expectations (manually calculated, synthetic)

<a id="synthetic-negative-usage-期望人工核算全合成样本"></a>

syn-a-bad has usageMetadata.promptTokenCount=-100, outside the allowed range.
Skip it with usage_shape_deviation, **without** whole-file rejection; import other records normally.

<a id="期望"></a>

## Expectations

- One event, syn-a-ok: input_total=1000, output_total=50, cache_read=400, total_tokens=1050;
  files[0].status="complete", source health=degraded.
- One usage_shape_deviation diagnostic; no qwen:syn-a-bad event.
- Summary: call_count=1, input_total_known=1000, output_total_known=50,
  cache_read_known=400, total_tokens_known=1050.
