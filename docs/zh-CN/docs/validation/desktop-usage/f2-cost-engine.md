# F2 费用估算引擎实施验证记录（2026-09-30）

<a id="f2-cost-estimation-implementation-checks-2026-09-30"></a>

范围：[价格规范](../../design/desktop-usage/pricing.md)实施任务 1–4、6–7（任务 5
可选在线刷新显式后置）。schema v8、种子快照、估算引擎、日成本回填、汇总查询、
命令与界面、V29 格式测试。不提交、不推送、不部署。
本页保留最初实施；后续[归档与精度修复](pricing-archive-repair.md)已替代逐分项提前舍入、
当前参考仅读取明细等旧行为。现行规则以价格规范为准。

<a id="implemented-features"></a>

## 实施内容

<a id="1-price-schema-task-1-schema_version-78"></a>

### 1. 价格 schema（任务 1；SCHEMA_VERSION 7→8）

`crates/core/src/storage/schema.rs`：`price_snapshots`（快照元数据：来源类型/URL/
检索与核验时间/内容哈希 FNV-1a/许可/核验人）+ `price_versions` 重定义（region/
channel 必填、service_tier、context_threshold_tokens、五档价格列 +
cache_storage 小时价，单位=最小货币单位百分之一/百万 token）+ `daily_cost_usage`
（日成本回填：分 tz/日/来源/模型/币种/kind 滚动，kind =
estimate_at_time / reported / source_estimate；未计价计数与原因直方图记在
currency='' 行；sealed 语义与 daily_usage 一致）。
预发布规则（2026-09-26 用户决策）：不做逐版本迁移，版本不匹配走既有
"备份→重建→重扫"路径（v7 本机库升级时会触发该提示）。

保留清理实现（retention_tiered.rs）：明细过期时封存日成本行（历史金额不改写）；
日层清理时同步清理日成本行（随 daily_days 保留期）。

<a id="2-seed-snapshots-and-import-task-2"></a>

### 2. 种子快照与导入（任务 2）

- `crates/core/prices/seed-2026-09-25.json`：19 行价格，来源=官方页正文
  （pricing.md S01/S03/S07/S09/S10/S15/S17，检索日 2026-09-25）。
  覆盖 openai（gpt-6-astra/gpt-6-sol；GPT-6 长档阈值未官方复核不收录）、
  moonshot 双渠道双币（kimi-k3 / kimi-k2.7-code；CN 站缓存写分项未公示不建行）、
  zhipuai bigmodel（GLM-5.3/5.2/5.3-Flash 限时五折/5.1 两档；缓存存储限时免费
  政策性记 0 并标注）、zai 国际（含 $0.075/M 的 flashx 缓存读——非整分值，
  百分之一单位可表达）、anthropic（尚未确认模型 ID 与本机记录对应，注释标注）、
  google（3.8 Flash 2026-12-31 前价 + 2027-01-01 起调价预登记为未来区间行；
  2027 行仅输入价已公示，其余分项 NULL）。
- 校验（pricing.rs `PriceSnapshot::from_file`）：格式标记、币种枚举
  （USD/CNY）、服务档枚举、价格非负、生效区间合法、同键区间不重叠
  （to=NULL 视为开放，其后不得再有同键行）、行内 price_id 唯一、
  seed/community 必须带来源 URL。
- 重复导入不重复新增（A10）：同快照 ID 同内容哈希跳过；同 ID 异内容报错
  （修正须换快照 ID）；price_id 跨快照冲突拒绝。应用启动
  （AppState::init）导入种子时不重复新增，失败不阻塞采集。

<a id="3-estimation-task-3-pricingrs-pure-functions"></a>

### 3. 估算引擎（任务 3；纯函数 `pricing.rs`）

匹配顺序：provider（casefold）→ 用户显式选定的 (region, channel)
（未配置 ⇒ channel_unknown 不套价）→ model（canonical 优先，casefold 精确，
不做别名推断）→ standard 档（batch/flex/fast 不串用，A4）→ 半开生效区间
（at_time 按事件 occurred_at；current_sim 按评估时点）→ 上下文档
（多档且输入规模未知 ⇒ tier_ambiguous 不猜档；已知则取最大满足档）。

分项计价：未缓存输入（显式值，否则 total−read−write 三者已知时派生）、
缓存读、缓存写（TTL 档由用户默认选定——事件无 TTL 记录，未设默认档则写分量
不计价，A6）、输出。金额 = round_half_up(token × 价格 / 1e8)，i128 中间量，
逐分量四舍五入到最小货币单位后累加。推理子集无价格行、绝不重复计价（A3）。
异常 token（负值、缓存读写合计大于已知总输入）整条拒绝（A8，不用 max(0,…)）。
计价覆盖标记：priced_tokens / known_tokens + has_unknown_components
（输出或未缓存输入未知 ⇒ 部分估算，A2）。

<a id="4-daily-backfill-and-summaries-task-4"></a>

### 4. 日成本回填与汇总（任务 4）

- `recompute_cost_day`（storage/pricing.rs）：只重写未封存日；该日无明细事件
  不重建（历史行可能来自已过期明细）。事件批次后由 scanner 在
  pricing.enabled 时按"本轮数据修订触及的未封存日"自动回填；
  显式重算命令（后台线程）回填全部仍有明细的未封存日。价格快照导入
  不触发重算（既有估算不被后台更新改写）。
- `cost_summary`：按发生时价（持久化日成本行，引用 price_basis 快照集合 +
  data_revision）与来源金额来自 daily_cost_usage；按当前价格模拟即时计算
  仍保留的明细事件并标 detail_limited（明细过期后无法模拟）。多币种分列
  小计，不合并（E8）；未计价原因直方图分列。

<a id="6-commands-and-ui-task-6"></a>

### 6. 命令与界面（任务 6）

- 命令：`cost_summary`、`recompute_costs`（后台线程 + 操作日志）、
  `list_price_snapshots`、`import_price_snapshot`（文件路径手工导入）。
- 设置（AppSettings.pricing，默认关闭）：enabled 开关 + 供应商默认表
  （provider/region/channel/TTL 5m|1h），随设置保存持久化。
- 界面：设置新增"费用"页（开关、提示、快照列表+导入+重算、供应商默认编辑）；
  趋势页新增"费用估算（参考）"面板（未启用时不渲染，布局持久化不动），
  三列分币种展示 + 覆盖/部分/未计价标注 + 快照基础。i18n 新增 26 键 ×10 语言。

<a id="7-v29-acceptance-task-7"></a>

### 7. V29 验收（任务 7）

- 单测（pricing.rs，9 项）：种子解析、负价/区间重叠拒绝、金额舍入
  （987.6536→988、2.8→3、26.2144→26）、渠道未配置不套价、异常拒绝、
  档位选择（E6/E7 数值）、TTL 默认档、派生未缓存输入、快照生效前事件不套价。
- 格式测试（tests/pricing_v29.rs，3 项）：P1–P6 + A4 batch 对照行的完整
  管线（导入→ingest→回填→汇总）；E1（2056 分）/E3（714 美分）/E4（604）/
  E5（312）/E6（29 分）/E7（22 分）逐项断言；A1/A2/A3/A4/A5/A6/A8/A10
  行为断言；E8 双币种分列；A7 按发生时价与按当前价格模拟分列（09-20 事件
  at_time 未计价、current_sim 803 分）；渠道不明不套价；重复导入种子不新增价格条目。
- E2 原示例输入合计 1.3M，达到 P3 的 ≥272K 条件；手工计算为
  2000+40+250+1875=4165 美分。当前 pricing_v29.rs 检查缩小至 130K 的 E2′
  （P2：100+2+13+125=240 美分）与 P2/P3 阈值边界。当前源码没有独立原始
  1.3M 输入用例；不把计算示例描述为已执行测试。pricing.md 已同步。
- 未执行项：A9（在线刷新失败回退）——在线刷新未实施（见缺口）；真实数据
  只读核对——本机已有数据的 GLM/Kimi 事件 provider_id 拼写与渠道归属
  需逐 Agent 核验后才能选定快照行，当前默认全部 channel_unknown 未计价
  （符合"渠道不明不套价"），待用户在设置中配置渠道默认后产生真实估算。

<a id="tests-and-commands-windows-11-x64-rootdesktopsrc-tauri"></a>

## 测试与命令（Windows 11 x64，仓库根/desktop/src-tauri）

| 命令 | 退出码 | 结果 |
| --- | --- | --- |
| `cargo test -p llm-usage-core --lib pricing` | 0 | 9 项通过 |
| `cargo test -p llm-usage-core --test pricing_v29` | 0 | 5 项通过（含分组筛选语义与选日修订号限制） |
| `cargo test -p llm-usage-core` | 0 | 全量 66 个测试目标全绿 |
| `cargo test -p llm-usage-desktop` | 0 | 15 项通过（含时区分区重建） |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 通过 |
| `cargo fmt --all --check` | 0 | 通过 |
| `npm --prefix desktop run check`（svelte-check） | 0 | 0 错误 0 警告 |
| `npm run test:ui` / `test:scripts` | 0 | 8 项 + 脚本测试通过 |

`npm run verify` 完整流程与 `npm run test:browser` 在本轮改动后重跑（见下）。

<a id="remaining-work-at-this-stage"></a>

## 缺口与后置项（如实登记）

1. **任务 5 可选在线刷新未实施**（默认关闭的功能）：当前仅手工导入/仓库种子。
   官方页无机器可读接口（Moonshot .md 除外），实施需逐渠道解析器；
   A9 场景随该任务执行。
2. **快照新鲜度展示有限**：面板显示 price_basis 快照 ID；fetched_at 在设置页
   快照列表可见，趋势面板未显示日期。
3. **真实数据端到端估算未发生**：需用户配置供应商渠道默认（设置 → 费用）；
   GLM 系本机事件 provider_id 拼写与 bigmodel/z.ai 渠道归属未逐 Agent 核验。
4. **估算行不含缓存存储费**（cache_storage 小时价列已建、引擎未计价）：
   事件未提供存储时长，计费需要存储时间序列，后置。
5. usage_observation（Hermes 形态区间汇总）不参与计价：区间汇总无输入拆分，
   仅 total 不能套价（数据规则），如实排除。
6. schema v8 升级会使既有 v7 本机库走"备份→重建→重扫"提示路径（预发布规则）。

<a id="review-repairs-second-round-2026-09-30"></a>

## 复核修复（2026-09-30 第二轮）

首轮实施后的自查发现并修复三处缺陷，均补测试或修正文档：

1. **扫描后回填选不到日（功能性）**：费用回填原先按"当前 data_revision ="
   过滤，但分级保留在扫描后会再 bump 修订号，过滤恒为空 → 自动回填从不
   触发。改为在刷新起点捕获 `revision_before`、经 `cost_backfill_days_since`
   （core 可测方法）按 `data_revision >` 选择本轮重写的未封存日；新增测试
   `v29_backfill_day_selection_uses_revision_floor` 固化语义（含封存日不入选、
   无新写入为空集）。
2. **分组筛选空值语义错误（功能性）**：cost_summary 与
   collect_events_for_pricing 的 provider/model 筛选原先无条件包含空值事件
   （`OR provider_id = ''`），与 query::Filters 的 "unknown" 语义不一致——
   筛选具体供应商时不应计入无供应商事件。改为共享 `fold_filter_condition`：
   casefold 后匹配允许值；仅当筛选值含 "unknown" 时空值/NULL 命中。
   新增格式测试 `v29_dimension_filters_do_not_include_empty_values`
   （openai 筛选 9500 美分不含无供应商事件；unknown 筛选命中 no_provider
   未计价；glm-5.3 模型筛选 3600 分）。
3. **TTL 下拉绑定与设置校验**：select 原用 `bind:value={null}` 依赖运行时
   类型保真，改为字符串 onchange 显式转换；`set_settings` 增加 F2 校验
   （供应商默认字段非空、TTL 仅 5/60 分钟），非法值拒绝保存。

配套完善：设置保存且费用启用时自动触发一次后台重算（重复执行结果稳定；否则启用后
既有明细需等下一次采集才回填）；时区重建清理其他时区的未封存日成本行
（封存历史保留，与 daily 分区语义一致）；费用面板在"全部未计价"时也显示
未计价计数与原因（原先被"暂无数据"遮蔽）。

文档同步：pricing.md 快照格式节改为实施定案并补导入 JSON 示例与校验规则；
validation.md V29 执行状态两处更新；README.md（范围映射）费用两行更新；
i18n.md 键数 292 → 329（M6 基线 292 + 后续递增，F2 新增 26）。

<a id="full-checks-rerun-after-changes-2026-09-30"></a>

<a id="门禁补跑2026-09-30本轮改动后"></a>

## 统一检查补跑（2026-09-30，本轮改动后）

`npm run verify`（仓库根）退出码 0：markdownlint、资产检查、脚本/UI 测试、
svelte-check（0 错误 0 警告）、cargo fmt/clippy（-D warnings）、Rust 全量测试、
vite 构建全通过。`npm run test:browser` 退出码 0（日历热图、十语言、主题、
时区、过期响应、用户隔离、成员、分页、空闲轮询检查通过）。
