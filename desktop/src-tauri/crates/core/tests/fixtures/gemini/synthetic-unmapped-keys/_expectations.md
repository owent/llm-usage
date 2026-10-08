# synthetic-unmapped-keys expectations (manually calculated, synthetic)

<a id="synthetic-unmapped-keys-期望人工核算全合成样本"></a>

Each of two gemini messages has a token key beyond the six documented keys:
surpriseKey or anotherUnknown. Preserve mapped fields and import the events.
Record unmapped_usage_keys once per scan.

<a id="期望"></a>

## Expectations

- Two events. Summary: call_count=2, input_total_known=300, output_total_known=50,
  total_tokens_known=350, cache_read_known=None. Neither message reports cached;
  unknown is not replaced with zero.
- Exactly one unmapped_usage_keys diagnostic; the second extra-key message adds none.
