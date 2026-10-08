# synthetic-schema-deviation expectations (manually calculated, synthetic)

<a id="synthetic-schema-deviation-期望人工核算全合成样本"></a>

A top-level shape differing from the documented schema rejects the whole file,
with session_schema_deviation:

- session-sid-not-string.json: sessionId is not a string. Detection checks key names
  only; scanning validates types.
- session-msgs-not-array.json: messages is not an array.

<a id="期望"></a>

## Expectations

- Both files have files[*].status="pending"; no imported events or cursor advancement.
- Two diagnostics, code=session_schema_deviation, one per file.
