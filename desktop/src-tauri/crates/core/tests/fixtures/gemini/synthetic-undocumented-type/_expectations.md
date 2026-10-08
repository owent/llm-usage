# synthetic-undocumented-type expectations (manually calculated, synthetic)

<a id="synthetic-undocumented-type-期望人工核算全合成样本"></a>

messages[1] has type="info", an existing but undocumented message type. Reject the
whole file with undocumented_message_type; do not guess its format.

<a id="期望"></a>

## Expectations

- files[0].status="pending"; no imported events. Clear events from messages visited
  before rejection, and do not advance the cursor.
- One diagnostic, code=undocumented_message_type.
