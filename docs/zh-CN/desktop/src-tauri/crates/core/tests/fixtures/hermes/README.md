# Hermes Agent fixtures

<a id="hermes-agent-test-samples"></a>

<a id="hermes-agent-合成-fixtures文档级证据待真实样本"></a>

`synthetic-*` 为合成场景（syn- 前缀），覆盖辅助、跨日和回填等数学边界。
`real-0.21.5` 来自官方镜像在隔离 Podman 中使用本地模型的真实 CLI 与公开续会话；
保存原生 DDL/允许保留的字段、API 与 CLI 用量核对以及版本/哈希依据，排除正文、配置和凭据。

<a id="源码级证据a24固定-commit-ef70b3661cbfcf57e583008ad91dd04d8ba46070"></a>

<a id="source-references-a24-fixed-commit-ef70b3661cbfcf57e583008ad91dd04d8ba46070"></a>

## 源码依据（A24，固定 commit `ef70b3661cbfcf57e583008ad91dd04d8ba46070`）

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
本适配器读取的列子集（时间回退与父子关系），列定义与固定源码一致。
`schema_version` 固定为 30（固定源码 SCHEMA_VERSION）。

<a id="scenarios"></a>

## 场景

| 目录 | 覆盖 |
| --- | --- |
| synthetic-basic-cumulative | 正常单日累计行（五计数列 + api_call_count + first/last_seen） |
| synthetic-aux-task-mutex | 主模型累计 100 + 独立 task 辅助累计 20 ⇒ 120（互斥，不是 220） |
| synthetic-cross-day | 跨两日累计行：单条区间汇总，不拆 3 条调用、不分配到单日 |
| synthetic-v20-backfill-compression | v20 回填行（NULL first/last_seen 回退 session 时间窗）+ 压缩子会话不双计 |
| real-0.21.5 | 两次真实流式调用：原生未缓存输入 849、缓存读 812、输出 4、来源调用汇总 2 |

期望值见各目录 `_expectations.md`（人工核算）。
0.21.5 固定 commit `f97608f178d1ffeca59860195ab7da295f7c8e5f` 与 A24 的
`agent/usage_pricing.py` 核验：input 为未缓存桶，reasoning 为输出子集，
缺字段与初始零均保存为零。因此默认零未知；全部必需桶已知才派生输入/完整总量。
真实 API 总输入 1661、总 token 1665 仅用于对照，不补入缺缓存写有效性标记的原生记录。

整库 `schema_version=30` 没有逐行客户端版本，不能据此核验混合历史会话；注册表保持空，
仍按真实 schema 指纹兼容读取（latest_fallback）。辅助、回填、gateway、混合模型的
合成回归不升级为真实验收。各阶段依据见 [Hermes 容器记录](../../../../../../../docs/validation/desktop-usage/hermes-container-sample.md)。
