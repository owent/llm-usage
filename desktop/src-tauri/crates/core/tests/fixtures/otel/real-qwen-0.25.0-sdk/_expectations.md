# Qwen Code 0.25.0 SDK file: anonymized native sample

<a id="qwen-code-0250-sdk-file-真实脱敏样本"></a>

The official CLI called a fixed local llama.cpp/Qwen2.5 model in Podman with networking
disabled. [provenance.json](provenance.json) records source and version; original SHA-256
matches the [container source record](../../../../../../../../docs/validation/desktop-usage/container-sources.md).
Retain SDK `_rawAttributes` arrays, `_spanContext`, timestamps, kind and permitted attributes.
Replace session and trace/span IDs consistently; exclude prompts, response bodies, URLs,
accounts and host information. Metrics retain only structure needed to distinguish them
from calls. The file contains 25 consecutive multiline JSON objects, rather than JSONL.

| Source | Count | Input (including cache) | Output | Cache read | thoughts | Total |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| interaction main loop | 1 | 10,226 | 2 | 0 | 0 | 10,228 |
| standalone managed-auto-memory-extractor | 1 | 5,639 | 556 | 3 | 0 | 6,195 |
| All CLI requests | 2 | 15,865 | 558 | 3 | 0 | 16,423 |

[native.jsonl](native.jsonl) retains the first user record's shape from this same native
session, without message.parts contents, and its only assistant usage record: the
main-loop 10,228 tokens. The first record tests manual-root routing.
[cli-stats.json](cli-stats.json) contains permitted final CLI statistics. The API-response
log and llm_request span observe the same request: select the span. Do not add HTTP,
interaction or metrics values. Background spans have no parentSpanContext; a shared
session.id does not establish a parent/child relationship.

Unreturned cache write/provider remain unknown. thoughts=0 does not verify inclusion
rules for positive reasoning or other providers/versions. Select one native/exported
representation per verified host/user/session/local-day partition. Do not infer call IDs
from equal timestamps or tokens. Coverage before export was enabled is limited. Boundary,
mixed-version, error and rollback cases are synthetic variants, without native SDK verification.
