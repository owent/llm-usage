# synthetic-dual-mismatch expectations (synthetic)

<a id="synthetic-dual-mismatch-期望全合成"></a>

AI SDK inputTokens=1000 includes cache read, while Anthropic input_tokens=999 plus cache_read=400
gives 1399. Record dual_caliber_mismatch and retain the primary AI SDK values.

See the manually calculated expectations in the header of tests/zcode_gaps_synthetic.rs.
