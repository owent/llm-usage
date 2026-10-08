# M1：统计核心与 SQLite 存储

<a id="m1-statistics-core-and-sqlite-storage"></a>

本页记录 M1 初始实施；后续 schema v2、统计/保留/查询修复与新增回归见
[M0/M1 审查](m0-m1-review.md)，下表 72 测试为初始基线。

<a id="run-information"></a>

## 元信息

| 项目 | 内容 |
| --- | --- |
| 日期 | 2026-09-24 |
| 执行环境 | Windows 11 Pro 26200 x64；Rust 1.98.0；Node v24.21.0 |
| 代码 revision | 工作树未提交改动（desktop/src-tauri 改造为 workspace） |
| 设计依据 | execution.md M1；data-contract.md（统计规则）；validation.md V01–V06/V09/V14/V16 与固定数学样本；scheduling.md 作业语义 |

<a id="commands-and-results"></a>

## 命令与结果

| # | 命令（cwd） | 退出码 | 结果摘要 |
| --- | --- | --- | --- |
| 1 | `npm run verify`（仓库根） | 0 | lint:md / assets:check / svelte-check / fmt --check / clippy -D warnings / cargo test / vite build 全绿 |
| 2 | `cargo test --locked --workspace`（desktop/src-tauri） | 0 | **72 passed / 0 failed**，14 个测试套件，全部真实临时 SQLite 文件库，无 mock |
| 3 | `git diff --check`（仓库根） | 0 | 无空白错误 |

<a id="implemented-changes"></a>

## 实现清单

`desktop/src-tauri` 改为 workspace（app crate + `crates/core`，包名 `llm-usage-core`，纯 Rust 库不依赖 tauri；
`default-members` 保证根命令覆盖 core）。核心模块：

- `domain`：TokenUsage 8 字段全可空（未知不补零）、RecordKind 六类、FieldQuality、Lifecycle、整数最小单位费用；
  token 上限 2^62 超限拒绝；秒/毫秒时间误判拒绝。
- `metrics`：互斥输入求和、cache_input_ratio（分母零/无样本返回无值）、矛盾记录诊断，不调整原值、i128 防溢出。
- `adapters/usage_map`：五种真实口径映射（codex cached⊆input / kimi 四字段互斥 / zcode 双口径相反 /
  copilot 三类相加 / kilo 全互斥），各自独立测试。
- `calendar`：jiff IANA 时区；DST 23/25 小时日、ISO 周含跨年、周日起始标签、保留截止 D−1。
- `storage`：设计中的 18 张表 DDL + 按版本事务迁移；foreign_keys=ON/WAL/synchronous=FULL/busy_timeout=5s；
  user_version 过新拒绝打开；data_revision 单调递增。
- `identity`：命名空间事件身份；修订号优先、生命周期裁决、同级冲突保留不取 MAX。
- `ingest`：批次单事务（事件+游标+解析上下文+日汇总+诊断+作业进度+修订号），6 个故障注入点。
- `aggregates`：来源区间汇总、累计值观察（重置/回归区分）、覆盖互斥求和、额度不转 token。
- `query`：周/月由日加总、比例重算、跨周期 DISTINCT 会话、部分周期标记、unknown 独立行进总计、
  归属排除可按原因列出。
- `jobs`：ingest_runs 状态机、同源合并、重启标 interrupted。
- `retention`：过期日封存（记录时区/字段/来源版本）后删明细、封存日不追加、容量统计含 WAL/备份。

<a id="test-coverage"></a>

## 测试覆盖对照

| 验收 | 测试 | 结果 |
| --- | --- | --- |
| 固定数学样本 11 组 | fixed_samples.rs 11 个，期望写死 | 通过 |
| V01 缓存包含关系 | mapping_v01.rs 8 个 | 通过 |
| V02 重复/更正/乱序/冲突 | dedup_v02.rs 6 个 | 通过 |
| V03 调用/尝试/消息分类 | classification_v03.rs 3 个 | 通过 |
| V04 跨午夜/闰日/跨年周/周日/DST | calendar_v04.rs 6 个 | 通过 |
| V05 模型切换/同名跨 provider/unknown | models_v05.rs 3 个 | 通过 |
| V06 日→周月/加权比例/distinct/部分周期 | rollup_v06.rs 4 个 | 通过 |
| V09 故障注入恢复 | faults_v09.rs 3 个（6 注入点逐一验证+重启重放不重复入库） | 通过 |
| V14 保留/封存/容量 | retention_v14.rs 5 个 | 通过 |
| V16 schema 拒绝/迁移回滚 | migration_v16.rs 5 个 | 通过 |
| 存储/作业/边界 | storage_jobs.rs 14 个 + src 内 4 个 | 通过 |

<a id="newly-locked-dependencies"></a>

## 新锁定依赖

| 依赖 | 版本 | 许可证 | 用途 |
| --- | --- | --- | --- |
| jiff（含 jiff-core/static/tzdb 传递依赖） | =0.2.37 | Unlicense OR MIT | IANA 时区日历与 DST |
| portable-atomic 系（传递） | 1.15.0 / 0.2.8 | Apache-2.0 OR MIT | jiff 传递 |

<a id="failures-and-outstanding-work-at-this-stage"></a>

## 失败与未执行项

| 项 | 状态 | 后续 |
| --- | --- | --- |
| 性能初值（M1 交付项） | 未执行 | 需 ≥100 万事件基准，随 V20 规模数据一起做 |
| 费用日聚合 | 未实现（仅存储） | 禁止跨币种相加，面板属 M6 |
| aggregate_generations 跨事务 building→published 切换 | 未实现 | 当前重算单事务原子完成；复杂重建需要时再加 |
| 迁移前空间检查与一致备份 | 未实现 | 随 V15/V16 完整验收补 |
| 归属核验实际逻辑（WSL/容器） | 仅实现存储与查询规则 | V25 阶段 |
| 定时器/防抖/系统任务 | 未实现（仅表结构与作业语义） | M6 |
| import_manifests 旧库导入逻辑 | 仅状态存储 | M2 |

<a id="证据文件"></a>

<a id="validation-files"></a>

## 验证产物

- `desktop/src-tauri/crates/core/`：核心库源码与测试（tests/ 下 10 个集成测试文件）。
- `desktop/src-tauri/Cargo.lock`：含 jiff 与 llm-usage-core 条目。
