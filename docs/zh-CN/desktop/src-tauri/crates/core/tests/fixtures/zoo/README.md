# Zoo Code fixtures

<a id="zoo-code-test-samples"></a>

<a id="zoo-code-合成-fixtures文档级证据待真实样本"></a>

`synthetic-*` 保留文档级回归；`real-3.86.0` 来自官方 VSIX 的真实 VS Code
扩展/公开 API/本地模型，原生输入 6,118、输出 53 与 API/扩展回调一致。
已删除正文、原始身份和配置，仅保留允许字段；固定分发物及源文件摘要见 provenance。
当前完整消息枚举和默认零规则来自 3.86.0 提交 `6aa9d0174a9ecae155c6c5db9134bead4b67197d`。

<a id="源码级证据a19固定-commit-f7806475331fcae5f4e8b5558d04415eeb5da88c"></a>

<a id="source-references-a19-fixed-commit-f7806475331fcae5f4e8b5558d04415eeb5da88c"></a>

## 源码依据（A19，固定 commit `f7806475331fcae5f4e8b5558d04415eeb5da88c`）

- usage 记录与合并语义（api_req_started text 五字段 + cost、condense_context
  的 contextCondense.cost、tokensIn 含缓存、contextTokens=in+out）：
  <https://github.com/Zoo-Code-Org/Zoo-Code/blob/f7806475331fcae5f4e8b5558d04415eeb5da88c/packages/core/src/message-utils/consolidateTokenUsage.ts>
- api_req_started/finished LIFO 配对合并（finish 覆盖 start；无配对 finished 丢弃）：
  <https://github.com/Zoo-Code-Org/Zoo-Code/blob/f7806475331fcae5f4e8b5558d04415eeb5da88c/packages/core/src/message-utils/consolidateApiRequests.ts>
- 任务文件形状（整写 JSON 数组 ui_messages.json）：
  <https://github.com/Zoo-Code-Org/Zoo-Code/blob/f7806475331fcae5f4e8b5558d04415eeb5da88c/packages/core/src/task-persistence/taskMessages.ts>、
  <https://github.com/Zoo-Code-Org/Zoo-Code/blob/f7806475331fcae5f4e8b5558d04415eeb5da88c/src/shared/globalFileNames.ts>
- 宿主路径（CLI 缺省 ~/.vscode-mock/global-storage；VS Code 扩展身份
  ZooCodeOrganization.zoo-code）：
  <https://github.com/Zoo-Code-Org/Zoo-Code/blob/f7806475331fcae5f4e8b5558d04415eeb5da88c/apps/cli/src/lib/task-history/index.ts>、
  <https://github.com/Zoo-Code-Org/Zoo-Code/blob/f7806475331fcae5f4e8b5558d04415eeb5da88c/src/package.json>

<a id="scenarios"></a>

## 场景

| 目录 | 覆盖 |
| --- | --- |
| synthetic-contract | 正常：两对 started/finished 合并 + condense_context 辅助 + 未配对 started 不入账 |
| synthetic-undocumented-say | future_undocumented_kind ⇒ 扫描层 fail closed 整文件拒绝；已证实 text 不再拒绝 |
| synthetic-not-array | 顶层非 JSON 数组 ⇒ detect 直接 UnknownFormat |
| real-3.86.0 | 内联 started、text/reasoning/resume 消息；默认零未知，不补缓存/模型/费用 |

期望值见各目录 `_expectations.md`（人工核算）。
旧摘要/未变化游标、并行/回滚和冲突保留见 `zoo_real_contract.rs`。
仅核验本次扩展 OpenAI-compatible 场景，CLI/其他协议/压缩仍待真实验收。
