# 价格归档、精度与模型合并修复验收

日期：2026-10-05。环境：Windows x64、Node.js 24.21.0、Rust 1.98.1。
实现与合同见 [价格](../../design/desktop-usage/pricing.md)及
[看板修正](../../design/desktop-usage/dashboard-repair.md)。

## 根因与修正

| 问题 | 已确认根因 | 修正 |
| --- | --- | --- |
| 大量历史 token 只显示 $0.01 | 用量表读封存日汇总，当前价只遍历尚存 usage_events；清理明细后费用范围缩小 | 复用用量查询的归档选择；明细/归档按实例、Agent、原始 provider/model、调用分类、质量和时间互斥 |
| 部分同模型缺价 | 有价模型的事件已归档；GPT 官方快照 ID 和备份/自定义路由前缀也未解析 | 恢复归档计价、增加有依据的别名，并仅剥离匹配该记录 provider 的自定义命名空间 |
| 小额费用丢失 | 每条事件的各分量提前舍入到分，再累加 | 保留 i128 整数乘积，按展示日/provider/模型/币种或不可拆分归档周期累计后舍入 |
| 相同模型重复行 | 单价界面每个档位渲染一条模型行，且显示参考供应商而非来源 provider | 每个来源 provider/模型一行，多个档位及快照在行内展示；大小写/分隔符及 provider 外围空白一致归组 |
| 新旧档位拼接 | 价目候选跨快照选最高阈值，可能把旧长档与新基础价混用 | 先确定优先快照，再在该快照内部选档 |

当前参考在同一 SQLite 读事务中取得用量、价格与修订，不修改真实源数据或历史封存金额。
发生时估算规则标记更新为 `official-reference-5`，只修正保留明细的未封存日期一次，
后续价目更新仍不重写历史。小时封存判定修正为完整来源分区，活跃的其他模型不再
遮蔽同日已归档的模型。周/月归档复用用量选择规则，不虚构日曲线点。

归档已不包含逐次上下文长度：固定单价可直接计算；多档价显示已知分项的上下界。
旧小时/周/月汇总未保存 uncached 分项时保留部分估算，不能用不同样本的输入/缓存
总和相减。已知总量仍计入覆盖分母，不能把仅输出已计价显示成完整覆盖。
跨动态别名变更日期的不可拆分归档保持型号歧义。

## 官方依据

2026-10-05 核对官方正文。下表为标准 API 每百万 token 的 USD 参考，不是订阅实付。

| 型号 | 未命中输入 / 缓存读 / 输出 | 依据及处理 |
| --- | --- | --- |
| gpt-5.5、gpt-5.5-2026-04-23 | 5 / 0.5 / 30 | [官方模型页](https://developers.openai.com/api/docs/models/gpt-5.5)明确列出快照身份及长上下文倍率 |
| gpt-6-sol | 2 / 0.2 / 10 | [官方模型页](https://developers.openai.com/api/docs/models/gpt-6-sol)另列缓存写 2.5；长上下文输入/缓存 2 倍、输出 1.5 倍 |
| glm-5.3-flash | 0.15 / 0.03 / 0.50 | [Z.ai 官方价格](https://docs.z.ai/guides/overview/pricing)，复核既有种子价 |
| glm-5.3 | 1.4 / 0.26 / 4.4 | [Z.ai 官方价格](https://docs.z.ai/guides/overview/pricing)，复核既有种子价 |
| k3、k3-256k | 3 / 0.3 / 15 | [Kimi Code 模型表](https://www.kimi.com/code/docs/en/kimi-code/models.html)确认 K3 身份；[Kimi API 价格](https://platform.kimi.ai/docs/pricing/chat)及其 Markdown 正文列出 K3 单价 |
| k28-agent-preview | 替代参考 0.95 / 0.19 / 4.00 | 用户确认其为 K2.8 Preview，并明确授权缺价时参考 K2.7；采用 Kimi 官方 kimi-k2.7-code 标准 API 价，界面明确标替代型号 |

新增 `seed-2026-10-05` 保存 GPT-5.5/GPT-6 Sol 完整短/长档，从核验日起生效，
不覆盖旧快照；官方条件为输入 **大于** 272000，整数阈值用 272001。
未知 GPT 日期后缀、未知命名空间、渠道/币种冲突仍拒绝套价。
`kimi-for-coding` 作为 provider 不改变 `k3-256k` 的模型身份；
它作为 model 的日期别名与此不同，不能混为同一条映射。

## 本机只读复算

在只读 SQLite 事务中提取模型、来源类别、日期、token 分项、质量及计数白名单；
实例 ID 哈希脱敏，无提示词、源文件路径、账号秘密或原始会话文本。
146 条日汇总导入临时内存库，在副本中模拟全部封存，覆盖 26 个 provider/模型组。
临时数据、脚本、独立计算结果与日志均留在忽略的 `build/pricing-root-cause/`。

- `kimi-code/k3-256k`：本机封存 token 992,415,571，尚存明细仅一条 9,080 token。
  原查询只算这一条，确实舍入为 USD 0.01。
- 按 Asia/Shanghai、2026-01-01 至 2026-10-05 的保留日汇总复算，该组合覆盖
  12 个有数据日期、992,424,651 token。新引擎结果 USD 409.65，与独立整数乘法
  及相同日分区舍入完全一致。这个范围不等同于用户截图中未明确的选区。
- `kimi-code-owent`、`kimi-for-coding` 下的 k3-256k，以及
  `kimi-code/k3-256k`、`kimi-for-coding-backup/k3-256k` 均匹配 K3 官方参考。
  GLM-5.3/Flash 各本机 provider 分组恢复参考价格；GPT 快照与 Sol 归档显示适用区间。
- GPT 的部分 Copilot 记录缺可计价 token，仍保留 `no_known_usage`；
  有价格不等于缺失 token 可以补零或补全。未核验的其他型号保持原有缺价。

## 执行结果与限制

| 检查 | 退出码 | 结果 |
| --- | --- | --- |
| `npm run verify` | 101 | 文档、资源、脚本 3 项、界面 21 项、Svelte、fmt 全部通过；进入根工作区 Clippy 时依赖解析失败 |
| `cargo test --manifest-path build/pricing-root-cause/rust/crates/core/Cargo.toml --offline --no-fail-fast` | 0 | 隔离核心 804 项通过，无失败/忽略；含新增价格 10 项 |
| `cargo clippy --manifest-path build/pricing-root-cause/rust/crates/core/Cargo.toml --offline --all-targets -- -D warnings` | 0 | 隔离核心无警告 |
| `cargo run --manifest-path build/pricing-root-cause/rust/crates/core/Cargo.toml --offline --example live_price_review` | 0 | 脱敏日汇总复算与独立期望一致 |
| 临时桌面副本 `cargo check --workspace --all-targets --locked --offline` | 0 | 桌面命令层及核心编译检查通过 |
| 临时桌面副本 `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` | 0 | 全工作区无警告 |
| 临时桌面副本 `cargo test -p llm-usage-desktop --locked --offline` | 0 | 应用单元测试 95 项通过，6 项显式外部/原生测试按既有标记忽略；合计 Rust 899 项通过 |
| `npm run build:web` | 0 | 前端生产构建通过 |
| `npm run test:browser` | 0 | Edge 模拟 IPC 回归通过：单模型多档一行、区间/上界曲线、窄窗无页面溢出，以及原有交互 |
| `git diff --check` | 0 | 无空白错误；工作区无未跟踪临时产物 |

根锁文件引用的 `foldhash 0.2.1` 在当前索引/缓存不可取得；这发生在产品编译之前。
没有修改用户现有依赖或锁文件。为完成业务验证，复制 Rust 源码到根 `build/`
中的独立核心工作区，以相同声明依赖离线解析；与原锁文件相比新增的版本仅为
`foldhash 0.2.0`。这不是原锁文件的完整桌面构建验收。
进一步对照已有差异发现 `core-graphics-types`、`foldhash`、`option-ext`、`powerfmt`
四条第三方记录的版本都从 0.2.0 改成 0.2.1，但校验和没有变化。
仅在临时 `build/pricing-root-cause/app-validation/` 中将四条版本还原为 0.2.0，
保留应用/core 的 0.2.1 及其余锁定记录，完成上表桌面编译、Clippy 与应用单元测试。
这三个命令均附加
`--manifest-path build/pricing-root-cause/app-validation/Cargo.toml --target-dir desktop/src-tauri/target`，
复用已有编译缓存；源工作区的 Cargo.lock 未改变。
本轮未构建 release 安装包或安装新的桌面程序，未执行原生 WebView2/真实 IPC 验收。

自动回归包括：6 亿缓存 token 保留前后金额一致、明细/归档并存不重计、千次小额
调用累计、272000 边界、归档多档上下界、原始分区隔离、小时筛选、周/月归档、
动态别名跨期、未知拆分覆盖、混合旧/新价目和跨 Agent 同 provider/模型合并。
浏览器截图位于 `build/browser-smoke/`，其中 `unit-prices.png` 与
`model-archive-range-narrow.png` 分别核验合并行和归档区间；模拟 IPC 与真实数据
核心复算分别记录，不能合称原生 GUI 验收。

## 用户确认后的 K2.8 替代参考

2026-10-05 用户明确确认 `k28-agent-preview` 是 Kimi K2.8 Preview，并授权没有
官方价目时参考 K2.7。这是本条身份映射与跨型号例外的授权依据，不将其描述为
官方确认的 K2.8 按量价格。Kimi 官方价格 Markdown 正文再次读取成功，
`kimi-k2.7-code` 标准每百万 token：未命中输入 USD 0.95、缓存读 USD 0.19、
输出 USD 4.00，与既有版本化种子一致；不选择 highspeed 的 1.90/0.38/8.00。

身份和价目替代分离：`k28-agent-preview` 保持来源模型，并识别为
`kimi-k2.8-preview`；缺精确渠道/同型号官方行时，当前参考才允许使用
`kimi-k2.7-code` 官方行。本型号价目补入后自动优先；渠道/币种歧义、已有价目
缺分项、未知 token 不通过替代绕过。其他近似型号不匹配；`kimi-for-coding`
仍按日期解析身份，只有已确认指向 K2.8 的日期适用同一参考规则。
发生时估算不使用跨型号替代，不新增历史修正规则或修改原价快照。

替代型号随金额传递至日曲线数据、模型小计与币种总计，金额卡片显示“替代参考”，
模型详情/单价表显示 K2.8 身份和实际采用的 K2.7 Code 价目，十种语言同步。
新增 `pricing_substitutes.rs` 四项回归验证：裸 ID/路由别名、精确价优先、官方
供应商与未知/歧义保护、封存前后金额及替代依据一致。固定样本各一百万未命中
输入/缓存读/输出合计 USD 5.14，金额及身份均保留到汇总。

本次增量验证：隔离桌面副本运行 `pricing_substitutes`、`pricing_archives`、
`model_reference`、`pricing_v29`、`dashboard_repair` 共 42 项通过（退出 0）；
全工作区 Clippy 无警告，Svelte 无错误/警告；界面 21 项、前端构建通过（退出 0）。
浏览器回归通过（退出 0），核验汇总短标记、模型原名、单价身份/替代价分离和窄窗
无页面溢出；截图 `build/browser-smoke/cost-substitute-narrow.png`。夹具初次将费用
模型改为 k28 而用量仍为 GPT，导致关联断言失败；统一两路模型身份后通过，未放松断言。
沿用上文临时副本的四条锁记录修复，源工作区锁文件保持不变。
