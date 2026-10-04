# OpenCode 合成 fixtures（基于源码、待真实样本）

<a id="opencode-合成-fixtures文档级证据待真实样本"></a>

全部场景为**合成数据**（syn- 前缀、常量占位），不是任何真实会话脱敏产物。
本机 2026-09-25 盘点未安装 OpenCode（not_found，m0-agent-fixtures.md），
按用户指示以固定版本源码依据实现，真实数据验收后置。

<a id="源码级证据a17固定-commit-0027387dc5c59793c12dfc531abc78f825ed6868"></a>

## 源码依据（A17，固定 commit `0027387dc5c59793c12dfc531abc78f825ed6868`）

- session/message/part 三表 DDL（tokens_* 累计列、parent_id、data JSON）：
  <https://github.com/anomalyco/opencode/blob/0027387dc5c59793c12dfc531abc78f825ed6868/packages/core/src/session/sql.ts>
- step-finish usage 提取规则（`usage()`：type=step-finish 且 cost+tokens 在场）
  与 session 计数维护（`applyUsage` 增量 + 删行补偿）：
  <https://github.com/anomalyco/opencode/blob/0027387dc5c59793c12dfc531abc78f825ed6868/packages/core/src/session/projector.ts>
- 迁移 json_extract 口径（message.data.$.tokens.* → session 累计列回填）：
  <https://github.com/anomalyco/opencode/blob/0027387dc5c59793c12dfc531abc78f825ed6868/packages/core/src/database/migration/20260510033149_session_usage.ts>
- step-finish tokens 语义（input=nonCachedInputTokens、output=visibleOutputTokens）：
  <https://github.com/anomalyco/opencode/blob/0027387dc5c59793c12dfc531abc78f825ed6868/packages/core/src/session/runner/publish-llm-event.ts>
- inclusive inputTokens = nonCached+cacheRead+cacheWrite 与 totalTokens 政策：
  <https://github.com/anomalyco/opencode/blob/0027387dc5c59793c12dfc531abc78f825ed6868/packages/llm/src/protocols/anthropic-messages.ts>、
  <https://github.com/anomalyco/opencode/blob/0027387dc5c59793c12dfc531abc78f825ed6868/packages/llm/src/protocols/shared.ts>
- 库布局（xdg 数据目录 + opencode.db/通道变体）：
  <https://github.com/anomalyco/opencode/blob/0027387dc5c59793c12dfc531abc78f825ed6868/packages/core/src/global.ts>、
  <https://github.com/anomalyco/opencode/blob/0027387dc5c59793c12dfc531abc78f825ed6868/packages/core/src/database/database.ts>

DDL 取固定源码逐字列名子集（适配器读取的列 + 累计五列）；FK 约束与索引
不影响只读解析，未在合成库重建。

## 场景

| 目录 | 覆盖 |
| --- | --- |
| synthetic-step-finish | 正常：主/子会话 × 3 条 step-finish 部件逐次入账 + text 部件过滤 + 会话累计对账 matched + 直报 total 对照 |
| synthetic-unknown-version | 未收录 session.version ⇒ latest_fallback 兼容尝试（数据照常入库） |

期望值见各目录 `_expectations.md`（人工核算）。
未知格式 fail closed（非 SQLite/缺表）与产品互斥（MiMo 库拒绝）用例在
`tests/opencode_contract.rs` 内联构造，不占 fixture 目录。
取得真实脱敏 fixture 后逐版本升级 known_version 并替换合成样本。
