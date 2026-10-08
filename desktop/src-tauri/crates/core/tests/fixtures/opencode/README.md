# OpenCode test samples

<a id="opencode-测试样本"></a>

<a id="opencode-合成-fixtures文档级证据待真实样本"></a>

synthetic-* uses syn- IDs and constant placeholders. These cases were created after the
2026-09-25 inventory found no installed OpenCode (not_found, m0-agent-fixtures.md), using
fixed-version source per user instruction. They remain separate from the anonymized
native 1.18.34 default/explicit-title samples added on 2026-10-05; see the
[container record](../../../../../../../docs/validation/desktop-usage/container-sources.md).

<a id="源码依据a17固定-commit-0027387dc5c59793c12dfc531abc78f825ed6868"></a>

<a id="源码级证据a17固定-commit-0027387dc5c59793c12dfc531abc78f825ed6868"></a>

## Source references (A17, fixed commit 0027387dc5c59793c12dfc531abc78f825ed6868)

- session/message/part DDL: cumulative tokens_* columns, parent_id and data JSON:
  <https://github.com/anomalyco/opencode/blob/0027387dc5c59793c12dfc531abc78f825ed6868/packages/core/src/session/sql.ts>
- step-finish usage(): type=step-finish with cost/tokens; session applyUsage updates
  incremental counters and compensates for deleted rows:
  <https://github.com/anomalyco/opencode/blob/0027387dc5c59793c12dfc531abc78f825ed6868/packages/core/src/session/projector.ts>
- json_extract migration from message.data.$.tokens.* into session cumulative columns:
  <https://github.com/anomalyco/opencode/blob/0027387dc5c59793c12dfc531abc78f825ed6868/packages/core/src/database/migration/20260510033149_session_usage.ts>
- step-finish input=nonCachedInputTokens, output=visibleOutputTokens:
  <https://github.com/anomalyco/opencode/blob/0027387dc5c59793c12dfc531abc78f825ed6868/packages/core/src/session/runner/publish-llm-event.ts>
- Inclusive inputTokens=nonCached+cacheRead+cacheWrite and totalTokens rules:
  <https://github.com/anomalyco/opencode/blob/0027387dc5c59793c12dfc531abc78f825ed6868/packages/llm/src/protocols/anthropic-messages.ts>,
  <https://github.com/anomalyco/opencode/blob/0027387dc5c59793c12dfc531abc78f825ed6868/packages/llm/src/protocols/shared.ts>
- XDG data directory, opencode.db and channel variants:
  <https://github.com/anomalyco/opencode/blob/0027387dc5c59793c12dfc531abc78f825ed6868/packages/core/src/global.ts>,
  <https://github.com/anomalyco/opencode/blob/0027387dc5c59793c12dfc531abc78f825ed6868/packages/core/src/database/database.ts>

Synthetic DDL retains exact source column names for adapter-read fields and five
cumulative columns. Foreign keys/indexes do not affect read-only parsing and are not rebuilt.

<a id="场景"></a>

## Scenarios

| Directory | Coverage |
| --- | --- |
| synthetic-step-finish | Main/child sessions, three individual step-finish parts, filtered text, matched session totals and reported-total comparison |
| synthetic-unknown-version | Unregistered session.version attempts latest_fallback and imports valid data |
| real-1.18.34-local-default | Native main-loop total 299; separate title API total 549 is absent from persisted usage |
| real-1.18.34-local-controlled | Explicit run --title avoids title generation; one API/part/event with total 299 |

Each directory's _expectations.md contains manual calculations. Unknown-format rejection
(non-SQLite/missing tables) and product separation (rejecting MiMo databases) are inline
in tests/opencode_contract.rs. Native samples remain latest_fallback: per-record version
verification and old-cursor upgrade acceptance are unfinished. A native sample does not
automatically register known_version or verify other sessions/versions; retain independent
synthetic boundary tests.
