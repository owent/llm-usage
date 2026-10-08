# subagent-community-research._expectations.md

Source: `<HOME>/.omp/agent/sessions/--D--Comfy-Desktop--/2026-08-21T02-19-22-638Z_<parentUUID>/CommunityResearch.jsonl`.
41 lines, oh-my-pi 18.2.7, 2026-08-21T02:35Z; extracted 2026-09-25.

<a id="结构期望"></a>

## Structural expectations

- All 41 lines parse: parseErrors=0.
- Types: title ×1, session ×1 (version=3, own session ID, no parentSession),
  session_init ×1, model_change ×1, thinking_level_change ×1, credential_pin ×1,
  custom ×15, message ×20 (one user, five assistant, 14 toolResult).
- The file lies under a `<ts>_<parentUUID>/` session directory; parent linkage comes
  from its UUID. The timestamp prefix matches parent-file creation time.
- All assistants use k3-256k/kimi-code, stopReason=toolUse, with duration/ttft and responseId.

<a id="usage-数值期望允许字段原字段jq-逐条求和"></a>

## Expected usage values (selected original fields, summed individually with jq)

Five sub_agent calls; parent_session_id is the parent UUID from the directory:

| # | input | output | cacheRead | cacheWrite | totalTokens | duration_ms (original) | ttft_ms (original) |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 8574 | 382 | 768 | 0 | 9724 | 17309.34289999993 | 6857.021999999997 |
| 2 | 3542 | 382 | 9216 | 0 | 13140 | 14649.890800000052 | 6983.533100000001 |
| 3 | 3432 | 303 | 12544 | 0 | 16279 | 8387.370200000005 | 1649.5432000000728 |
| 4 | 3467 | 275 | 15872 | 0 | 19614 | 14960.637299999944 | 7235.652699999977 |
| 5 | 1695 | 4793 | 19200 | 0 | 25688 | 126737.42190000007 | 9318.338600000017 |
| Σ | 20710 | 6135 | 57600 | 0 | 84445 | | |

- Round floating-point duration/ttft milliseconds before storage.
- Five sub_agent events: input_total=78310 (derived: 20710+57600+0),
  input_uncached=20710, cache_read=57600, output_total=6135, total_tokens=84445.

45 anonymized IDs: anon-1…anon-45.
