# Zoo Code test samples

<a id="zoo-code-fixtures"></a>

<a id="zoo-code-合成-fixtures文档级证据待真实样本"></a>

synthetic-* retains documentation-based regressions. real-3.86.0 comes from the official
VSIX running as an actual VS Code extension through its public API with a local model.
Native input=6,118 and output=53 match API and extension callback. Extraction removes
bodies, original identities and configuration; provenance records distribution/source
digests. Complete message enums/default-zero rules come from 3.86.0 commit
6aa9d0174a9ecae155c6c5db9134bead4b67197d.

<a id="源码依据a19固定-commit-f7806475331fcae5f4e8b5558d04415eeb5da88c"></a>

<a id="源码级证据a19固定-commit-f7806475331fcae5f4e8b5558d04415eeb5da88c"></a>

## Source references (A19, fixed commit f7806475331fcae5f4e8b5558d04415eeb5da88c)

- Usage fields and merging: api_req_started text's five fields plus cost,
  condense_context's contextCondense.cost, tokensIn includes cache, contextTokens=in+out:
  <https://github.com/Zoo-Code-Org/Zoo-Code/blob/f7806475331fcae5f4e8b5558d04415eeb5da88c/packages/core/src/message-utils/consolidateTokenUsage.ts>
- api_req_started/finished LIFO pairing: finish overwrites start; discard unmatched finish:
  <https://github.com/Zoo-Code-Org/Zoo-Code/blob/f7806475331fcae5f4e8b5558d04415eeb5da88c/packages/core/src/message-utils/consolidateApiRequests.ts>
- Task format: whole-file JSON array in ui_messages.json:
  <https://github.com/Zoo-Code-Org/Zoo-Code/blob/f7806475331fcae5f4e8b5558d04415eeb5da88c/packages/core/src/task-persistence/taskMessages.ts>,
  <https://github.com/Zoo-Code-Org/Zoo-Code/blob/f7806475331fcae5f4e8b5558d04415eeb5da88c/src/shared/globalFileNames.ts>
- Host paths: CLI defaults to ~/.vscode-mock/global-storage; extension ID ZooCodeOrganization.zoo-code:
  <https://github.com/Zoo-Code-Org/Zoo-Code/blob/f7806475331fcae5f4e8b5558d04415eeb5da88c/apps/cli/src/lib/task-history/index.ts>,
  <https://github.com/Zoo-Code-Org/Zoo-Code/blob/f7806475331fcae5f4e8b5558d04415eeb5da88c/src/package.json>

<a id="场景"></a>

## Scenarios

| Directory | Coverage |
| --- | --- |
| synthetic-contract | Two merged started/finished pairs, auxiliary condense_context, unmatched started excluded |
| synthetic-undocumented-say | future_undocumented_kind rejects the whole file during scanning; verified text is accepted |
| synthetic-not-array | Non-array top level gives detect=UnknownFormat |
| real-3.86.0 | Inline started, text/reasoning/resume messages; default zeros unknown; no invented cache/model/cost |

Each directory's _expectations.md has manually calculated values. Complete old summaries,
unchanged cursors, parallelism, rollback and conflict retention are in zoo_real_contract.rs.
Only this extension's OpenAI-compatible scenario is verified; CLI, other protocols and
compression await native acceptance.
