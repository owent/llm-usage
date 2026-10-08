# Claude Code 2.1.197 native usage sample

<a id="claude-code-21197-真实用量提取字段"></a>

<a id="claude-code-21197-真实用量白名单"></a>

Collected on 2026-10-07 in Debian/rootless Podman through Zhipu Coding Plan's
Anthropic-compatible endpoint. HTTP/SSE usage, CLI output and native JSONL were compared
field by field for two requests. Message bodies were removed and stable IDs hashed;
provenance retains the original file SHA-256. Each JSONL has six lines and two assistant
records sharing message.id. requestId is absent. Deduplicate by message ID, counting
one call rather than one call per content block.

| Model | Deduplicated calls | Uncached input | Output |
| --- | --- | --- | --- |
| glm-5.3-flash | 1 | 1,339 | 33 |
| glm-5.3 | 1 | 1,338 | 23 |

The provider did not report cache creation, but the client still wrote zero. Native zero
fields have no validity marker, so cache read/write, complete input/total and reasoning
remain unknown. Do not infer provider/cost from protocol or model name; CLI cost is an
estimate. Each event has version=2.1.197 and known_version. Other versions use compatibility
reading; this sample does not verify historical records.

See the [container validation record](../../../../../../../../docs/validation/desktop-usage/claude-container-sample.md).
