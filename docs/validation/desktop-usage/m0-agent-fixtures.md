# M0: Local agent versions and redacted test samples

<a id="m0本机-agent-版本记录与脱敏-fixture"></a>

<a id="元信息"></a>

## Run information

| Item | Value |
| --- | --- |
| Date | 2026-09-24 |
| Environment | Windows 11 Pro 26200 x64; Node v24.21.0; read-only extraction, no agent-source changes |
| Code revision | Uncommitted working tree |
| Design references | implementation-readiness.md, real-data validation during implementation; execution.md M0 item 5 |

<a id="范围与方法"></a>

## Scope and method

- Search only known candidate directories, not entire disks. Versions come from local
  packages/install metadata; agent entrypoints were not started.
- Retained fields: event kind, schema/version hints, stable anon-N IDs, time, model, token
  values and inclusion relations. Bodies become constants. SQLite queried read-only from
  staged copies, then copies removed.
- Ignored build/desktop-usage-validation/ holds `fixtures/<source>/`, each with manually
  checkable _expectations.md, agent-inventory.md and tools/ extraction/leak scanners.
- tools/leak-check.py scanned all 42 outputs for UUID/path/email/body/username leaks:
  zero. Two redaction defects were found/fixed before regenerating all samples.

<a id="本机产品矩阵实测"></a>

## Observed local products

| Product | Version and reference | Local format | Sample |
| --- | --- | --- | --- |
| Codex, VS Code extension host | CLI 0.155.0-alpha.16.3 from rollout session_meta; openai.chatgpt extension 26.917.62051 | rollout JSONL token_usage_record per call, token_count cumulative, turn_context; 6 SQLite files | extracted; 3 sessions, up to 49 calls/1,457 lines |
| New Kimi Code | Desktop 1.0.3; wire protocol_version=1.5 | `sessions/<wd>/session_*/agents/*/wire.jsonl`; camelCase usage.record inputOther/output/inputCacheRead/inputCacheCreation; epoch ms | extracted; main/subagent comparison |
| ZCode | 3.14.3, EXE 3.14.3.7762; protocol client 0.16.9/schemaVersion 1 | model-io JSONL AI SDK camelCase and Anthropic snake_case with opposite input definitions; db.sqlite model_usage/turn_usage | extracted; last 8 requests, daily log, two tables |
| Copilot CLI | 1.0.73 from events.jsonl copilotVersion | events.jsonl has no per-call tokens; session-store.db assistant_usage_events has per-turn fields | extracted |
| Kilo Code CLI | pnpm 7.4.21; session.version observed 7.4.15–7.4.20 | ~/.local/share/kilo/kilo.db, 1.6 GB, **not** ~/.config/kilo; OpenCode-derived SQLite, message.data.tokens | extracted schema/redacted rows |
| VS Code | code --version: 1.139.0 | copilot-chat session-store.db no token columns, recheck M5; kilocode.kilo-code extension 7.7.9 | schema_recorded |
| pi | Scoop 0.87.1 | Empty sessions | no_data |
| WorkBuddy | version file 37.10.3-24 | Three directories empty or version-only | no_data, remnants |
| Claude Code | No CLI install metadata found | ~/.claude lacks projects; sessions/backups empty | no_data |
| Gemini CLI | No install metadata found | ~/.gemini lacks chats | no_data |
| oh-my-pi | Scoop manifest 18.2.11 | ~/.omp/agent/{agent.db,history.db} exists | exists_only; this stage permits existence checks only |
| qwen/opencode/mimocode/openclaw/hermes/zoo/dsh | — | Candidate paths absent | not_found |

<a id="字段口径结论实读核验供-m1m2-解析器合同"></a>

<a id="字段口径结论实读核验供-m1m2-解析器规则"></a>

## Observed field meanings for M1/M2 parsers

- Codex: total=input+output, cached is an input subset, reasoning an output subset (98/98).
  Per-call sum equals final cumulative snapshot without compaction. **Compaction resets
  cumulative snapshots**: 32.06M sum differs from 31.59M snapshot. usage has no model;
  attribution requires turn_context.
- Kimi wire: four exclusive fields, no total. event.usage echoes usage.record; choose one
  to prevent double counting. subagent.completed.usage equals per-field child wire sums.
- ZCode: response.usage.inputTokens includes cache reads; anthropic.usage.input_tokens
  excludes them. turn_usage equals model_usage sums for 16 request rounds.
- Copilot: input=uncached+read+write; request_multiplier=27.0 is a paid multiplier.
- Kilo: input/output/reasoning/cache.read/cache.write are all exclusive, unlike the above.
  Codex threads.tokens_used also agrees with rollout per-call totals across stores.

<a id="失败与未执行项"></a>

## Failures and unexecuted checks

| Item | Status | Cause/next step |
| --- | --- | --- |
| Positive cache writes | Copilot only | Codex/Kimi/ZCode samples all zero; labeled synthetic cases in M2/M3 |
| Mid-session model changes | Codex only | Not observed in other samples |
| Reasoning | Kimi/ZCode samples lack values | Keep unknown, no zero filling |
| Active Kimi wire file | Point-in-time snapshot | Grew 394→515 lines during sampling; expectations state snapshot time |
| .omp DB contents, WSL/container instances, F1 products | Not inspected | Stage restrictions/out of scope |

<a id="验证产物"></a>

<a id="证据文件"></a>

## Validation files

Ignored build/desktop-usage-validation/: agent-inventory.md; fixtures/ for Codex, Kimi Code,
ZCode, Copilot CLI and Kilo; tools/extract-jsonl.mjs, sqlite-probe.py, kilo-sample.py and
leak-check.py. Review redaction before importing samples; CI uses reviewed samples only.
