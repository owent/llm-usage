# MiMo Code 合成 fixtures（基于源码、待真实样本）

<a id="mimo-code-合成-fixtures文档级证据待真实样本"></a>

全部场景为**合成数据**（syn- 前缀、常量占位），不是任何真实会话脱敏产物。
本机 2026-09-25 盘点未安装 MiMo Code（not_found，m0-agent-fixtures.md），
按用户指示以固定版本源码依据实现，真实数据验收后置。

<a id="源码级证据a14固定-commit-456678b6a5afb0eef3fe2754575637218cfb3c84"></a>

## 源码依据（A14，固定 commit `456678b6a5afb0eef3fe2754575637218cfb3c84`）

- session/message/part 三表 DDL（message.agent_id 特有列、session 无
  tokens_* 累计列）：
  <https://github.com/XiaomiMiMo/MiMo-Code/blob/456678b6a5afb0eef3fe2754575637218cfb3c84/packages/opencode/src/session/session.sql.ts>
- StepFinishPart zod schema（tokens{total?, input, output, reasoning,
  cache{read, write}} + cost）与 Assistant（modelID/providerID 必需）：
  <https://github.com/XiaomiMiMo/MiMo-Code/blob/456678b6a5afb0eef3fe2754575637218cfb3c84/packages/opencode/src/session/message-v2.ts>
- 路径解析（MIMOCODE_HOME → `<home>/data`；XDG 缺省 ~/.local/share/mimocode）：
  <https://github.com/XiaomiMiMo/MiMo-Code/blob/456678b6a5afb0eef3fe2754575637218cfb3c84/packages/shared/src/global.ts>
- 库文件（data/mimocode.db，通道变体）：
  <https://github.com/XiaomiMiMo/MiMo-Code/blob/456678b6a5afb0eef3fe2754575637218cfb3c84/packages/opencode/src/storage/db.ts>

tokens 字段语义沿用家族 固定版本依据（OpenCode 侧
publish-llm-event.ts tokens()/llm protocols 证实 input=未缓存输入、
inclusive inputTokens=in+cr+cw）；MiMo 侧固定源码证实 shape 同形，
数值口径待真实样本复验。

## 场景

| 目录 | 覆盖 |
| --- | --- |
| synthetic-step-finish | 正常：主/子会话 × 3 条 step-finish 部件逐次入账 + text 部件过滤；无累计列 ⇒ 无对账 |
| synthetic-unknown-version | 未收录 session.version ⇒ latest_fallback 兼容尝试 |

期望值见各目录 `_expectations.md`（人工核算）。
未知格式 fail closed 与产品互斥（OpenCode 库拒绝）用例在
`tests/mimo_code_contract.rs` 内联构造。
取得真实脱敏 fixture 后逐版本升级 known_version 并替换合成样本。
