# synthetic-negative-tokens expectations (manually calculated, synthetic)

<a id="synthetic-negative-tokens-期望人工核算全合成样本"></a>

syn-msg-bad has tokens.input=-100, outside the permitted range. This shape deviation
does **not** reject the whole file. Skip that message, record usage_shape_deviation,
and import the remaining messages normally.

<a id="期望"></a>

## Expectations

- One event, syn-msg-ok: input_total=1000, output_total=50, cache_read=400,
  total_tokens=1050; files[0].status="complete", source health=degraded.
- One diagnostic, code=usage_shape_deviation.
- Summary: call_count=1, input_total_known=1000, output_total_known=50,
  cache_read_known=400, total_tokens_known=1050.
