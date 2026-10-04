# M1a：历史来源身份与存储分区

## 元信息

| 项目 | 内容 |
| --- | --- |
| 日期 | 2026-09-25 |
| 执行环境 | Windows 11 x64；rustc 1.98.0；Node v24.21.0 / npm 12.0.2 |
| 代码 revision | 未提交工作树（M2-D 之后 + 本次新增） |
| 依据合同 | [data-contract.md 来源身份与交换合同](../../design/desktop-usage/data-contract.md#provenance)；execution.md M1a；validation.md V28 |

## 命令与结果

| # | 命令（cwd） | 退出码 | 结果摘要 |
| --- | --- | --- | --- |
| 1 | `cargo test -p llm-usage-core --test migration_v28`（desktop/src-tauri） | 0 | **7 passed / 0 failed** |
| 2 | `cargo test -p llm-usage-core` | 0 | 283 passed / 0 failed（M2-D 基线 276 + V28 7） |
| 3 | `npm run verify`（仓库根） | 0 | 全链路绿（见 m6-desktop-core.md 同日记录） |

## 实现清单

- **schema v4 迁移**（storage/schema.rs）：
  - `origin_hosts`（host_id 不透明稳定 ID、is_local）+ `origin_host_names`
    （主机名观察史：改名不换 ID，同名可为不同主机）；
  - `source_instances.origin_host_id`（默认 `legacy_unknown` 命名空间）+
    主机索引；已属其他主机的来源不被覆盖，`legacy_unknown` 可被本机核验采集
    认领（文件在本机 + locality 已核验 = 可证明映射）；
  - `daily_usage` 重建为按来源实例分区（instance_id 进主键）：旧混合行保留
    原值进 legacy_unknown 分区（不虚构拆分）；迁移时未封存日按现存事件重算
    为逐来源分区（推导，非猜测）；封存行随分区迁移且语义不变（仍阻止重算）；
  - `recompute_day` 双形状（v4 前旧库走无分区 INSERT，保证 v2 迁移路径可用）。
- **主机身份 API**（storage/mod.rs）：`ensure_local_host`（randomblob 生成
  `host-<32hex>`，settings.local_origin_host_id 持久化，不用主机名/IP 作身份）、
  `observe_hostname`、`register_origin_host`（外部来源登记，导入用）。
- **采集接线**：`RunConfig.origin_host_id` → `upsert_source_instance` 带主机写入。
- **交换合同**（exchange.rs，格式 `llm-usage-exchange-1`）：ExchangeExport
  （版本/批次/主机（可脱敏）/来源注册/逐事件记录（含 parse_basis、修订、
  完整性）/封存日分区/快照-增量性质与显式删除声明）；`build_export` 只读构建；
  `decide_record_merge` 复用 ingest 仲裁（修订号优先→生命周期→内容）保证
  交换判定与本地写入单一事实来源。

## V28 用例对应

| 用例 | 结果 |
| --- | --- |
| 主机改名不重复计数（同 ID、主机名观察史） | 通过 |
| 同名不同主机不发生键冲突（键用 host_id） | 通过 |
| 旧实例仅被本机核验采集认领；既属他机不覆盖 | 通过 |
| v3 库真实迁移：legacy_unknown 分区、封存值保留、未封存日重算分区 | 通过 |
| 查询跨来源求和；删除明细后分区与注册保留来源身份 | 通过 |
| 导出含合同字段（版本/批次/来源/parse_basis/脱敏）并 JSON 往返 | 通过 |
| 合并判定合同表（幂等跳过/权威替换/互斥新增/冲突保留） | 通过 |

## 与合同的偏差说明

- 同修订号且内容不同的更正（corrected）判 Conflict 而非替换：与 ingest 仲裁
  现行为一致（修订号相等只比内容）；更正要替换须携带更高修订号。交换判定
  不单独发明第二种语义。
- 复制数据库到新机器的场景：身份与采集由调用方显式传入，注册冲突的
  映射/确认流程 UI 属后续 Merge 功能（合同原文"另行排期"）。

## 未完成项

| 项 | 状态 | 后续 |
| --- | --- | --- |
| 完整导入/Merge 写入路径 | 未实施（按计划） | 随导入功能排期；判定与格式已就绪 |
| 多主机历史导出（按 host 分包） | 未实施 | build_export 当前导出本库 local host 视角 |

<a id="证据文件"></a>

## 验证产物

- `storage/schema.rs`（v4）、`storage/mod.rs`、`ingest.rs`（recompute_day 分区）、
  `adapters/framework.rs`（origin_host_id）、`exchange.rs`、
  `tests/migration_v28.rs`。
