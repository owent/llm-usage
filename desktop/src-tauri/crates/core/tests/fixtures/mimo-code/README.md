# MiMo Code fixtures

<a id="mimo-code-合成-fixtures文档级证据待真实样本"></a>

`synthetic-*` 为合成数据。`real-0.1.15` 是官方客户端调用真实本地模型后的
SQLite 白名单投影，八条 part 与八条独立 API 用量逐条核对；原始库摘要见 provenance。
仅保留身份占位、时间、模型、用量和必要 schema，正文、配置及原始路径不保留。
八次 finish 均为 length，不能以 CLI 退出 0 宣称任务完成。

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

当前语义以官方 0.1.15 提交 `14dfe68a1c121f859544ba810b3c308e8501bfb2`
自身 `session/session.ts:getUsage` 为准：SDK 输入减缓存、输出减 reasoning 后落盘；
加回同份分项恢复正 SDK 总量，默认零子桶及费用仍未知。
不从 OpenCode 血统或库内最高版本认证行，仍 latest_fallback。

## 场景

| 目录 | 覆盖 |
| --- | --- |
| synthetic-step-finish | 正常：主/子会话 × 3 条 step-finish 部件逐次入账 + text 部件过滤；无累计列 ⇒ 无对账 |
| synthetic-unknown-version | 未收录 session.version ⇒ latest_fallback 兼容尝试 |
| real-0.1.15 | 八次 CLI/续会话 API/part，输入 25,588、输出 1,024、总量 26,612，已知缓存读 20,733 |

期望值见各目录 `_expectations.md`（人工核算）。
未知格式 fail closed 与产品互斥（OpenCode 库拒绝）用例在
`tests/mimo_code_contract.rs` 内联构造。
默认零/旧完整摘要/越过样本的旧处理位置、并行/回滚及冲突保护见 `mimo_real_contract.rs`。
单一路线真实样本不认证所有协议、角色或版本。
