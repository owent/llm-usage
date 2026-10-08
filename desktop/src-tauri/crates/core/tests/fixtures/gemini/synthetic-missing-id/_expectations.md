# synthetic-missing-id expectations (manually calculated, synthetic)

<a id="synthetic-missing-id-期望人工核算全合成样本"></a>

messages[1] is a gemini message with tokens but no id. Fall back to the array index,
`gemini:<sessionId>:idx-<index>`, under the append-only assumption, and record missing_message_id.

<a id="期望"></a>

## Expectations

- One event; source_record_key="gemini:syn-sess-noid:idx-1"; origin_call_id=NULL.
- One diagnostic, code=missing_message_id.
- Summary: call_count=1, input_total_known=100, output_total_known=20, total_tokens_known=120.
