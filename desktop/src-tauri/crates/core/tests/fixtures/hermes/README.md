# Hermes Agent 合成 fixtures（文档级证据、待真实样本）

全部场景为**合成数据**（syn- 前缀、常量占位），不是任何真实会话脱敏产物。
本机 2026-09-25 盘点未安装 Hermes Agent（not_found，m0-agent-fixtures.md），
按用户指示以固定源码/官方文档级证据实现，真实数据验收后置。

## 源码级证据（A24，固定 commit `ef70b3661cbfcf57e583008ad91dd04d8ba46070`）

- SCHEMA_SQL（sessions / session_model_usage 18 列与六列主键）：
  <https://github.com/NousResearch/hermes-agent/blob/ef70b3661cbfcf57e583008ad91dd04d8ba46070/hermes_state_common.py>
- 计数累计语义（增量/absolute、record_auxiliary_usage 只写 task 键、
  first_seen 只插 last_seen 推进）：
  <https://github.com/NousResearch/hermes-agent/blob/ef70b3661cbfcf57e583008ad91dd04d8ba46070/hermes_state_usage.py>
- v20 回填 / v22 主键迁移：
  <https://github.com/NousResearch/hermes-agent/blob/ef70b3661cbfcf57e583008ad91dd04d8ba46070/hermes_state_schema.py>
- 路径解析（HERMES_HOME → %LOCALAPPDATA%/hermes → ~/.hermes；命名 profile）：
  <https://github.com/NousResearch/hermes-agent/blob/ef70b3661cbfcf57e583008ad91dd04d8ba46070/hermes_constants.py>
- 官方存储文档：
  <https://hermes-agent.nousresearch.com/docs/developer-guide/session-storage/>

`session_model_usage` 的 DDL 取固定源码逐字列名/主键；`sessions` 只保留
本适配器读取的列子集（时间兜底与世系），列定义与固定源码一致。
`schema_version` 固定为 30（固定源码 SCHEMA_VERSION）。

## 场景

| 目录 | 覆盖 |
| --- | --- |
| synthetic-basic-cumulative | 正常单日累计行（五计数列 + api_call_count + first/last_seen） |
| synthetic-aux-task-mutex | 主模型累计 100 + 独立 task 辅助累计 20 ⇒ 120（互斥，不是 220） |
| synthetic-cross-day | 跨两日累计行：单条区间汇总，不拆 3 条调用、不落单日 |
| synthetic-v20-backfill-compression | v20 回填行（NULL first/last_seen 回退 session 时间窗）+ 压缩子会话不双计 |

期望值见各目录 `_expectations.md`（人工核算）。
取得真实脱敏 fixture 后逐版本升级 known_version 并替换合成样本。
