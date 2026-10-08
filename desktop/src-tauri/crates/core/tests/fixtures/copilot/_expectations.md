# Copilot CLI assistant_usage_events native sample expectations

<a id="copilot-cli-assistant_usage_events-真实脱敏样本预期值"></a>

Read-only extraction from local `~/.copilot/session-store.db`, schema_version=8:
36 rows from 2026-08-03T14:25:56Z–14:36:22Z, one session and model claude-opus-4.8,
request_multiplier always 27.0. session_id/agent_id use anonymized syn-N replacements;
all native agent_id values were NULL. Only allowlisted columns were extracted,
excluding body columns such as token_details_json/api_endpoint.

Manual totals over all 36 rows:

| Field | Value |
| --- | --- |
| input_tokens | 4,649,981 |
| output_tokens | 34,157 |
| cache_read_tokens | 4,416,791 |
| cache_write_tokens | 233,118 |
| reasoning_tokens | 17,678 |
| input_uncached (derived: input−read−write) | 72 |
| total_tokens (derived: input+output) | 4,684,138 |

All 36 rows satisfy input>=cache_read+cache_write, with input=uncached+read+write.
This native sample confirms the M0 m0-agent-fixtures.md finding.
request_multiplier=27.0 is a premium billing multiplier and contributes no tokens.
