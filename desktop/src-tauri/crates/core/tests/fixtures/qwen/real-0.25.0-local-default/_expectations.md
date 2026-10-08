# Qwen Code 0.25.0 with default automatic memory

<a id="qwen-code-0250默认自动记忆"></a>

On 2026-10-05, ran the official npm client in rootless Podman against local llama.cpp
and Qwen2.5-0.5B GGUF. JSONL contains selected native assistant fields, not the complete
transcript: anonymized IDs, same-day shifted timestamps, unchanged token values, and no
bodies/project paths/other non-usage fields. Version, image/model digests and independent
counts are in [container acceptance](../../../../../../../../docs/validation/desktop-usage/container-sources.md).

- One main-loop call: prompt=10,226, output=2, cache read=0, total=10,228.
- CLI stats.bySource separately reports one automatic-memory extraction, total=5,854,
  absent from the session's per-call records. CLI totals of two calls/16,082 do not become
  this sample's count/total; do not subtract to invent a background event.
- Retain schema_version=0.25.0 and raw local model name. canonical/provider remain unknown.
  Missing cache write/reasoning relationships remain unknown; thoughts=0 does not justify values.
- Rescan adds/updates nothing; still one call and 10,228 tokens.
