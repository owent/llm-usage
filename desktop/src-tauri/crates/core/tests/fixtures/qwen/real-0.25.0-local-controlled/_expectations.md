# Qwen Code 0.25.0 with automatic memory disabled

<a id="qwen-code-0250关闭自动记忆的对照"></a>

Use the same official client/local model as the [default sample](../real-0.25.0-local-default/_expectations.md).
Only container configuration changes: memory.enableManagedAutoMemory=false and
enableManagedAutoDream=false. Verify keys/defaults in the installed schema/configuration
loader first. JSONL contains selected native fields, anonymized IDs and timestamps shifted
within the same day; token values remain unchanged.

- Model service, CLI stats, native ChatRecord and app SQLite agree: one call,
  prompt=8,903, output=2, cache read=0, total=8,905; tool calls=0.
  The default sample's coverage limits remain separately recorded.
- Retain version/raw model name; canonical/provider, cache write and reasoning stay unknown.
- Rescan adds/updates nothing; still one call and 8,905 tokens.
- Does not verify Qwen cloud, other versions, archives or cache-hit scenarios.
