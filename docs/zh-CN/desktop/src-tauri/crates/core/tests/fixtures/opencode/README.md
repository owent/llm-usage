# OpenCode 测试样本

<a id="opencode-test-samples"></a>

<a id="opencode-合成-fixtures文档级证据待真实样本"></a>

synthetic-* 为合成数据（syn- 前缀、常量占位）。2026-09-25 本机盘点未安装
OpenCode（not_found，m0-agent-fixtures.md），按用户指示依据固定版本源码建立这些用例。
2026-10-05 已另加入 1.18.34 默认标题和显式标题的真实脱敏样本，仍与合成测试分开；
执行过程见 [容器记录](../../../../../../../docs/validation/desktop-usage/container-sources.md)。

<a id="源码级证据a17固定-commit-0027387dc5c59793c12dfc531abc78f825ed6868"></a>

<a id="source-references-a17-fixed-commit-0027387dc5c59793c12dfc531abc78f825ed6868"></a>

## 源码依据（A17，固定 commit `0027387dc5c59793c12dfc531abc78f825ed6868`）

- session/message/part 三表 DDL（tokens_* 累计列、parent_id、data JSON）：
  <https://github.com/anomalyco/opencode/blob/0027387dc5c59793c12dfc531abc78f825ed6868/packages/core/src/session/sql.ts>
- step-finish usage 提取规则（`usage()`：type=step-finish 且 cost+tokens 在场）
  与 session 计数维护（`applyUsage` 增量 + 删行补偿）：
  <https://github.com/anomalyco/opencode/blob/0027387dc5c59793c12dfc531abc78f825ed6868/packages/core/src/session/projector.ts>
- 迁移 json_extract 计算规则（message.data.$.tokens.* → session 累计列回填）：
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

<a id="scenarios"></a>

## 场景

| 目录 | 覆盖 |
| --- | --- |
| synthetic-step-finish | 正常：主/子会话 × 3 条 step-finish 部件逐次入账 + text 部件过滤 + 会话累计对账 matched + 直报 total 对照 |
| synthetic-unknown-version | 未收录 session.version ⇒ latest_fallback 兼容尝试（数据照常入库） |
| real-1.18.34-local-default | 主循环总量 299；另有标题 API 总量 549，未保存到用量记录 |
| real-1.18.34-local-controlled | 显式 run --title 跳过标题生成；一条 API/part/event，总量 299 |

期望值见各目录 `_expectations.md`（人工核算）。
未知格式 fail closed（非 SQLite/缺表）与产品互斥（MiMo 库拒绝）用例在
`tests/opencode_contract.rs` 内联构造，不占 fixture 目录。
真实样本仍为 latest_fallback；逐记录版本核验与旧游标升级验收未完成。真实样本不自动
登记为 known_version，也不确认其他会话或版本；独立合成边界测试继续保留。
