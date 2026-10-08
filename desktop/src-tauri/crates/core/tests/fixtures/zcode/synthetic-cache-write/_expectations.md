# synthetic-cache-write expectations (synthetic)

<a id="synthetic-cache-write-期望全合成"></a>

cacheWriteTokens=200>0 covers a gap in native samples, where all values were zero. Anthropic
cache_creation_input_tokens=200 is also present. Both representations agree: 1000+800+200=2000.

See the manually calculated expectations in the header of tests/zcode_gaps_synthetic.rs.
