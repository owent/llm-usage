# OpenClaw 合成 fixtures（基于官方文档、待真实样本）

<a id="openclaw-合成-fixtures文档级证据待真实样本"></a>

全部场景为**合成数据**（syn- 前缀、常量占位），不是任何真实会话脱敏产物。
本机 2026-09-25 盘点未安装 OpenClaw（not_found，m0-agent-fixtures.md）。

<a id="官方文档级证据a092026-09-24-核验"></a>

## 官方文档依据（A09，2026-09-24 核验）

- 磁盘位置/持久层概念（每 Agent 一个 openclaw-agent.sqlite；旧 sessions/
  目录与 sessions.json 为迁移/归档输入，Gateway 启动不导入）：
  <https://docs.openclaw.ai/reference/session-management-compaction/store>
- usage 形状（assistant transcript 条目持久化规范化 usage、input/output
  别名归一、total 回退 input+output、usage.cost）：
  <https://docs.openclaw.ai/reference/token-use>

**文档未给出任何表名/列名**（research.md A09：具体表和兼容版本待验），
因此 fixtures 里的表名/条目字段是**占位发明**（`synthetic-table-*` 与
`syn-*` 字段），仅用于验证发现形状与 fail-closed 行为，不代表真实 schema。

## 场景

| 目录 | 覆盖 |
| --- | --- |
| synthetic-runtime-store | agents/main/agent/openclaw-agent.sqlite（占位表）⇒ 身份确认但表级 schema 待核验，fail closed |
| synthetic-legacy-archive | agents/main/sessions/ 旧 JSONL 归档 + sessions.json ⇒ 迁移输入降级，fail closed |

期望值见各目录 `_expectations.md`。取得真实脱敏样本（运行时库 + 旧归档）
后在 versions/runtime_store 实现读取映射并替换。
