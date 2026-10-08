# synthetic-contract expectations (manually calculated, synthetic)

<a id="synthetic-contract-期望人工核算全合成样本"></a>

All files are **synthetic** test data. Local inspection returned not_found; there was
no real sample. All IDs have the syn- prefix. Layout:
`<root>/tmp/<project_hash>/chats/session-*.json`, whole-file JSON rather than JSONL.

<a id="内容"></a>

## Contents

- Four messages: one user without tokens, two gemini with tokens, one gemini without tokens.
- syn-msg-1: tokens {input 1000, output 50, cached 400, thoughts 10, tool 5, total 1050}.
- syn-msg-2: tokens {input 2000, output 100, total 2100}; cached/thoughts/tool are absent
  and independently optional.
- syn-msg-3: no tokens, so no event.

<a id="期望"></a>

## Expectations

- Two events, syn-msg-1 and syn-msg-2; records_seen=4.
- Mapping: input_total=tokens.input (reported), total_tokens=tokens.total (reported),
  cached→input_cache_read (reported). thoughts/tool map to no normalized field;
  output_reasoning and input_uncached remain NULL. source_total is unknown.
- 2026-01-05 summary: call_count=2, input_total_known=3000, output_total_known=150,
  cache_read_known=400, total_tokens_known=3150, cache_write_known=None because this
  format has no cache-write field. Unknown is not replaced with zero.
- provider_id="google", session_id="syn-sess-g1", model_raw="gemini-3.0-flash",
  schema_version="session-doc-1", call_category="primary", agent="gemini-cli".
- Event keys: gemini:syn-sess-g1:syn-msg-1 and gemini:syn-sess-g1:syn-msg-2.

Calculation: compare all six token keys per message, then sum:
1050+2100=3150, 1000+2000=3000, 50+100=150. Only syn-msg-1 reports cached=400.
