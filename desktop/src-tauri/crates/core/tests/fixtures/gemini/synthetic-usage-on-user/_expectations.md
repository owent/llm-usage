# synthetic-usage-on-user expectations (manually calculated, synthetic)

<a id="synthetic-usage-on-user-期望人工核算全合成样本"></a>

A user message contains a tokens object, although the documented format permits
tokens only on gemini messages. Reject the whole file with usage_on_unexpected_message_type.

<a id="期望"></a>

## Expectations

- files[0].status="pending"; no imported events or cursor advancement; no checkpoint.
- One usage_on_unexpected_message_type diagnostic per scan. A second scan is still
  pending and adds another diagnostic, confirming repeated rejection rather than a successful empty result.
