# 原生交互、归档增量与规模验收

日期：2026-10-04；版本 0.2.1；cwd：D:/workspace/projs/github/owent/llm-usage。
Windows 11 Pro x64 10.0.26300；Ryzen 9 9950X3D（16 核/32 线程）、约 125 GiB RAM。
Node 24.21.0、Rust 1.98、Tauri CLI 2.12.0、WebView2 154.0.4258.53。
依赖以锁文件为准。本页保留该日较早批次的结果；后续查询加速、资源及并行采集
结果见 [最新验收](current-acceptance.md) 和 [剩余计划执行](plan-execution.md)，
当前任务状态以 [Plan.md](../../../Plan.md) 为准。临时日志、程序与合成库均在根
build/query-next/、build/plan-finalization/ 或 build/plan-completion/，不复制私人来源正文。

## 当前实现

- 清理与清空共用后台维护任务，提交前可取消；原子状态决定取消或提交谁成功。
  SQLite 进度回调中断耗时 SQL，事务整体回滚，事件、修订、保留处理位置及汇总不变，
  取消不重扫。提交开始后明确拒绝取消；已接受取消的按钮禁用至任务结束。
  不完整备份只删除自身文件，命名冲突耗尽时保留已有快照并提前失败。
- 系统任务失败测试仅在 OS 调用边界注入错误，实际 AppState/SQLite 核对调用前
  意图已持久化、错误可查询、关闭后的残留触发不采集及重试恢复。
  “仅扫描手工目录”默认关闭，开启后同时限制 GUI/后台的默认来源和额度发现。
- 图表方向键、Home/End 移动，Shift 连续预选，Enter 查询；沿用鼠标的范围、
  模型/Agent/费用联动。十语言实际保存，HTML lang 随之更新，设置控件具可访问名称，
  表单按容器宽度换行。CSS 页面缩放不能代替 OS DPI 或辅助技术实测。
- 日汇总、Agent 分布及日粒度图表逐行合并，仅保留结果分组与活动日集合。
  会话/耗时先按准确本地日边界在 SQLite 分组，跨日继续合并完整来源/会话身份，
  不叠加每日 DISTINCT；小时/DST、未知字段、封存和归档覆盖口径保持不变。
- schema 11 不变；旧库首次打开补建归属排除、载体覆盖及明细覆盖索引，
  不清库、不重建汇总、不改数据修订。覆盖索引增加空间和写入成本；最终查询与首屏
  使用已建索引的库，不把该首屏结果当成旧库首次补建索引的耗时。
- 每连接汇总缓存最多 4 项、字符串/向量保留容量预算 2 MiB；这不等于进程内存预算。
  键包含完整筛选、选区、时区、日期、粒度、今天和保留边界；同一读事务内核对修订、
  data_version 和 total_changes，同连接写入或其他连接提交后失效，失败/超限不缓存。
  两个只读连接互斥复用，忙时使用短命备用；查询结束不留下读事务。
- 仅今天增量使全局修订推进时，不再重复物化稳定历史。事务内保存日期/策略、
  已完成分区的修订和行数；覆盖已完成周/月/年的日边界取三者最新边界。
  迟到数据、删除、缺失归档、日期/策略变化、无效标记或异常未来修订均走完整重建。
  清空同步移除标记，取消回滚标记和统计，不据来源最高版本或未变条数猜测兼容。
- 采集期间每 500 ms 核对状态，空闲恢复 10 秒检查。已接受但尚未运行的任务保留
  完成等待；两次检查间完成的短采集也刷新界面，避免掉回空闲间隔后延迟通知。

缓存依据见 [SQLite data_version](https://www.sqlite.org/pragma.html#pragma_data_version)
及 [rusqlite 0.40.2 total_changes](https://docs.rs/rusqlite/0.40.2/rusqlite/struct.Connection.html#method.total_changes)。
跨连接 WAL 小库探测另核对：持有读快照期间 data_version 与旧快照一致，结束后
下一次读可见外部提交；同连接/外部无修订号写入的缓存失效另有 Rust 回归。

## 工程与原生检查

| 命令/方法 | 退出码与结果 | 核验范围 |
| --- | --- | --- |
| npm run verify | 0；Rust 852、前端 20、脚本 3；默认忽略 5 项显式/环境测试 | 177 Markdown、资源、类型 0 错误/告警、fmt、Clippy -D warnings、合同/集成测试及前端构建 |
| npm run build:desktop | 0；NSIS 3,810,444 字节（3.63 MiB） | 最终 release 与嵌入前端；未安装 |
| npm run test:headless | 0；11 项，最终 3 事件 / 75 token | 实际 release、SQLite、隔离非空合成文件与持久化规则 |
| npm run test:desktop -- --runs 20 | 0；13 项，小数据 20 次首屏 P95 724.3 ms | 实际 WebView2/Tauri IPC、鼠标/键盘、离线、重启、交换、十语言及系统分钟任务 |
| npm run test:browser | 0 | Edge 模拟 IPC，五页/选区/联动、十语言、主题、筛选、分页、短采集交接及空闲轮询 |
| Windows 显式测试 | 0；COM 任务、HKCU 测试键各 1 项 | 测试命名空间的注册/查询/漂移修复/幂等删除；不替代真实权限拒绝 |
| ai-maintenance quick_validate.py | 0；Skill is valid | 静态格式；description 未改，不声称模型触发或质量评估通过 |

日志：build/query-next/verify-final.log、desktop-build-final.log、browser.log、
headless-final.log、native-final.log；最后两项结果分别在
build/plan-completion/headless/1791056946337/ 与 native/1791056947059/result.json。
前端产物 JS 1,065.74 kB / gzip 357.03 kB，CSS 51.73 / 9.50 kB。
Windows 显式测试在当前版本分别执行 native_task_roundtrip 和 registry_roundtrip
（cargo test --offline，--ignored），日志为 windows-task-final.log / windows-registry-final.log；
Skill 校验使用 python -X utf8，避免 Windows 系统默认编码误读中文。

13 项原生回归包含十语言逐项真实保存及 CSS 缩放 1 / 1.25 / 1.5 / 2，核对
无水平溢出、保存按钮可见、可见设置表单可访问名称及统计不随语言改变。
无 GUI 场景注册当前用户的分钟任务，关闭界面后新增第三条合成事件；观察 OS
执行时间、Ready/退出 0，再在重开界面前只读 SQLite，核对 3 事件 / 75 token。
重启保留结果，测试结束删除自身任务。普通用户实际触发与失败注入分别记证。

## 完整查询

bench_v20 的既有合成库经实际 EventPipeline、事务和日/小时聚合入库，50,000 条一批；
覆盖 366 日、50 模型、20 Agent、20 来源实例。会话跨日重复，20% 事件在每来源复用
两个大规模会话键，跨来源仍独立；多 Agent 合成实例只作维度压力测试。
最终程序只读既有库，实际 query_summary 包括指标、会话、耗时、覆盖、模型/Agent
分布，断言全部调用数及维度数，不是单独 SUM/COUNT 的计时。

每组先清除连接缓存测 20 次完整查询，再测 20 次命中；明细测 100 次每页 200 条。
完整查询 P95 取排序后第 19 项，明细 P95 取第 95 项；OS 文件缓存未主动清空，
未命中指应用汇总缓存。
从仓库根执行，均退出 0：

```powershell
cargo build --offline --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core --release --example bench_v20
desktop/src-tauri/target/release/examples/bench_v20.exe build/plan-finalization/million-1 1000000 --query-only
desktop/src-tauri/target/release/examples/bench_v20.exe build/plan-finalization/ten-million 10000000 --query-only
```

| 事件数 | 主库 / WAL MiB | 未命中 P50 / P95 ms | 命中 P95 ms | 明细 P95 ms |
| --- | --- | --- | --- | --- |
| 1,000,000 | 906.3 / 0.3 | 926.5 / 964.6 | 0.033 | 1.29 |
| 10,000,000 | 8,034.1 / 0.0 | 4,093.8 / 4,177.5 | 0.049 | 1.23 |

结果日志：build/query-next/million-final.log 与 ten-million-final.log。
在自身合成库首次补建当前索引时，Storage::open 分别耗时 904.4 / 15,245.0 ms；
此过程含打开检查，不是原生首屏。未重测当前索引下从空库完整入库的吞吐和内存，
不沿用旧索引阶段的入库数字。未命中仍超出 [200 ms 目标](../../design/desktop-usage/architecture.md#budgets)，
缓存命中结果不认证该目标。

## 原生首屏与资源

`node desktop/tests/native-scale.mjs --runs 20 --idle-seconds 600 --ui-cancel` 退出 0；
最终 release，结果 build/plan-finalization/native-scale/1791056317247/result.json，
日志 build/query-next/native-scale-final.log。
规模脚本启动前只读核验真实路径在根 build/ 内、100 万事件及 20 个 synthetic/bench
来源，拒绝将维护 IPC 指向真实 Agent 库；--help 不启动 GUI。
帮助、非法目录拒绝及合法库 1 次原生/取消回归均通过（native-scale-guard.log）；
该检查不替代下表独立 20 次首屏与完整十分钟资源测量。

| 项目 | 实际测量 |
| --- | --- |
| 百万事件首屏 | 20 次，P95 1,223.1 ms；进程启动至总览有值且历史图已渲染，含 CDP 等待 |
| 取消 | 手动清理/清空各 1 项实际 IPC，加设置页取消按钮；事件/修订不变，取消不重扫 |
| 空闲 | 连续 601.4 秒、118 次采样，根进程始终存活；页面静止，自动采集暂停 |
| 进程范围 | 根应用及全部后代 WebView，最多 7 进程，约每 5 秒采样；包含退出进程最后可见 CPU 累计值 |
| Private bytes | 均值 333.6 MiB、峰值 483.9 MiB；180 MiB 目标未达标 |
| Working set | 峰值 524.4 MiB，单独报告，不替代 private bytes |
| 空闲 CPU | 单个逻辑核 0.309%，本机低于 1% 目标，不除以 32 线程数 |

首屏使用已建索引的百万库，首次和后续 OS 缓存混合。空闲在两项 IPC 与一次 UI
取消后、同一应用的总览测量，CDP 已启用；不代表全新启动、托盘或 headless 状态。
这台开发机不是拟定 4 核/16 GiB 基准，不能认证后者。5 秒资源样本不是分配器瞬时峰值。
已连接 WebView 的已观测请求无外部 HTTP、无脚本异常，不扩大为全进程网络审计。

## 百万库增量刷新

`node desktop/tests/native-incremental.mjs --runs 20 --enforce-budget` 退出 0；
结果 build/query-next/native-incremental/1791056283753/result.json，日志 incremental-final.log。
使用 [Node SQLite 在线备份](https://nodejs.org/api/sqlite.html#sqlitebackupsource-db-path-options)
（API 最低 Node 22.16）从只读百万库复制到新建隔离目录，先断言全部来源为 synthetic/bench；
不修改基线库或真实 Agent 文件。本机实际运行 Node 24.21.0。

先发现并消费一条合成 Codex 文件记录，再连续 20 轮每轮追加 1,000 条；计时从实际
采集按钮点击到今日卡片可见新值，包含来源读取、解析、提交、日聚合、分级保留、
状态轮询、真实 IPC 和界面更新。每轮另核对精确调用 +1,000、token +15,000、
修订推进及数据库事件数；最后重扫幂等。合成版本不认证真实 Agent 新版本。

- P95 1,386.7 ms，最大 1,406.3 ms；2 秒预算通过，最终 1,020,001 条事件。
- 可见卡片没有替换 IPC 或注入查询结果；已观测请求无外部 HTTP、无脚本异常。
- 稳定归档跳过另有迟到数据、删除、缺失归档、日期/策略/异常修订回归。
  隔离归档探测完整物化耗时 1,770.7 ms；稳定历史下仅推进全局修订的两次检查
  为 147.5 / 147.7 ms、物化 0 行。它不是完整增量刷新预算的替代。
- ingest_runs 的起止字段使用任务统一时间，不能由其差值声称解析/聚合分阶段耗时。

## 最终文档与剩余条件

最终检查均退出 0：177 Markdown 无问题；12 份变更文档的 95 个本地引用及锚点无错误；
Skill 静态格式通过；git diff --check 通过。只读残留核对：测试任务、应用进程、测试
开机项均为 0，临时文件不进入 Git 状态。结果在 build/query-next/ 的 docs-final.log、
links-final.json、skill-final.log、diff-final.log、residue-final.json。
不以工程测试代替链接检查，不把模拟 IPC 当作原生验收。

未命中查询和全进程内存仍未达标；拟定硬件基准、当前索引下完整导入峰值、
唤醒次数、OS DPI/辅助技术、macOS/Linux 原生 GUI 与持续 CI 尚未验收。
安装按既有指示跳过，升级/卸载/注销/回滚尚未验收。Gemini/Qwen 默认本机载体目录
未发现，不启动 Agent 制造样本；其余来源版本/遥测覆盖、托盘、节能、文件监听及
应用内时间预算/重试仍见 [Plan.md](../../../Plan.md)，本页通过不代表全部计划完成。
