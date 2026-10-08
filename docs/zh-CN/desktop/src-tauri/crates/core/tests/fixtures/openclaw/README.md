# OpenClaw fixtures

<a id="openclaw-test-samples"></a>

<a id="openclaw-合成-fixtures文档级证据待真实样本"></a>

synthetic-* 保留原合成数据（syn- 前缀、常量占位），只验证未知 schema 与旧迁移
输入仍 fail closed。real-2026.9.8 为 2026-10-06 官方 npm 客户端在独立无网络
容器内公开 CLI/续会话两次调用真实本地模型的允许字段脱敏产物。

<a id="官方文档级证据a092026-09-24-核验"></a>

<a id="official-documentation-a09-checked-2026-09-24"></a>

## 官方文档依据（A09，2026-09-24 核验）

- 磁盘位置/持久层概念（每 Agent 一个 openclaw-agent.sqlite；旧 sessions/
  目录与 sessions.json 为迁移/归档输入，Gateway 启动不导入）：
  <https://docs.openclaw.ai/reference/session-management-compaction/store>
- usage 形状（assistant transcript 条目持久化规范化 usage、input/output
  别名归一、total 回退 input+output、usage.cost）：
  <https://docs.openclaw.ai/reference/token-use>

此前 synthetic fixtures 的表名/条目字段是**占位发明**（`synthetic-table-*` 与
`syn-*` 字段），仅用于验证发现形状与 fail-closed 行为，不代表真实 schema。

<a id="scenarios"></a>

## 场景

| 目录 | 覆盖 |
| --- | --- |
| synthetic-runtime-store | agents/main/agent/openclaw-agent.sqlite（占位表）⇒ 身份确认但表级 schema 待核验，fail closed |
| synthetic-legacy-archive | agents/main/sessions/ 旧 JSONL 归档 + sessions.json ⇒ 迁移输入降级，fail closed |
| real-2026.9.8 | schema_meta、session_windows、transcript_events 精确字段提取及独立 API 用量；非缓存输入 6,157/输出 154/缓存读 6,101，默认零未知 |

期望值见各目录 `_expectations.md` 与 real-2026.9.8/provenance.json。真实数据提取剥离
提示词、响应、配置和私有路径，身份替换为稳定占位。重建 SQLite 使用最小所需
结构，坏类型/压缩/外部来源等变体为定向合成边界，不能据此核验对应真实产品场景。
完整原始样本仅存根 build/，schema/时间/默认零语义见
[读取说明](../../../../../../../docs/design/desktop-usage/openclaw-runtime.md)。
