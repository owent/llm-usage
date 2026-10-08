# MiMo Code test samples

<a id="mimo-code-fixtures"></a>

<a id="mimo-code-合成-fixtures文档级证据待真实样本"></a>

synthetic-* is synthetic data. real-0.1.15 extracts selected SQLite fields after the
official client called a real local model. Eight parts match eight independent API usage
records individually; provenance records the original database digest. Retain only
placeholder identities, times, models, usage and required schema, excluding bodies,
configuration and original paths. All eight finish reasons are length; CLI exit 0 does
not establish task completion.

<a id="源码依据a14固定-commit-456678b6a5afb0eef3fe2754575637218cfb3c84"></a>

<a id="源码级证据a14固定-commit-456678b6a5afb0eef3fe2754575637218cfb3c84"></a>

## Source references (A14, fixed commit 456678b6a5afb0eef3fe2754575637218cfb3c84)

- session/message/part DDL: distinctive message.agent_id, no session tokens_* cumulative columns:
  <https://github.com/XiaomiMiMo/MiMo-Code/blob/456678b6a5afb0eef3fe2754575637218cfb3c84/packages/opencode/src/session/session.sql.ts>
- StepFinishPart zod schema: tokens{total?, input, output, reasoning, cache{read, write}}
  plus cost; Assistant requires modelID/providerID:
  <https://github.com/XiaomiMiMo/MiMo-Code/blob/456678b6a5afb0eef3fe2754575637218cfb3c84/packages/opencode/src/session/message-v2.ts>
- Path resolution: `MIMOCODE_HOME`→`<home>/data`; XDG default `~/.local/share/mimocode`:
  <https://github.com/XiaomiMiMo/MiMo-Code/blob/456678b6a5afb0eef3fe2754575637218cfb3c84/packages/shared/src/global.ts>
- Database: data/mimocode.db with channel variants:
  <https://github.com/XiaomiMiMo/MiMo-Code/blob/456678b6a5afb0eef3fe2754575637218cfb3c84/packages/opencode/src/storage/db.ts>

Current semantics follow official 0.1.15 commit 14dfe68a1c121f859544ba810b3c308e8501bfb2,
session/session.ts:getUsage. It stores SDK input after subtracting cache and output
after subtracting reasoning; adding those same components restores positive SDK totals.
Default-zero subfields and costs stay unknown. OpenCode ancestry or a database's highest
version does not verify individual rows; use latest_fallback.

<a id="场景"></a>

## Scenarios

| Directory | Coverage |
| --- | --- |
| synthetic-step-finish | Main/child sessions with three step-finish parts and a filtered text part; no cumulative columns, so no comparison |
| synthetic-unknown-version | Unregistered session.version attempts latest_fallback |
| real-0.1.15 | Eight CLI/session-continuation API/part calls; input 25,588, output 1,024, total 26,612, known cache read 20,733 |

Each directory's _expectations.md contains manually calculated values. Unknown-format
rejection and product separation (rejecting OpenCode databases) are constructed inline
in tests/mimo_code_contract.rs. Default zeros, complete old summaries, consumed positions
beyond samples, parallelism, rollback and conflict retention are covered by
mimo_real_contract.rs. A single native request path does not verify every protocol, role or version.
