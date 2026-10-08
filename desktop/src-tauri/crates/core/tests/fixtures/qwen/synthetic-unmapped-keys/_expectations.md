# synthetic-unmapped-keys expectations (manually calculated, synthetic)

<a id="synthetic-unmapped-keys-期望人工核算全合成样本"></a>

Two assistants have an extra usageMetadata key beyond the six fixed categories:
serviceTier or anotherUnknown. Import mapped fields. Record unmapped_usage_keys once
per file, with the flag persisted in parsing context across scans.

<a id="期望"></a>

## Expectations

- Two events. Summary: call_count=2, input_total_known=300, output_total_known=50,
  total_tokens_known=350, cache_read_known=None; neither record directly reports cache read.
  Unknown remains unknown.
- Exactly one unmapped_usage_keys diagnostic.
