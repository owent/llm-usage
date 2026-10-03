# 看板交互与查询性能验证

2026-10-02，Windows 11 x64；用户授权的第二轮看板修复。
依据 [交互合同](../../design/desktop-usage/dashboard-polish.md)、
[可观测性合同](../../design/desktop-usage/copilot-otel.md) 和
[价格合同](../../design/desktop-usage/pricing.md)。保留本轮开始前全部用户及第一轮修改。

## 实施结果

| 用户问题 | 修复与证据 |
| --- | --- |
| 总览提示太大 | 一行状态和两个操作；必要导出说明放标题提示及详情。Edge 1440 宽下高度小于 70 px |
| 一键配置后仍报 5 项待开启 | 只计 missing 且 configurable；已配置待数据、有效数据、配置冲突分开，不用过往批次文案盖掉实时状态 |
| 详情需手工核对 | 只读核验实际输出路径；已有有效调用自动核验，尚无输出等待数据，非有效样本继续自动检查。策略冲突与数据证据独立 |
| 配置多空行 | JSONC 插入复用已有换行，保留注释/BOM/LF/CRLF，重复合并幂等；不重写用户已有空白 |
| 费用缺少明细 | 单次查询提供各 provider/model 发生时估算与当前价模拟、币种小计、日金额；不合并货币，不改历史金额 |
| 模型表过窄 | 今日和趋势模型表全宽，逐模型两列费用、底部统计与币种汇总，表头可换行；桌面截图/scrollWidth 核验无横向滚动 |
| 没费用曲线 | 发生时估算按币种显示日/周/月曲线，可切换逐模型；小时用量下费用保留日粒度，未知金额不补零 |
| 周分布慢 | 定位同步费用请求和逐事件遍历完整价目表；后台线程查询、按型号缓存候选价目、日汇总 SQL 分组、日/小时和明细 SQL 提前筛选 |
| 曲线/饼图拥挤 | 默认用量/调用/周分布全宽，两饼图各半宽；圆环按尺寸计算、滚动图例独立置底；无金额时曲线折叠 |
| 点击后汇总不变 | 曲线点、x 轴标签、dataZoom 范围更新顶部汇总；真实日期/本地小时筛选，跨周期会话去重，过期响应丢弃，可恢复全部范围 |

## 本机只读证据

检查未修改真实 IDE 配置、原始 Agent 输出或用户数据库，未产生模型调用。
探测脚本、脱敏计数、SQL 查询计划和性能日志放忽略目录 build/dashboard-polish/；
统计查询使用第一轮隔离数据库副本，真实原始数据不进版本库。

| 本机目标（Profile 身份不输出） | 配置状态 | 数据证据 |
| --- | --- | --- |
| Copilot · VS Code 主实例 | configured | verified，30 条有效记录 |
| Copilot · Agent Host | configured | waiting，0 条 |
| Copilot · Profile | configured | waiting，0 条 |
| Copilot · Profile Agent Host | configured | waiting，0 条 |
| Codex | missing | waiting，0 条 |

因此按本次只读核查应显示 1 项待开启、3 项已配置待数据、1 项已核验，不能把目标
总数都列为待开启。未获取用户旧进程的原始 IPC 响应，不据此推断当时各目标的具体状态。
核验有效记录不等于全部历史完整，也不能证明未识别客户端的数据。共享接收文件按客户端
事件身份检查；多个无法区分的 VS Code Profile 指向同一文件时不分别宣称核验成功。
外部端点保留，无法关联到本机输出时仍显示待关联，不用旧数据消除配置冲突。

主导出文件约 14.1 MB，有效调用在前段而尾段只有其他日志。尾部无匹配时兼查前部，
避免误报“无有效数据”；每样本最多 2,000 行，尾 2 MiB/前 8 MiB、单行 256 KiB、
读取时间预算 200 ms。只读真实检查整体约 0.72–1.50 秒（并发构建时较慢），在后台线程执行。
未变化文件的证据缓存有上限，文件追加使其失效；页面每 30 秒自动检查。

## 性能与查询计划

同一副本：usage_events 3,369 行，daily_usage 22、hourly_usage 60、period_usage 4、
daily_cost_usage 20；查询 Asia/Shanghai，2026-09-03 至 2026-10-02。
Rust debug，同一示例各执行三次；数字为本机证据，不作其他规模或 release P95 承诺。

| 查询 | 修复前 | 修复后 |
| --- | --- | --- |
| 费用（含当前价模拟） | 1,950–2,162 ms | 106–116 ms |
| 日分布/周分布底层 | 热 0.71–1.31 ms | 热 0.54–0.61 ms；首次约 10 ms |
| 汇总（包括会话/耗时） | 35–41 ms | 34–38 ms |

周分布本身没有秒级查询。旧费用请求在 Tauri 同步命令中遍历价目全表，约 2 秒延迟
会阻塞其他 UI 请求；现在费用、汇总、日分布分别在 spawn_blocking 中使用只读连接。
界面共用一次费用结果供面板/表格/曲线；不在每个模型行重复调用 API。
价格缩小候选保留全部供应商、快照顺序、有效期和上下文档位，不缓存某次估算结论。
费用曲线按日期二分定位周期，避免逐模型逐日遍历全部周期。

只读 EXPLAIN QUERY PLAN（Python SQLite 3.50.4）结果：

| 路径 | 计划 |
| --- | --- |
| 日期受限日汇总及筛选 | SEARCH daily_usage，主键前缀 tz_version/local_day |
| 日期受限小时汇总及筛选 | SEARCH hourly_usage，主键前缀 tz_version/local_day |
| 会话/耗时及即时计价明细 | SEARCH usage_events，idx_usage_events_occurred，双端时间范围 |
| 每日模型费用 | SEARCH daily_cost_usage，idx_daily_cost_day；GROUP BY 使用临时 B-tree |
| 归档与日层修订比较 | SEARCH period_usage，idx_period_usage_range；相关子查询 SEARCH daily_usage，日期主键前缀 |

现有副本未见无边界全表扫描；分组临时 B-tree 不等于缺失比较索引，本轮不新增索引或
迁移。日分布读日表，不用原始消息求和；归档不能分配到日时仍留覆盖缺口。
日/小时聚合和留存明细的 Agent/provider/model/实例筛选已下推 SQL。
归档周/月的比较仍受周期日期边界限制，并保持现有替换和修订规则。
依据 [SQLite 查询计划](https://www.sqlite.org/eqp.html) 与
[查询优化说明](https://www.sqlite.org/queryplanner.html) 核查 SEARCH/范围及前缀，
未执行原库 ANALYZE、加索引、清库或改写历史。

## 验证命令与范围

Node.js 24.21.0、Rust/Cargo 1.98.1、已安装 Microsoft Edge；依赖版本以工作区锁文件为准。

| 命令（仓库根） | 结果 |
| --- | --- |
| cargo test --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core --test dashboard_polish | 退出 0；3 项：小时范围/去重/字段未知、DST 重复小时、字节预算/半行续读 |
| cargo test --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core --test pricing_v29 | 退出 0；10 项，追加各模型/日金额与币种汇总一致性断言，包含多币种/筛选/归档/官方回退 |
| cargo test --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-desktop telemetry_setup | 退出 0；37 项、2 忽略，新增换行、证据/缓存失效/配置冲突和共享 Profile 测试 |
| cargo test --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-desktop local_export_verification -- --ignored --nocapture | 退出 0；只读真实输出 5 项状态及 30 条有效记录 |
| cargo run --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core --example benchmark_dashboard | 退出 0；只读真实副本三次计时 |
| python build/dashboard-polish/query_plans.py | 退出 0；只读计划和表行数 |
| npm run test:browser | 退出 0；Edge 模拟 IPC：配置批次部分失败/重试、定期数据核验、金额/曲线/全宽布局、点选/缩放/小时/旧响应/恢复、多语言及既有回归 |
| npm run verify | 退出 0；Rust 793 通过、3 忽略；UI 13、脚本 3，Markdown/资源、svelte-check、fmt/clippy 和前端构建均通过 |
| npm run build:desktop | 退出 0；Windows x64 release 应用和 NSIS 安装包已生成，安装包 3.57 MiB；未安装或启动原库 |
| npm run lint:md | 退出 0；168 文件、0 问题 |
| python build/dashboard-polish/check_links.py | 退出 0；7 个受影响文档、129 个相对链接，0 缺失 |
| git diff --check | 退出 0；只有工作区既有 LF/CRLF 提示，没有空白错误 |

浏览器使用合成数据，不等同真实 Tauri IPC 服务或实际 IDE 重启验收。
宽屏表格、金额和曲线已检查截图；窄屏、多语言和主题沿用浏览器回归。
缩放/事件边界查阅 [ECharts 官方事件定义](https://github.com/apache/echarts-doc/blob/master/en/api/events.md)，
并核对安装源码和实际 ECharts 实例；没有依赖不明来源教程。
验证中发现字节预算停止未报告待续读半行，保留失败日志并修复后重跑。

新应用在 desktop/src-tauri/target/release/LLMUsage.exe，安装包在同目录
bundle/nsis/LLMUsage_0.1.2-dev_x64-setup.exe。构建成功不等于安装或真实 GUI/IPC
验收；本轮未启动新 GUI、未替换正在运行的旧进程。

不将未配置 Codex、等待导出的目标、CLI/JetBrains 新版本、其他未核验导出载体宣称为
已完整验收；没有修改价格快照、启用在线刷新或清理旧游标/历史。

## 总览暂无数据提示补充（2026-10-02）

按用户追加要求，总览单独统计“待开启 / 暂无数据 / 已核验”。缺配置且可自动配置的
项目保留待开启；其余尚无已核验有效数据的项目显示暂无数据，包括配置暂受限制的项目。
总览不固定显示零项“配置受限”；已有有效数据但仍有配置限制时保留限制提示。
详情继续分别展示配置状态、原因和数据证据，批量配置仍跳过受限项目。
同步十种语言及交互规范；本次仅修改前端展示，未改变采集、只读核验、数据库或用户配置。

Windows 11 x64，同前述 Node.js、Edge 和锁文件环境；日志放 build/telemetry-no-data/。

| 命令（仓库根） | 结果 |
| --- | --- |
| npm run test:ui | 退出 0；14 项，包括十种语言键与占位符一致性 |
| npm run check | 退出 0；Svelte 0 错误、0 警告 |
| npm run test:browser | 退出 0；Edge 合成 IPC 核验未收数据、数据到达/消失和真实策略限制；原有交互回归通过 |
| npm run build:web | 退出 0；生产前端构建通过 |
| npm run lint:md | 退出 0；Markdown 检查通过 |
| git diff --check | 退出 0；无空白错误 |

浏览器断言覆盖总览的受限无数据项目显示“暂无数据”、详情保留策略限制、有效数据
自动变为已核验且不消除真实限制，以及后续无有效数据恢复暂无数据。未重建桌面安装包，
未启动原生 GUI；浏览器证据不代替真实 IDE 生效或完整历史验收。
