# OpenCode 1.18.34 default-title scenario

<a id="opencode-11834-默认标题场景"></a>

On 2026-10-05, the official CLI at source revision
aec0b9a6d8898f68f923aaf08b7306d931fd9d76 called a real local CPU model in isolated rootless
Podman with networking disabled and fresh HOME. Tokens are native; the
[container record](../../../../../../../../docs/validation/desktop-usage/container-sources.md)
describes the procedure and source. Extraction retains the original three-table DDL and
permitted usage fields. Bodies are removed; IDs, paths and titles are anonymized.
Timestamps share one offset into 2026-10-05 UTC; tokens are unchanged. Message and session
cumulative totals are comparison values and are never added to individual usage.

| Measurement | Calls / usage |
| --- | --- |
| Native main-loop API usage | input 298 (including cache read 3), output 1, total 299 |
| CLI / part / assistant message / session | uncached input 295, cache read 3, write 0, output 1, reasoning 0, total 299 |
| Application | one event; input_total 298, output_total 1, total 299; repeat scan changes no events, revisions or summaries |
| Default-title API usage | another call: input 539, output 10, total 549; absent from step-finish and session cumulative totals |

The two default API calls total 848. A matched comparison between main-loop events and
cumulative totals does not establish complete client coverage. Do not invent a title
event from the difference or infer a cloud model or price from the local model name.
The client wrote cost=0; no actual bill or positive cost was verified. Version handling
remains latest_fallback. Per-record version verification and old-cursor upgrade testing
are unfinished, so this sample cannot verify other sessions in the same database.
