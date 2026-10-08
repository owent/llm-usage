# OpenClaw test samples

<a id="openclaw-fixtures"></a>

<a id="openclaw-合成-fixtures文档级证据待真实样本"></a>

synthetic-* retains original synthetic data with syn- prefixes and constant placeholders.
It tests rejection of unknown schemas and legacy migration input. real-2026.9.8 comes
from the official npm client on 2026-10-06 in an isolated container with networking
disabled: two real local-model calls through public CLI/session continuation, extracted
with selected fields and anonymization.

<a id="官方文档依据a092026-09-24-核验"></a>

<a id="官方文档级证据a092026-09-24-核验"></a>

## Official documentation (A09, checked 2026-09-24)

- Disk location and persistence: one openclaw-agent.sqlite per Agent; legacy sessions/
  and sessions.json are migration/archive inputs and are not imported at Gateway startup:
  <https://docs.openclaw.ai/reference/session-management-compaction/store>
- Assistant transcript entries persist normalized usage, normalize input/output aliases,
  fall back to input+output for total, and include usage.cost:
  <https://docs.openclaw.ai/reference/token-use>

Earlier synthetic table names/entry fields were **invented placeholders**
(`synthetic-table-*` and `syn-*`). They test discovery shape and rejection, without
representing the real schema.

<a id="场景"></a>

## Scenarios

| Directory | Coverage |
| --- | --- |
| synthetic-runtime-store | agents/main/agent/openclaw-agent.sqlite with placeholder tables; product identified, table schema unverified, reject |
| synthetic-legacy-archive | legacy agents/main/sessions/ JSONL archives plus sessions.json; migration input remains excluded |
| real-2026.9.8 | Selected exact schema_meta/session_windows/transcript_events fields and independent API usage; uncached input 6,157, output 154, cache read 6,101; default zeros unknown |

Expectations are in each directory's _expectations.md and real-2026.9.8/provenance.json.
Native extraction removes prompts, responses, configuration and private paths; stable
placeholders replace identities. Rebuilt SQLite contains only required structures.
Invalid-type, compression and external-source variants are targeted synthetic cases,
without verification of those native product scenarios. Complete originals stay only
in root build/. Schema, timestamps and default-zero semantics are in the
[reading specification](../../../../../../../docs/design/desktop-usage/openclaw-runtime.md).
