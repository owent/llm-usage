# M6 核心实现：桌面界面、刷新、调度、设置与导出（主体功能，部分验收）

本记录覆盖 M6 的功能实现与已执行的验证；**真实 Windows 桌面逐操作验收
（V13–V18/V23–V25 的 GUI 部分）尚未执行**，见「未完成项」。

## 元信息

| 项目 | 内容 |
| --- | --- |
| 日期 | 2026-09-25 |
| 执行环境 | Windows 11 x64；rustc 1.98.0；Node v24.21.0；WSL2 Ubuntu（rustc 1.98.1） |
| 代码 revision | 未提交工作树（M1a 之后 + 本次新增） |
| 依据合同 | execution.md M6；architecture.md（refresh/IPC/资源）；scheduling.md；F3 i18n 合同 |

## 命令与结果

| # | 命令（cwd） | 退出码 | 结果摘要 |
| --- | --- | --- | --- |
| 1 | `npm run verify`（仓库根） | 0 | lint:md 110 文件 0 问题、svelte-check 0 错 0 警、fmt/clippy、cargo test 283+2、vite build **650.86 kB / gzip 220.32 kB**（预算 gzip ≤ 1 MiB 内） |
| 2 | `cargo run -p llm-usage-m0 -- --headless`（desktop/src-tauri，APPDATA 指向临时目录） | 0 | 三源全 succeeded：codex 2,597 + pi 37 + omp 8,767 = **11,401 事件**；跨源合计 input 1,322,967,993 = pi 2,805,788 + omp 1,021,931,700 + codex 298,230,505（与 m2bc/m2d 基线一致） |
| 3 | 同上（复扫） | 0 | **幂等**：added=0，总数仍 11,401（不兼容旧版文件的重复诊断为既有设计行为） |
| 4 | WSL `cargo check -p llm-usage-core` / `-p llm-usage-m0` | 0 | core 与 app 在 Linux 编译通过（V27 分项证据） |
| 5 | WSL `cargo test -p llm-usage-core` | 0 | 36 个测试二进制全绿（Linux 侧） |

## 实现清单

- **后端**（desktop/src-tauri/src/）：app_state（单写者 Mutex+Storage+、设置持久化、
  主机身份初始化）、scanner（六适配器注册表、全源刷新、间隔调度线程、有限保留接线）、
  commands（summary/heatmap/list_sources/set_source_enabled/refresh_*、get/set_settings、
  app_info、export_data）。M0 试验命令（sqlite_probe/read_sample_file）退役。
  token 大数值按 IPC 合同以十进制字符串传输；错误为结构化 code+message。
- **查询扩展**（core/query.rs）：agent_breakdown、hourly_breakdown（今日逐小时，
  质量白名单过滤 unknown）、heatmap_cells（周×小时）；calendar 增加 local_hour_of/
  local_weekday_of。
- **前端**（desktop/src/）：App（四页签：总览/趋势/数据源/设置）+ 组件
  （SummaryCards、BreakdownTables 模型/Agent 表、TodayHourly、TrendChart 日/周/月、
  UsageHeatmap、SourceList 含兼容尝试/不兼容标记与启停、SettingsPanel 含导出）。
  ECharts 按需（bar/line/heatmap + Canvas 一个渲染器）；筛选（范围/粒度/Agent/模型）
  防抖重查；空态/错误态/进行中标记/部分历史标记/未知行；刷新按钮合并触发 +
  3 秒轮询状态。
- **i18n（F3 合同落地）**：src/lib/i18n.svelte.ts——自实现轻量消息目录
  （zh-CN 完整 + en），语言协商（设置→系统→默认）、缺键回退默认语言+告警、
  {name} 插值、Intl 数字/百分比格式化（统计口径不随语言改变）、切换即时生效。
- **headless 模式**：`--headless`/`--scan-once` 无 WebView 单次采集后退出
  （V24 系统任务的提取路径）。
- **导出**：summary-csv（展示用，公式注入防护）与 exchange（M1a 无损交换 JSON，
  含来源身份/修订/parse_basis，主机名默认脱敏）；写入应用数据目录 exports/
  或调用方指定目录，返回完整路径。
- **调度**：全局间隔（默认 60 秒，0=暂停）；启动先回填一次；休眠醒来立即补扫
  一次（错过合并）；设置变化下一轮生效。逐源启停在数据源页。

## 发现并修复的缺陷

| 处 | 问题 | 修法 |
| --- | --- | --- |
| scanner | 六适配器共用同一 run_id 前缀 ⇒ ingest_runs.run_id 主键冲突（pi/omp 整实例失败） | 前缀按适配器序号唯一化（`scan-{ts}-a{n}`） |
| main.rs db_path | 漏拼文件名，把数据库指到 `%APPDATA%` 下既有占位文件 `llm-usage`（空 SQLite 库）并写入了数据 | 修正路径为独立目录 `llm-usage-desktop/llm-usage.sqlite`；见下方事故记录 |

### 事故记录（Roaming 占位文件）

`%APPDATA%\llm-usage` 是一个既有的空 SQLite 文件（无任何表；来源不明，第三方遗留）。
db_path 缺陷使首次 headless 把应用库写在该文件上（4.1 MB，仅含本应用 schema）。
处置：数据无外部内容受损（文件原本无表）；已将该文件恢复为空 SQLite 占位
（保留其存在，不删除第三方文件），应用改用 `llm-usage-desktop` 独立目录避开。

## 修订：UI 查询为空缺陷（2026-09-26 发现并修复）

用户实机反馈"查询失败、页面无显示"。定位（query_probe 对应用库复现）与修复：

| 处 | 问题 | 修法 | 验证 |
| --- | --- | --- | --- |
| scanner.rs | 日汇总分区硬编码 `timezone: "UTC"`，UI 按用户统计时区（Asia/Shanghai）查询 ⇒ tz_version 不匹配，汇总永远为空（数据实际已入库） | RunConfig.timezone 取设置值（V04/V12 日界随用户时区） | 回归测试：UTC 提交后上海时区 0 可见 → recompute 后 1 调用归属上海日 2026-09-26 |
| app 读写共锁 | 查询与扫描共用一把互斥锁，长扫描（kilo 首扫分钟级）阻塞全部 UI 命令 | 查询路径改 `Storage::open_readonly` 独立只读连接（WAL 一写多读合同）；写锁按适配器分段获取 | 回归测试：写事务未提交期间只读查询照常、未提交数据不可见 |
| 存量库 | 已有 UTC 分区在新时区下不可见 | init/时区变更触发 `recompute_days_in_tz`（事件仍在 ⇒ 推导非猜测；封存日跳过；750 天上限） | 用户库副本修复后：14 周期、12,954 调用、1.55B token、18 模型、7 Agent |

新增诊断工具：`examples/query_probe.rs`（复现 app 查询构建步骤，可选 repair 模式）。
core 新增公开 API：`Storage::open_readonly`、`ingest::recompute_days_in_tz`。

### 追加加固（2026-09-26 用户复验后）

用户实机 dev 模式复验：查询间歇性报
`db_readonly: ... disk I/O error`（截图证据；同一会话内亦有成功查询——修订号
芯片与错误横幅同现）。单测/探针对库文件与 API 均无法复现（含带 stale WAL/SHM
副本），判断为打开瞬间的环境性冲突（杀软实时扫描/热文件句柄等）：

- 读路径加固为 `read_conn`：只读打开带 3 次短重试（40ms），仍失败回退写连接
  互斥锁——UI 查询宁可短暂排队也不硬错；
- `open_readonly` 去掉多余 foreign_keys pragma（查询不需要，减少失败面）；
- 单实例纪律：发现旧实例崩溃残留（stuck running run + 18MB WAL）。跨进程互斥
  属 V24 未实施项，当前需用户避免多实例同时运行（已在此记录）。

**应用改名**（用户要求）：Cargo 包 llm-usage-m0 → llm-usage-desktop、二进制与
安装包 **LLMUsage**（LLMUsage_0.1.0_x64-setup.exe）、npm 包 llm-usage-desktop、
窗口标题 "LLM Usage"。数据库路径不变（llm-usage-desktop 目录），数据无需迁移。

**用户真实库预跑验证**（headless 等价重启路径）：时区分区修复生效
（Asia/Shanghai 180 行）、stuck run 清零、查询 14 周期/13,272 调用/
1.65B token/18 模型/7 Agent。

## 分级归档与设置页合同（2026-09-26 用户需求实施）

- **schema v5**：`hourly_usage`（小时分桶，提交事务内随受影响日重算）与
  `period_usage`（周/月/年物化）两张新表；`enforce_tiered_retention`
  按明细 7/小时 30/日 365/周 3650/月 10950 天/年终身逐级清理；
  **进行中周期保护**（日层下限不越过当前周/月/年起点——否则未完成周期
  永远缺失；实际生效保留期可比设定多至一个周期）。
- **查询合并**：周/月粒度在日层存活期外读物化行（同周期二选一，日层
  更新鲜）；小时图改读持久化小时表（明细删除后仍有 30 天数据）。
  修复既有缺陷：小时桶 `hour` 字段从未赋值（旧实现所有数据落在 0 点）。
- **设置 DTO**：week_start 改 Option（None=跟随语言地区：zh→周一、
  en-US/CA→周日）；refresh_interval_secs 默认 3600（每小时）；
  新增 RetentionTiers 与 hostname_alias。
- **系统命令**（Windows，无需提权）：`set_auto_start`（HKCU Run 键）、
  `set_refresh_task`（schtasks 每小时 headless 任务 LLMUsageDataRefresh）、
  `system_task_status`、`pick_save_path`（rfd 原生保存对话框，导出位置
  由用户选定——满足 architecture.md 导出合同）。
- **导出改造**：交换包改为**聚合数据**（来源注册 + 全部日分区 + 周期分区
  \+ 小时层，不含 session 明细；重导入可重建历史趋势）。
- 增量读取既有方案（文档化）：JSONL 字节偏移游标 + SQLite 水位
  （kilo/opencode part.id 更新序）+ 文件身份/代数（改名/截断/同长替换重扫）。
- 回归：tests/tiered_retention.rs 4 用例（分层清理/物化/查询合并/小时存活
  /策略校验）；全套 57 个测试二进制绿。

## 多用户、导入闭环、Codex 旧版与 UI 补全（2026-09-26 第三轮）

### v6 多用户

- `users` 表 + `source_instances.user_id`（默认 default）；不同用户的来源可
  共享同一来源主机（origin_hosts 与 user 多对多）；查询按
  `Filters.instances`（app 层解析当前用户来源集合，core 不感知 user）。
- 用户命令：list/create/set_current/assign_source；顶栏用户切换下拉。
- 真实库副本预跑：schema 6、7 来源全归 default、查询过滤正确。

### 导入闭环（M1a 合同实施）

- 导出补小时层（ExchangeHourlyPartition）；`import_aggregate`：
  同分区键修订比较（更高替换/同修订幂等/更低冲突+诊断）；来源注册
  不覆盖已有归属；导入主机登记外部。UI：设置→导出→导入按钮
  （pick_open_path 对话框 → import_exchange → 计数展示）。

### 分级归档新默认与手动清理

- 新默认（二轮调整降低聚合消耗）：小时 3/日 90/周 3 年/月 10 年/年终身；
  明细 7 天不变。明细层无层级约束（可比小时长——冗余不丢数据）。
- `storage_stats`（各层条目数 + 库/WAL 字节）；`manual_cleanup(days_before)`
  各层统一按天数截断（进行中周期保护仍生效）。归档页显示统计+手动清理。

### Codex 0.139–0.151 旧版支持（M2-D 遗留清零）

- **取证**：全量 238 文件 13,481 条 token_count 逐条分桶
  （build/codex-legacy-forensics/）。语义判据（total 增量法）：
  delta>0 ⇒ 新调用（last=最新一次）；delta==0 且 last 未变 ⇒ 重复上报去重；
  delta==0 且 last 变化 ⇒ 85/85 紧随 compacted（压缩回声，carried 口径）；
  delta<0 ⇒ 源端回退（诊断+基线重定）。
- `rollout_legacy.rs`：21 个版本注册 → 不兼容 238→**0**；
  真实核对 codex 事件 2,578→**15,955**（legacy 13,358 与 Python 取证一致）；
  对账 262 matched/37 mismatch（均已解释类别）。
- **端到端**：全新空库 headless 采集 40,472 事件 → 7 天明细层即时清理
  （合同行为）→ 聚合层保留完整历史：codex 日汇总 1,835,125,364
  （与 real_verify 逐位一致）、56 天、查询 14 周期/13,194 调用可见。

### 前端补全

- 滚动修复：根因 app.css `#app{height:100vh;overflow:hidden}` 钉死视口；
  改 `min-height:100vh` 恢复原生滚动。
- 总览：今日概览卡 + 缓存构成堆叠图 + Agent/模型饼图（两列网格）；
  趋势：指标四选一切换 + 周分布条形图。
- 设置页 Typora 风格左侧竖排菜单；归档统计与手动清理；导入按钮。
- i18n 新增 43×2 键（累计 161×2）。

## 第四轮（2026-09-26 用户需求）

- **清理全部数据**：`clear_all_data` 命令删除所有归档层+诊断+游标
  （usage_events/hourly/daily/period/diagnostics/checkpoints/aliases/
  aggregates/runs/quotas），source_files 状态重置 new（下次刷新全量重采）；
  主机/用户/设置保留。UI 红色按钮+确认层。
- **采集进度**：RefreshState 增加 progress_percent（适配器序号/总数）+
  eta_seconds（按已完成适配器平均耗时估算）；顶栏进度条展示。
- **导入覆盖语义**：同修订也替换（原为 skip——重复导出导入时同修订
  不同内容会缺失）。回归测试：同键同修订重复导入（值 100→200→200）
  ⇒ 单行 200，不冗余不缺失不双计。
- **默认用户名**：AppState::init 首次运行时把 v6 的 default 用户重命名为
  OS 当前用户名（USERPROFILE/USER 环境变量推导），来源归属不变（同 user_id）。

## 第五轮修复（2026-09-26 用户反馈）

- **AVG 返回 REAL 类型错误**：小时图 duration 子查询 `AVG()` 返回浮点，
  Rust 读 `Option<i64>` 不匹配——SQL 侧加 `CAST(AVG(...) AS INTEGER)`。
- **小时粒度真正生效**：`query_summary` 的小时分支从 `hourly_usage` 读数据
  （DailyRow 新增 `hour` 字段；标签格式 "YYYY-MM-DD HH:00"）。
- **图表维度分组**：`chart_series` 命令按 总用量/模型/Agent/Agent+模型 分组，
  直接从 `daily_usage` 读（低计算量）；前端维度下拉切换。
- **详情页性能**：v7 迁移加复合索引 `idx_usage_events_instance_time`
  (source_instance_id, occurred_at_ms DESC)；`event_details` 日期范围由请求
  给出（不再 0..now+1d 全量扫描）；page_size 上限 500。
- **诊断日志查询**：`diagnostic_logs` 命令（设置页日志 Tab）；
  v7 加 `idx_diagnostics_created_code` 索引。

## 数据库简化（2026-09-26 用户决策：预发布阶段）

- 删除全部 7 个增量迁移（v1–v7），改为**单一全量建库 SQL**（FULL_SCHEMA）。
- 打开逻辑：user_version 匹配 → 正常使用；不匹配 → SchemaMismatch 错误 →
  应用层弹原生对话框（rfd）"数据库版本不兼容，是否删除重建？"
  → 用户允许则删除 .sqlite/.sqlite-wal/.sqlite-shm 后重建；不允许则退出。
- 同时修复 tauri.conf.json 的 dragDropEnabled（从 app 级移到 window 级，
  解决 HTML5 拖拽事件被 WebView2 拦截的问题）。
- 旧迁移相关测试（migration_v16/review_regressions/multi_user_import/v28）
  重写为简化版：建库/幂等/PRAGMA/版本不匹配拒绝。

## 第六轮修复（2026-09-26 用户反馈）

- **kimi-code 兼容残留清零**：注册表更新后 active_compat 文件的游标已推进，
  unchanged 短路跳过重新检测——`run_refresh` 开头主动清除 active_compat
  文件的游标与状态（重扫幂等，事件去重保证不双计）。实测 2→0。
- **窗口大小自适应**：`setup` 回调读主显示器分辨率，窗口取 72%×78%
  （clamp 900–1600 × 600–1000）。
- **前端**（子代理）：筛选器移到历史趋势面板、时区纯下拉、导出多选
  checkbox、图表 legend/轴间距修复、维度分段选择器（非下拉）、token
  曲线图（非柱状）、面板 resize 把手、详情页 loading 骨架。

## 第七轮修复（2026-09-26 用户反馈）

- **操作日志**：settings_changed / import_completed / export_completed /
  manual_cleanup / clear_all_data / scan_completed 写入诊断表（白名单消息）；
  `diagnostic_logs` 增加 code_filter 参数；前端日志 Tab 加 code 过滤下拉。
- 后端同时：kimi-code active_compat 重扫（run_refresh 开头清除游标与状态）。

## 第八轮（2026-09-26 用户需求）

- **tooltip 修正**：上一轮 hideDelay:3000 是错误方向（离开后残留 3 秒）。
  改为 `hideDelay: 0, showDelay: 0`（hover 立即显示，离开立即消失；
  axis trigger 模式下鼠标在图表内持续显示）。
- **主题系统**：AppSettings 加 theme 字段（system/light/dark）；
  themes.css 定义 CSS 变量三套（data-theme 属性切换 + prefers-color-scheme
  media query）；设置页主题下拉；全局样式/组件卡片/图表 ECharts 主题感知。

## 图表粒度修复与时间点汇总（2026-09-26 用户反馈）

- **分组维度粒度不生效（缺陷）**：`chart_series` 旧实现固定从 `daily_usage`
  按 `local_day` 分组，忽略请求的 granularity 与 filters——切小时/周/月后
  仅"总用量"（走 `query_summary`）变化，按模型/按Agent/按Agent+模型仍是日序列，
  且 Agent/模型筛选对分组序列完全无效。重写：周期分组键提取为 `period_key_of`
  （`query_summary` 与 `chart_series` 共用，保证两种视图时间轴标签一致）；
  小时粒度读 `hourly_usage`（标签 "YYYY-MM-DD HH:00"），日/周/月读
  `daily_usage` 按日历周期聚合，周/月并入 `period_usage` 物化周期
  （日层已覆盖的标签不重复计入，与 query_summary 同口径）；筛选
  （Agent/provider/model/实例白名单）经同一 `Filters::matches` 生效；
  组内 token 合并语义同 SQL SUM（全部未知保持 null，不补零）。
- **总览历史趋势时间点汇总（新功能）**：点击历史趋势图（调用图或 token 图）
  的数据点，历史分区上方出现该时段汇总卡（结构同今日汇总）：总调用、
  输入 token（总量 + 命中/未命中分解）、输出、总 token、缓存命中率、
  会话数；周/月粒度附加活动天数。卡片下方附该时段的模型/Agent 占比
  饼图（chart_series 按维度拉取后按标签过滤，total_tokens 占比，与趋势页
  饼图同口径；SharePie 加可选 height 参数，选中态用 190px 紧凑高度）。
  再次点击同一点或"清除"按钮取消；切换粒度/范围/筛选自动清空选择。
  汇总数据直接取 `summary.periods` 按标签回查（分组模式下点击经
  chart_series 标签与 periods 对齐，无需额外查询）；饼图数据在选中后
  懒加载（两个 chart_series 调用，聚合表直读）。
  图表点击经 ECharts click 事件上报原始标签（总用量/分组两模式均生效）。
- 回归测试：`tests/chart_series_grouping.rs` 5 例——小时粒度读 hourly 且
  标签与 query_summary 一致、周粒度并日成周、月标签、模型/实例筛选、
  物化周期并入 + covered 去重 + 实例白名单过滤物化行。
- 验证：`npm run verify`（仓库根）退出码 0（svelte-check 0 错 0 警、
  fmt/clippy 通过、cargo test 全绿含新增 5 例、vite build 798.75 kB）。
  i18n 新增 4×2 键（cards.activeDays、overview.periodSummary[.clear/.hint]）。

## 闪烁修复、x 轴点击与设置默认项（2026-09-26 用户反馈）

- **总览页周期性闪烁（缺陷）**：根因一，`pollRefresh` 每 3 秒轮询且条件
  `!running && last_finished_ms > 0` 在首次刷新后恒真——每 3 秒重查
  summary/sources，`notMerge` 整图重建全部图表；根因二，重查结果无条件
  赋值 `summary`，数据未变也触发派生重算。修复：
  1. 只在采集结束转换（running→结束）或完成时间变化（计划任务/headless
     触发的采集结束）时重查数据；空闲轮询仅更新状态。
  2. `loadSummary` 加查询键+数据修订守卫：均未变化时保留现有对象引用，
     图表/派生不重算；用户切换/导入/设置保存路径强制重载。
  3. 状态轮询间隔自适应：采集中 3 秒（进度条/ETA），空闲 10 秒。
- **顶部自动刷新间隔（新功能；默认后改 5 分钟）**：顶栏新增"自动刷新"下拉
  （关闭/30 秒/1 分/2 分/5 分/10 分），定时重查界面数据；localStorage 持久化。
  默认值初为 60 秒，同日按用户要求改为 300 秒（5 分钟），存储键升级
  `llm-usage-auto-refresh-v2` 使旧默认的存量记录回落新默认。
  与后台采集间隔（默认 3600 秒）相互独立。
- **图表增量更新**：四图表组件（CallsChart/TokenChart/TodayHourly/SharePie）
  改为结构签名渲染——签名（子图/维度/标签/系列名/主题/语言）相同 →
  `setOption` 合并更新（ECharts 内部 diff）；结构变化 → `notMerge` 重建。
  数据变化时不再整图重建。
- **x 轴任意位置点击选择时间点**：图表点击从 series 元素事件改为 zr 级
  画布事件：`containPixel('grid')` 限定网格内，`convertFromPixel` 像素 →
  最近类目索引，不要求命中数据点；legend/轴外区域被排除。
- **设置默认项**：常规页/归档保留页各加"恢复默认设置"按钮（仅改草稿，
  保存后生效；常规页不动手工根目录——用户数据源清单不属于偏好默认；
  时区默认取系统值 Intl，采集间隔默认 3600，保留层级默认 7/3/90/1095/
  3650/终身）。间隔字段加"默认 3600（每小时）"提示；修正归档页月层级
  默认天数提示 10950→3650（与后端 RetentionTiers::default 一致，
  原提示为笔误）。
- **时间点汇总"原地替换"**（用户复检反馈：换选时间点仍闪一下）：三个来源
  一并消除——换选时饼图数据先置 null 导致整条塌缩成加载态再撑开；首次
  加载态（一行文字）与饼图行高度不一致造成二次跳动；无数据时段饼图折叠。
  修复：换选保留旧饼图直到新数据到达（stale-while-revalidate，原地换数据）；
  加载占位改为与饼图同尺寸的双骨架卡；饼图卡固定 min-height（空数据态
  不塌缩）；SharePie 增量签名只含名称集合（同名不同值走 setOption 合并，
  饼图原地动画过渡而非重建）。换选时间点现在布局零跳动，汇总卡数值与
  饼图数据原地更新。

## Tooltip 离开隐藏加固（2026-09-27 用户反馈，三轮，浏览器复现定论）

- 现象：鼠标移出图表后 tooltip 不消失（用户两次截图复现）。
- **真根因（第三轮浏览器复现 + ECharts 6.1.0 源码实锤）**：此前定论的
  `hideDelay: 999999 + globalout 手动 hideTip` 方案在 ECharts 6 失效——
  `TooltipView.manuallyHideTip` 内部调用 `tooltipContent.hideLater(
  tooltipModel.get('hideDelay'))`，即**手动 hideTip 动作同样被 hideDelay
  延迟**；999999ms ≈ 16.7 分钟，等于永远不隐藏。之前两轮加的
  globalout/document mouseout/blur/mousemove 兜底事件路径全部正确执行了
  dispatchAction，但动作本身被延迟，故无一生效。用同版本 echarts 6.1.0
  搭独立复现页（build/tooltip-repro，已清理服务）以真实 CDP 鼠标事件
  验证：999999 下 hover→移出→直接 dispatchAction({hideTip})、甚至
  setOption({tooltip:{show:false}}) 均**无法**隐藏可见 tooltip；
  `hideDelay: 0` 下全部路径立即隐藏。
- **修复**：六个图表组件 tooltip `hideDelay: 999999 → 0`（transitionDuration
  保持 0）。行为变化：指针在画布内但网格外（legend/边距）时 tooltip 立即
  隐藏（标准 ECharts 行为），网格内移动仍持续显示。保留
  `setupTooltipAutoHide` 封装（globalout/document mousemove 目标不在本图
  容器/document mouseout 出窗/window blur）——覆盖布局位移（点击选点后
  汇总条展开、画布从指针底下移走，无 mouseout 派发）与 WebView2 漏事件
  路径，hideDelay 0 后这些兜底的 hideTip 立即生效；item 触发图表（饼图/
  热力图）保留 `hideTooltipOnBlank`；两趋势图点击选点后主动 hideTip。
- 验证：浏览器复现页全路径（悬停显示/legend 隐藏/回网格重显/移出隐藏/
  点击选点后隐藏、位移后指针仍在图内则正确重显）；`npm run verify`
  退出码 0。**定论修订：ECharts 6 下 tooltip 不得使用大 hideDelay；
  手动隐藏必须配合 hideDelay 0。**
- 验证：`npm run verify` 退出码 0。i18n 新增 8×2 键（header.autoRefresh.*
  5 个、settings.interval.hint/restoreDefaults/defaultsPending）。
  后端无改动（采集间隔默认值本为 3600，无需变更）。

## 未完成项（显式遗留）

| 项 | 状态 | 后续 |
| --- | --- | --- |
| 真实 Windows 桌面逐操作验收（V13–V18、V23–V25 GUI 部分） | 未执行 | 计划要求"不能仅以 Web DOM 测试证明 IPC 正常"；需实机操作或 GUI 自动化记录 |
| 系统对话框选择导出位置（tauri-plugin-dialog） | 未实现 | 当前写应用数据目录/指定目录；对话框插件随后续迭代（合同要求最终落地） |
| Windows 系统任务注册、跨进程互斥、卸载清理（V24） | 未实现 | headless 路径已就绪；注册/对账另做 |
| 逐源定时（per-source interval/定点） | 未实现 | 当前全局间隔 + 逐源启停；逐源配置待 extraction_schedules 接线 |
| 文件监听触发 | 未实现 | 调度合同允许轮询先行；监听为优化项 |
| 清理影响预览/恢复默认/会话导入/重扫预览 | 未实现 | M6 后续 |
| V20 性能初值（100 万事件查询分位数） | 未执行 | M7 前 |
| kilo/zcode/kimi/kimi-work 适配器未接入 scanner 注册表 | 进行中 | 见下 |

## kilo/zcode 适配器中断记录（2026-09-25）

两个并行实施子代理因 API 使用限额中断（17:12），**未留下源码**，但已完成
脱敏 fixture 提取：`tests/fixtures/kilo/`（2 个真实会话 + 期望值）、
`tests/fixtures/zcode/`（2 个真实 + 9 个合成 + 期望值）。泄漏核查（UUID/路径）
通过；markdownlint 已修。适配器实现待限额恢复后继续（M3/M4）。
M3/M4/M5 本机盘点结论（2026-09-25，build/desktop-usage-validation/
m345-inventory-2026-09-25.md，gitignored）：kilo 495 MB 活跃、zcode 140 MB 活跃、
kimi-code 18.2 MB、**kimi-work 31.5 MB（首次证实真实数据，路径已迁移）**；
Copilot CLI 用量存储一日内消失（疑升级迁移，需重定位）；cline/opencode/mimo/
zoo/dsh/openclaw/hermes/codebuddy 未安装。

## 证据文件

- 后端：desktop/src-tauri/src/{app_state,scanner,commands,main}.rs；
  前端：desktop/src/{App.svelte,lib/api.ts,lib/i18n.svelte.ts,components/*}；
- core：query.rs（agent/hourly/heatmap）、calendar.rs；
- 临时核对库：`C:\Users\owt50\AppData\Local\Temp\llm-appdata-test\`（系统临时目录，可清理）。
