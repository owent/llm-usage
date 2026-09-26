# Zoo Code 合成 fixtures（文档级证据、待真实样本）

全部场景为**合成数据**（syn- 前缀、常量占位），不是任何真实会话脱敏产物。
本机 2026-09-25 盘点未安装 Zoo Code（not_found，m0-agent-fixtures.md），
按用户指示以固定源码证据实现，真实数据验收后置。

## 源码级证据（A19，固定 commit `f7806475331fcae5f4e8b5558d04415eeb5da88c`）

- usage 载体与合并语义（api_req_started text 五字段 + cost、condense_context
  的 contextCondense.cost、tokensIn 含缓存、contextTokens=in+out）：
  <https://github.com/Zoo-Code-Org/Zoo-Code/blob/f7806475331fcae5f4e8b5558d04415eeb5da88c/packages/core/src/message-utils/consolidateTokenUsage.ts>
- api_req_started/finished LIFO 配对合并（finish 覆盖 start；无配对丢弃）：
  <https://github.com/Zoo-Code-Org/Zoo-Code/blob/f7806475331fcae5f4e8b5558d04415eeb5da88c/packages/core/src/message-utils/consolidateApiRequests.ts>
- 任务文件形状（整写 JSON 数组 ui_messages.json）：
  <https://github.com/Zoo-Code-Org/Zoo-Code/blob/f7806475331fcae5f4e8b5558d04415eeb5da88c/packages/core/src/task-persistence/taskMessages.ts>、
  <https://github.com/Zoo-Code-Org/Zoo-Code/blob/f7806475331fcae5f4e8b5558d04415eeb5da88c/src/shared/globalFileNames.ts>
- 宿主路径（CLI 缺省 ~/.vscode-mock/global-storage；VS Code 扩展身份
  ZooCodeOrganization.zoo-code）：
  <https://github.com/Zoo-Code-Org/Zoo-Code/blob/f7806475331fcae5f4e8b5558d04415eeb5da88c/apps/cli/src/lib/task-history/index.ts>、
  <https://github.com/Zoo-Code-Org/Zoo-Code/blob/f7806475331fcae5f4e8b5558d04415eeb5da88c/src/package.json>

## 场景

| 目录 | 覆盖 |
| --- | --- |
| synthetic-contract | 正常：两对 started/finished 合并 + condense_context 辅助 + 未配对 started 不入账 |
| synthetic-undocumented-say | 未文档化 say 种类（text）⇒ 扫描层 fail closed 整文件拒绝 |
| synthetic-not-array | 顶层非 JSON 数组 ⇒ detect 直接 UnknownFormat |

期望值见各目录 `_expectations.md`（人工核算）。
取得真实脱敏 fixture 后按实际消息形状扩展文档化集合并升为已验证。
