# 模型 API 按量价格获取与 token 费用估算方案

状态：费用估算引擎已实施（2026-09-30，[验证记录](../../validation/desktop-usage/f2-cost-engine.md)）：
schema v8 价格表 + 种子快照 + 估算引擎 + 日成本回填 + 汇总查询 + 命令与界面 +
V29 合同测试。可选在线刷新（任务 5）与真实数据端到端估算（待用户配置渠道默认）
后置，见验证记录缺口清单。
调研完成于 2026-09-25；费用口径以 [数据合同](data-contract.md#pricing) 为权威，
本文补渠道证据与本地快照设计。

## 调研范围与方法

优先覆盖本机已观测模型的供应商：OpenAI（Codex 的 gpt-6-astra/gpt-6-sol）、
智谱 GLM（omp/pi/zcode 的 glm-5.3-flash/GLM-5.3，含 glm-5.2 合成样本）、
Moonshot Kimi（kimi-code/kimi-for-coding/k3）、Anthropic 与 Google
（本机暂无数据，但属合同覆盖的接入目标，一并核验）。
渠道比较分三类：官方机器可读接口、官方定价页面、第三方社区目录；另评估手工维护快照。
外部来源检索日期均为 2026-09-25，清单见文末；查不到证据的项目写明"未核实"。
官方页面正文与可 curl 的公开 JSON 是事实来源，搜索摘要只用于定位。
多数官方页无版本号或日期，快照标识以 URL、检索日期与页面可见的
更新标记为准（如 Gemini 页脚 "Last updated"）；社区目录以数据文件内容
哈希特征与仓库推送时间标识。

## 价格获取渠道比较

### 官方机器可读接口

结论先行：四家供应商的公开模型列表 API 均需鉴权且都不含价格字段；
仅 Moonshot 因文档平台（Mintlify）提供与 HTML 同源的 `.md` 与 `llms.txt`
索引，构成官方机器可读价格来源。未发现任何一家提供免鉴权价格 JSON/YAML API。

| 供应商 | 模型列表 API | 响应含价格 | 公开价格机器可读文件 | 证据 |
| --- | --- | --- | --- | --- |
| OpenAI | `GET /v1/models` 需 `Authorization: Bearer` | 否；字段仅 id/created/object/owned_by/shutdown_date | 未发现；定价为文档站 HTML（platform.openai.com 已 301 迁移至 developers.openai.com） | S01、S02 |
| Anthropic | `GET /v1/models` 需 `X-Api-Key` | 否；字段仅 id/display_name/created_at/max_tokens 等 | 未发现；定价为文档站 HTML | S03、S04 |
| Google Gemini | `GET /v1beta/models?key=` 需 API key | 否；Model 资源只有 token 限制等字段 | `publishPriceDocs` 未检索到（S23）；定价页为 HTML，页脚带 "Last updated" 日期 | S07、S08 |
| Moonshot Kimi | 官网未提供公开模型列表端点（未核实） | — | 有：Mintlify 的 `docs/pricing/chat.md`、`docs/llms.txt` 可匿名 curl，与网页同源；CN 站（moonshot.cn/kimi.com）与 global 站（moonshot.ai/kimi.ai）分列 | S09、S10 |
| 智谱 GLM | bigmodel 开放平台未提供公开模型列表端点（未核实） | — | 未发现；定价页为 JS 渲染 SPA，正文需渲染后提取；国际站 docs.z.ai 为静态文档 | S15、S17 |

官方 HTML 页面的可用性限制：OpenAI/Anthropic/z.ai/bigmodel 页面均无日期或版本标记，
无法从页面本身判断数据新旧；Gemini 页有更新日期但抓取会随请求语言出现机器翻译版本
（本轮一次抓取得到波兰语文本，数值仍为美元原文，S07 附注）。
更新频率均未承诺；OpenAI 页面提到 2026-07-30 服务档更名与
2026-11-21 促销截止，说明价格页存在不定期修订（S01）。

### 社区目录

| 目录 | 数据与许可 | 结构要点 | 与官方核对结果（2026-09-25） | 证据 |
| --- | --- | --- | --- | --- |
| models.dev | MIT；仓库 anomalyco/models.dev（自 sst 迁移），推送 2026-09-25；`api.json` 约 4.9 MiB、200+ 提供商 | 按提供商分组；`cost.input/output/cache_read/cache_write`（美元/百万 token）+ `tiers`（按 context size 分档）+ reasoning/tool 能力；含大量 `*-coding-plan` 订阅渠道条目 | gpt-6-astra（10/50/1/12.5、272K 档 20/75）与官方页一致；zai 条目 glm-5.3（1.4/4.4/0.26）与 docs.z.ai 一致；但 zhipuai 条目（bigmodel API 端点）挂的是 z.ai 文档与美元价，缺 CN 人民币价；coding-plan 条目 cost 全 0，是"订阅非按量"占位而非真实价格 | S18、S19 |
| OpenRouter | `GET /api/v1/models` 匿名 200，460 个模型；单位为美元/token 字符串小数；数据许可未核实 | `pricing.prompt/completion/input_cache_read/input_cache_write`，`overrides` 按 `min_prompt_tokens` 表达长上下文档，`:batch` 独立 SKU | gpt-6-astra 与 moonshotai/kimi-k3、z-ai/glm-5.3 与官方一致；z-ai/glm-5.3-flash（0.045/0.14）低于官方 z.ai 价（0.15/0.50），moonshotai/kimi-k2.7-code（0.6562/3.3）低于官方（0.95/4.0）——聚合商自定费率，不能当官方价 | S20 |
| pydantic/genai-prices | MIT；仓库推送 2026-09-25；按提供商 YAML 源文件 + 生成的 `prices/new_data/v2/data.json` 与 JSON Schema | 每模型 `prices_checked` 日期、`pricing_urls` 来源、input/cache_read/output（美元/百万 token）；支持历史价（价格变更前后分段）、分层价、按日变价；提供商文件注明换算与取舍 | kimi-k3（3/0.3/15）与 zai GLM-5.3（1.4/0.26/4.4）与官方一致；zhipuai 文件注明"bigmodel 仅人民币计价，按 1 USD=7.25 CNY 换算，旗舰按最低档收录"——是对 CN 渠道缺口的显式弥补而非官方数据 | S21 |
| libgen pricing | 未找到名为 libgen 的 LLM 价格目录（多轮检索仅命中 Library Genesis 与无关项目） | — | — | S22 |

社区目录共同缺口：不保证实时；缓存写 TTL 档（5m/1h）、批处理档、
订阅渠道与按量渠道的区分随目录而异；聚合商费率条目（OpenRouter 部分模型）
与官方按量价不是同一概念。可作为交叉核对与缺项候选，不能替代官方依据。

### 手工维护本地快照

格式与流程是本设计的主体（见 [版本化价格合同](#snapshot)）：
仓库维护版本化价格 JSON 种子，逐条记录来源 URL、检索日期、生效区间与提取方式；
用户可在界面导入本地 JSON 覆盖或补充；导入经 schema 校验与区间冲突检查。
维护成本集中在旗舰模型调价与新模型发布时点，本机在用模型集合小，
人工核验成本可控。

### 渠道结论与选择

首选手工维护的版本化本地快照：官方缺机器可读接口，社区目录有许可与
结构优势但存在与官方不一致的实测案例。社区目录（models.dev、genai-prices）
用于交叉核对与官方页面缺失时的候选来源，采纳时必须携带社区来源标注；
OpenRouter 仅作参照，不作为快照来源（含聚合商自定费率）。
可选在线刷新仅限拉取公开价格元数据（官方 .md、社区 JSON），默认关闭。

<a id="billing-dimensions"></a>

## 计费维度核验

四家各以本机在用或合同目标模型的价格页为证据；金额单位为每百万 token。
"未核实"表示官方页面未给出该维度说明，不用推测补齐。

### 矩阵总览

| 维度 | OpenAI | Anthropic | Gemini | Kimi | GLM |
| --- | --- | --- | --- | --- | --- |
| 普通输入 | 有 | 有 | 有 | 有（未命中价） | 有 |
| 缓存读 | 有，输入价 1/10 | 有，0.1x（个别模型 0.025x/0.05x） | 有（context caching 价） | 有，未命中价 1/10 | 有（缓存命中价） |
| 缓存写 5m | 有，1.25x 输入 | 有，1.25x 输入 | 无单独写价 | 有，等于未命中输入价 | 无单独写价 |
| 缓存写 1h | 未核实（未见 1h 档） | 有，2x 输入 | — | 有，2x 未命中输入 | — |
| 缓存存储 | — | — | 有，按百万 token/小时 | 未核实 | 限时免费（政策性） |
| 输出 | 有 | 有 | 有 | 有 | 有 |
| 推理输出 | 无单独费率，按输入/输出价计 | thinking 按输出计 | 含在输出价 | reasoning_content 计入输入/输出配额 | 官方未明示（见下） |
| 批处理 | Batch/Flex 五折 | Batch 五折，可与缓存叠加 | Batch/Flex 五折 | BatchJob 六折 | bigmodel Batch 五折；z.ai 未核实 |
| 长上下文阶梯 | 有（短/长两档；GPT-6 阈值页面未标注，目录记 272K） | 4.6+ 无阶梯（1M 标准价） | Pro 系列有 >200K 档 | 未见阶梯 | GLM-5.1 有 32K 档；5.2/5.3 未见 |
| 币种 | USD | USD | USD | CN 站 CNY、global 站 USD（不含税） | bigmodel CNY、z.ai USD |
| 地区/渠道附加 | FedRAMP 与数据驻留端点 +10% | US-only inference 1.1x（4.6+） | 未见（未核实） | 双平台双价 | 双渠道双价 |

### 各家要点与本机在用模型价目

OpenAI（证据 S01，页面无日期；gpt-6-astra/gpt-6-sol 为本机 Codex 观测模型）：
短上下文 astra 输入 $10、缓存读 $1、缓存写 $12.50、输出 $50；
sol $2/$0.20/$2.50/$10；长上下文档翻倍（astra $20/$2/$25/$75）。
页面未标注 GPT-6 长/短档阈值（旧系列标注 272K）；models.dev 与 OpenRouter
均以 272,000 为 gpt-6-astra 长档起点（S18、S20），正式采纳前需官方复核。
Fast 档（原 Priority，2026-07-30 更名）2 倍；FedRAMP/数据驻留端点 +10%
（2026-03-05 后发布模型）。推理 token 页面无单独费率：
"Tokens are billed at the chosen model's input and output rates"。

Anthropic（证据 S03；本机 Claude 暂无数据，为合同覆盖目标）：
Opus 5.5 $4/$20（缓存写 $5/$8、读 $0.20，读为 0.05x）、Sonnet 5 $2/$10、
Haiku 4.5 $1/$5；Sonnet 5 的 $2/$10 促销价已转正（原定 2026-09-01 上调取消）。
Batch 一律五折且与缓存折扣叠加（S06）；thinking token 按输出计费，
`usage.output_tokens_details.thinking_tokens` 是"已计费输出中内部推理"的拆示（S05）；
多轮保留的历史 thinking 块按输入计费（Opus 4.5/4.6+）。

Gemini（证据 S07，页脚 "Last updated 2026-09-24 UTC"）：
3.8 Flash 标准档输入 $0.75（公示 2026-12-31 前价，2027-01-01 起 $1.50）、
输出 $3.75（含 thinking token）、缓存读 $0.075、缓存存储 $0.50/百万 token/小时；
Batch/Flex 五折；2.5 Pro 有 >200K 阶梯（输入 $1.25/$2.50、输出 $10/$15）。
3.1 Pro（preview）两次抓取（英文与机器翻译版本）对 >200K 档的呈现不一致，
正式收录前需复核（S07 附注）。

Kimi（证据 S09–S13；本机 Kimi Code/Work 观测 kimi-code/kimi-for-coding，wire 字段
inputOther/inputCacheRead/inputCacheCreation 与计费维度对齐）：
kimi-k3 CN 站 ¥2（命中）/¥20（未命中）/¥100（输出），global 站 $0.30/$3.00/$15.00
（不含税）；kimi-k2.7-code CN ¥1.30/¥6.50/¥27、global $0.19/$0.95/$4.00。
缓存写单列为新政策（S12）：global k3 写 5m $3.00（=未命中输入价）、1h $6.00（2 倍），
命中续期；CN 站未见对应分项价（未核实）。BatchJob 六折（S13，非五折）。
`reasoning_content` 计入输入/输出 token 配额（S11）。

GLM（证据 S15、S17；本机 omp/pi/zcode 观测 glm-5.3-flash/GLM-5.3）：
bigmodel CN：GLM-5.3 输入 ¥8/缓存命中 ¥2/输出 ¥28（1M 上下文）；
GLM-5.3-Flash 限时五折 ¥0.4/¥0.115/¥1.4（标准 ¥0.8/¥0.23/¥2.8）；
GLM-5.2 同 5.3；GLM-5.1 有 [0,32K)/[32K+) 阶梯；Batch 五折；
缓存存储限时免费。z.ai 国际：GLM-5.3 $1.4/$0.26/$4.4、Flash $0.15/$0.03/$0.50、
FlashX $0.37/$0.075/$1.25；未见批处理与阶梯说明（未核实）。
思考 token 计费官方未明示：能力文档仅说"思考过程会消耗额外的 Token"，
示例 usage 将 reasoning_content 与 content 并列、completion_tokens 合并计数，
无 reasoning_tokens 拆示（S16）——按未核实处理，快照不建推理价，
费用侧推理子集也不得与已包含它的输出重复计价。

### 本机在用模型与价目映射

| 本机观测模型 | 来源 | 按量价存在性 | 处理 |
| --- | --- | --- | --- |
| gpt-6-astra / gpt-6-sol | Codex（ChatGPT 订阅通道） | 存在（S01） | 订阅用量套按量价，标"参考估算" |
| GLM-5.3 / glm-5.3-flash | omp、pi、zcode | 存在，双渠道双币（S15、S17） | 按实际订阅/端点渠道选行；渠道不明时不套价 |
| kimi-code / k3 | Kimi Code/Work、omp | k3 存在（S09、S10） | Coding Plan 用量套按量价，标"参考估算" |
| kimi-for-coding | Kimi Code/Work | 不存在：官方说明其为订阅专属 K2.8 Preview，按量价目表无此 ID（S14） | 未计价；人工别名须另获证据 |
| glm-5.2（合成样本） | kilo fixtures | 存在（S15、S17） | 同 GLM-5.3 双渠道 |

Coding Plan 与按量价是两种计费体系：models.dev 的 `kimi-code-plan-*`、
`zai-coding-plan` 条目 cost 全 0，是"订阅非按量"的占位表示，不是免费价格（S18）；
本设计禁止把 0 当作价格写入快照。

<a id="snapshot"></a>

## 版本化价格合同

### 快照格式

快照 = 元数据头 + 价格行数组。价格行粒度为
"一个计费上下文"：供应商 × 模型 × 地区渠道 × 服务档 × 上下文档 × 生效区间。
实施落库（2026-09-30，schema v8）：`price_snapshots`（快照元数据）+
`price_versions`（价格行，列含 region/channel 必填、service_tier、
context_threshold_tokens、五档 `*_per_mtok_hundredths` 价格列与
cache_storage 小时价），与下表定案一致。

| 计费维度 | 调研期 schema | 实施定案 |
| --- | --- | --- |
| 缓存写 TTL 两档 | 仅一列 cache_write | `cache_write_5m` + `cache_write_1h` 两列；无该档的供应商置 NULL |
| 服务档（batch/flex/fast） | 无 | `service_tier` 列，默认 `standard`；档位价格单独成行，估算只自动匹配 standard |
| 长上下文阶梯 | 无 | `context_threshold_tokens`（该行适用的最低输入 token，NULL=0）；事件按请求输入规模匹配，多档且输入未知不猜档 |
| 缓存存储费 | 无 | `cache_storage_hour` 列（Gemini/bigmodel 用；限时免费记 0 并附政策标注）；引擎暂不计价（存储时长无事件级证据） |
| 价格精度 | 整数最小单位/百万 token | 官方价存在 $0.075/M、¥0.115/M 等非整分值；单位为"最小货币单位的百分之一/百万 token"（i64），如 $0.075/M = 750 |
| 快照出处 | 仅 price_version 字符串 | `price_snapshots` 表（ID、来源类型、URL 列表、fetched_at、content_hash、许可、核验人/方式） |
| 推理价 | 无列 | 不建：四家核验均无独立推理价（GLM 未明示按未核实处理），推理子集并入输出，禁止重复计价 |

种子快照随仓库版本化：`desktop/src-tauri/crates/core/prices/seed-2026-09-25.json`
（应用启动幂等导入）；用户在「设置 → 费用」导入的本地快照同格式（JSON），
来源类型标 `manual`。金额与价格一律不用二进制浮点（沿用
[数据合同](data-contract.md#pricing)）。

**导入文件格式**（`llm-usage-price-snapshot/1`；日期为 ISO `YYYY-MM-DD`（UTC），
区间半开 `[effective_from, effective_to)`，`null` = 开放；价格数字=
最小货币单位百分之一/百万 token，`null` = 该维度无价不套用）：

```json
{
  "format": "llm-usage-price-snapshot/1",
  "snapshot": {
    "id": "manual-2026-10-01",
    "source_type": "manual",
    "source_urls": ["https://example.com/pricing"],
    "fetched_at": "2026-10-01"
  },
  "rows": [
    {
      "price_id": "my-openai-gpt-6-astra",
      "provider_id": "openai",
      "model": "gpt-6-astra",
      "region": "global",
      "channel": "api",
      "service_tier": "standard",
      "context_threshold_tokens": null,
      "effective_from": "2026-10-01",
      "effective_to": null,
      "currency": "USD",
      "input": 100000, "cache_read": 10000,
      "cache_write_5m": 125000, "cache_write_1h": null,
      "output": 500000, "cache_storage_hour": null,
      "note": null
    }
  ]
}
```

校验规则：币种枚举 USD/CNY、服务档枚举、价格非负、区间合法且快照内同键
（供应商/模型/地区/渠道/档/阈值）不重叠、行内 `price_id` 唯一且不与已导入
行冲突；同快照 ID 重复导入须同时匹配价格字段哈希、来源元数据与行注释
才幂等跳过，内容修正必须换快照 ID。
导入的 `provider_id`/`region`/`channel` 需与「设置 → 费用」中配置的供应商
默认一致才参与估算（大小写不敏感）。
跨快照同一计费项允许覆盖：匹配日期与输入阈值后，手工快照优先于种子，
种子优先于社区；同类快照先取较新的 `fetched_at`，再取较晚的导入时间。
不足最低上下文阈值的事件不套该行；输入规模未知且存在高档行时不猜档。

### 生效区间与更新流程

1. 区间为半开 `[effective_from_ms, effective_to_ms)`；同一快照内同键（供应商/模型/渠道/
   档/上下文档）多行不得重叠，导入时校验。事件按 `occurred_at_ms` 落入区间匹配。
2. 更新只新增行：新快照可用较晚的 `effective_from_ms` 覆盖旧快照的开放区间，
   无需修改已导入旧行；
   已公示的未来调价（如 Gemini 2027-01-01、GLM Flash 限时五折到期）预登记为
   未来区间行，到期自动切换。
3. 获取方式：手动导入优先。仓库维护者按官方页面/官方 .md 制作种子并记录检索
   日期；用户可在设置页导入本地 JSON。可选在线刷新默认关闭（与
   [费用及预算默认关闭](data-contract.md#settings)一致），开启后仅 HTTPS GET
   公开价格端点（官方 .md、社区 JSON），请求不携带任何本地用量、主机/来源
   身份、会话内容或账户密钥；不接入远端用量/账单 API。
4. 更新校验：JSON Schema 校验；数值非负、币种枚举、区间合法且不重叠；
   与现快照 diff 预览（新增/收口/价格变化逐条列出）；
   社区来源数据与官方页交叉核对不一致时警告并可拒收。
5. 离线缓存与回退：快照持久化于本地数据库，核心统计与既有估算不依赖网络；
   刷新失败或校验失败保留已验证快照并显示其年龄（fetched_at/verified_at），
   不静默改写任何已产出金额。
6. 历史可复现：估算结果引用用量 `data_revision` 与 `price_version`；
   按发生时价格估算优先，事件时点无适用价格时不得用当前价冒充，
   而是分列"按当前价格模拟"并标注；后台价格更新不触发既有估算重算，
   重算仅在用户显式请求或数据修订时进行并更新引用。查询范围内只要有符合
   当前筛选条件的已封存用量日，当前价模拟就标记明细受限。

## 费用合同细则

以下是对 [费用合同](data-contract.md#pricing) 的落点细化，不改变其权威性：

1. 未计价：必要 token 拆分未知（如仅 total_tokens）、模型无按量价
   （kimi-for-coding）、渠道/档位无法判定、缓存写 TTL 未知时，
   相应分量金额为空并显示"未计价"，不补零，不套平均单价。
2. 部分可计价：可计算分量（如输出已知、输入未知）计入小计，
   附覆盖标记：已计价 token 数 / 已知 token 数；总金额标"部分估算"。
3. 精度与舍入：价格按"最小货币单位百分之一/百万 token"存储；
   单条事件单个计费项金额 = token × 价格 ÷ 1,000,000，i128 中间量，
   四舍五入到最小货币单位后累加；负值、缓存大于已知总输入等异常进入诊断，
   不参与费用计算。
4. 多币种：按币种分组小计，不同币种不直接相加；用户手工设定汇率转换时
   结果标 `estimated` 并记录汇率来源与时点；无汇率时只分列展示。
5. 参考估算：本机订阅通道（ChatGPT/Coding Plan/GLM 套餐）用量套 API
   按量价一律标"参考估算"，展示时注明订阅实付不等于该值；
   缓存节省量同理不得称为实际返款。
6. TTL 未知默认：事件有缓存写 token 但无 TTL 证据（本机 Kimi wire 的
   inputCacheCreation 无 TTL 字段）时默认未计价；用户可为供应商设置默认
   TTL 档（如 5m），设置后按该档计价并标 `estimated`。

## V29 验收样本设计

V29 行见 [验证清单](validation.md)。样本分三部分：固定价格样本（快照）、
人工期望金额、异常场景。价格取自 2026-09-25 官方页证据（S01、S09–S13、S15），
仅为验收用固定值，不代表持续有效；实施时随种子快照一起版本化。

### 固定价格样本

| 价格行 ID | 供应商/模型 | 渠道/币种 | 档 | 阈值 | 输入 | 缓存读 | 缓存写 5m | 缓存写 1h | 输出 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| P1 | glm-5.3 | bigmodel-cn / CNY | standard | — | 800 | 200 | NULL | NULL | 2800 |
| P2 | gpt-6-astra | openai / USD | standard | 0 | 1000 | 100 | 1250 | NULL | 5000 |
| P3 | gpt-6-astra | openai / USD | standard | 272000 | 2000 | 200 | 2500 | NULL | 7500 |
| P4 | kimi-k3 | moonshot-global / USD | standard | — | 300 | 30 | 300 | 600 | 1500 |
| P5 | glm-5.1 | bigmodel-cn / CNY | standard | 0 | 600 | NULL | NULL | NULL | 2400 |
| P6 | glm-5.1 | bigmodel-cn / CNY | standard | 32768 | 800 | NULL | NULL | NULL | 2800 |

单位：最小货币单位百分之一/百万 token（P1 输入 800 = ¥8/M；P2 输入 1000 = $10/M）。
P1 缓存写 NULL 表示该渠道无写价（限时免费政策另以元数据标注，不写 0 价）。

### 人工期望金额

单条事件逐项计价、每项四舍五入到最小货币单位后求和；金额用最小货币单位
（分/美分）。半开生效区间：全部事件设为快照区间内某日。

| 用例 | 事件 token（未命中/读/写/输出） | 匹配价格行 | 期望金额 |
| --- | --- | --- | --- |
| E1 混合输入 | 1,234,567 / 500,000 / 200,000 / 345,678 | P1 | 988+100+968 = 2056 分（¥20.56）；缓存写分量未计价，覆盖标记注明 |
| E2 全维度 | 1,000,000 / 200,000 / 100,000 / 250,000 | P2 | 1000+20+125+1250 = 2395 美分（$23.95）。**注（2026-09-30）**：该 token 量输入合计 1.3M ≥ 272K，按 P2/P3 档位语义（同 E4/E5）应命中 P3 长档；实施测试以两种取值覆盖——缩至 130K 命中 P2（100+2+13+125 = 240 美分）与原量值命中 P3（4165 美分），见 [验证记录](../../validation/desktop-usage/f2-cost-engine.md) |
| E3 TTL 1h | 400,000 / 300,000 / 600,000（1h）/ 150,000 | P4 | 120+9+360+225 = 714 美分（$7.14） |
| E4 阶梯上界 | 输入合计 272,000 / 0 / 0 / 8,000 | P3 | 544+60 = 604 美分 |
| E5 阶梯下界 | 输入合计 271,999 / 0 / 0 / 8,000 | P2 | 272+40 = 312 美分 |
| E6 GLM 阶梯 | 输入 32,768 / 0 / 0 / 输出 1,000 | P6 | 26+3 = 29 分 |
| E7 GLM 阶梯下界 | 输入 32,767 / 0 / 0 / 输出 1,000 | P5 | 20+2 = 22 分 |
| E8 多币种分列 | E1 + E2 事件并存 | P1 + P2 | ¥2056 与 $2395 分列小计，无汇率不合并 |

### 异常场景

| 场景 | 输入 | 期望 |
| --- | --- | --- |
| A1 无按量价 | kimi-for-coding 任意 token | 未计价，金额空；不写 0 |
| A2 部分可计价 | 输出未知、输入已知 | 输入项计价，总额标部分估算，覆盖比例正确 |
| A3 推理不重复计价 | output_reasoning 50,000 ⊂ output_total 200,000 | 仅按 output_total 计一次 |
| A4 档位不串用 | 普通实时调用 | 不匹配 batch 行（service_tier 不一致时不套用） |
| A5 缓存读价缺失 | 缓存读 token 已知、价格行该列 NULL | 该分量未计价，其余照计，覆盖标记 |
| A6 TTL 未知 | 缓存写 token 已知、无 TTL 证据、未设默认档 | 写分量未计价；设置默认档后按档计且标 estimated |
| A7 历史复现 | 事件落在旧价格区间；对照事件落在区间外 | 按发生时价计；无历史价时"按当前价模拟"分列并标注，不混算 |
| A8 异常 token | 负值、缓存大于已知总输入 | 拒绝进入费用计算，进诊断；不用 max(0,…) |
| A9 刷新失败 | 在线刷新网络错误/校验失败 | 保留旧快照，显示新鲜度，既有金额不变 |
| A10 快照不可变 | 同版本快照重复导入 | 幂等跳过；价格修正走新 price_version 行 |

## 后续实施任务清单

实施进度（2026-09-30，[验证记录](../../validation/desktop-usage/f2-cost-engine.md)）：

1. **已实施**：价格 schema 演进（SCHEMA_VERSION 7→8：price_snapshots +
   price_versions 重定义 + daily_cost_usage；预发布合同走备份重建路径）。
2. **已实施**：种子快照（`crates/core/prices/seed-2026-09-25.json`）与校验
   （币种/档位枚举、非负、区间重叠、幂等导入）；官方页提取脚本未做
   （种子为人工制作，脚本属可选工具）。
3. **已实施**：费用估算引擎（区间/渠道/档位/阈值匹配、逐项计价与舍入、
   未计价/部分计价覆盖标记、i128 中间量与溢出防护；"参考估算"标注在界面层）。
4. **已实施**：历史复现查询（按发生时价持久化、按当前价模拟即时分列；
   估算引用 data_revision 与 price_basis 快照集合持久化）。
5. **未实施（后置）**：可选在线刷新（默认关闭合同项；官方页缺机器可读接口，
   实施需逐渠道解析器；A9 场景随该任务执行）。
6. **已实施**：界面费用面板（默认关闭）、币种分组展示、价格管理
   （快照查看/手工导入/显式重算）；预算提醒未做（提醒仅提示不阻止，
   随后续版本排期）。
7. **已执行**：V29 验收（fixtures 化 P1–P6 与 E/A 场景 + 人工期望测试；
   真实数据只读核对待用户配置渠道默认）。

## 风险与限制

- 官方页面普遍无版本/日期标识，快照新鲜度只能以检索时间度量；
  价格静默调整风险由"社区目录交叉核对 + 用户可见快照年龄"缓解，不能消除。
- 渠道/地区差异是主要正确性风险：GLM 与 Kimi 均为双渠道双币种且价差显著
  （kimi-k3 CN ¥20 与 global $3 输入价不可互换）；本机订阅通道的实际端点
  需逐 Agent 核验后才能选定快照行，渠道不明时不套价。
- 订阅专属模型（kimi-for-coding = K2.8 Preview）无按量价，长期未计价；
  若官方日后发布对应价目再行接入。
- 社区目录与官方存在实测偏差（OpenRouter 聚合费率、models.dev 的 zhipuai
  条目混用国际站价），采纳社区数据必须保留来源标注并可被用户识别。
- 限时政策（GLM Flash 五折、GLM 缓存存储免费、Kimi 缓存写单列新政）
  随时可能到期；生效区间行能表达切换，但到期检测依赖人工或刷新。
- OpenAI GPT-6 长上下文阈值、GLM 思考 token 计费、Kimi CN 站缓存写分项、
  z.ai 批处理折扣、Gemini 3.1 Pro preview 阶梯呈现均为未核实或证据不一致项，
  收录前需官方复核。
- 本机未见 OpenAI/Anthropic/Gemini 真实用量（适配器 no_data/not_found），
  对应价格核验只到官方页面层级，未经过本机端到端对账。
- 本调研未实施任何代码、未制作种子文件、未执行在线刷新；
  调研完成不等于已获取实时价格或已具备费用计算能力，验收以 V29 执行记录为准。

## 调研记录

环境：Windows 11 x64、Git Bash、curl/jq 直接抓取公开 JSON 与 Markdown、
WebFetch/web reader 抓取官方页面正文。检索日期均为 2026-09-25；
status=研究证据，非运行验收。

| ID | 来源 | 采用的事实 |
| --- | --- | --- |
| S01 | [OpenAI 定价页](https://developers.openai.com/api/docs/pricing)（platform.openai.com/docs/pricing 301 迁移；页面无日期） | gpt-6-astra/sol/luna 与 gpt-5.6 系价格、短/长档、Batch/Flex 五折、Fast 2 倍（2026-07-30 更名）、FedRAMP/驻留 +10%、无推理单独费率、促销期标注 |
| S02 | [OpenAI models API](https://developers.openai.com/api/docs/api-reference/models/list) | Bearer 鉴权；响应无价格字段 |
| S03 | [Anthropic 定价页](https://platform.claude.com/docs/en/about-claude/pricing)（无版本标识） | 各模型输入/输出/5m/1h 缓存写/缓存读价、Sonnet 5 促销转正、4.6+ 无长上下文档、US-only 1.1x |
| S04 | [Anthropic models API](https://platform.claude.com/docs/en/api/models-list) | X-Api-Key 鉴权；响应无价格字段 |
| S05 | [Anthropic extended thinking](https://platform.claude.com/docs/en/build-with-claude/extended-thinking) | thinking token 按输出计费；thinking_tokens 为已计费输出拆示；保留 thinking 按输入计费 |
| S06 | [Anthropic batch](https://platform.claude.com/docs/en/build-with-claude/batch-processing) | "All usage is charged at 50% of the standard API prices"；缓存折扣可叠加 |
| S07 | [Gemini 定价页](https://ai.google.dev/gemini-api/docs/pricing)（页脚 Last updated 2026-09-24 UTC；一次抓取为机器翻译版本，数值美元原文） | 3.8 Flash 输入/输出/缓存/存储价与 2027 调价公示、Batch/Flex 五折、2.5 Pro >200K 阶梯、thinking 含在输出价；3.1 Pro preview 阶梯两次抓取不一致 |
| S08 | [Gemini models API](https://ai.google.dev/api/models) | key 鉴权；Model 资源无价格字段 |
| S09 | [Kimi CN 定价](https://platform.moonshot.cn/docs/pricing/chat)（canonical platform.kimi.com） | k3/k2.7-code/highspeed/k2.6 人民币价、1M 上下文、计费概念 |
| S10 | [Kimi global 定价](https://platform.moonshot.ai/docs/pricing/chat)（canonical platform.kimi.ai；.md 可匿名 curl） | 同模型美元价（不含税）；Mintlify .md 与 llms.txt 机器可读 |
| S11 | [Kimi thinking](https://platform.kimi.ai/docs/guide/use-thinking-models.md) | reasoning_content 计入输入/输出配额 |
| S12 | [Kimi context caching](https://platform.kimi.ai/docs/guide/context-caching.md) | 缓存写单列：k3 5m $3、1h $6、命中 $0.30；命中续期；拆分不改总成本声明 |
| S13 | [Kimi batch](https://platform.kimi.ai/docs/pricing/batch.md) | BatchJob 为标准价 60% |
| S14 | [Kimi Coding Plan 模型](https://www.kimi.com/code/docs/en/kimi-code/models.html) | 模型 ID：k3、k3-256k、kimi-for-coding（K2.8 Preview）、highspeed；会员配额制，无按量价 |
| S15 | [智谱定价页](https://open.bigmodel.cn/pricing)（JS 渲染，经渲染提取；无日期） | GLM-5.3/5.3-Flash/5.2/5.1 人民币价与 32K 档、Batch 五折、缓存存储限时免费、缓存命中价 |
| S16 | [智谱深度思考](https://docs.bigmodel.cn/cn/guide/capabilities/thinking) | 仅"消耗额外 Token"；示例 completion_tokens 合并、无 reasoning 拆示；计费口径未明示 |
| S17 | [Z.AI 定价页](https://docs.z.ai/guides/overview/pricing)（无日期） | GLM-5.3/Flash/FlashX 等美元价；无批处理/阶梯说明 |
| S18 | [models.dev api.json](https://models.dev/api.json)（4.9 MiB 快照，2026-09-25） | 结构（cost/tiers/reasoning_options）、200+ 提供商、coding-plan 条目 cost 0、zhipuai 条目挂 z.ai 价、gpt-6-astra 与官方一致 |
| S19 | [anomalyco/models.dev 仓库](https://github.com/anomalyco/models.dev)（自 sst 迁移） | MIT；推送 2026-09-25；README：社区贡献、api.json 端点、被 opencode 使用 |
| S20 | [OpenRouter models](https://openrouter.ai/api/v1/models)（匿名 200，460 模型，2026-09-25） | 美元/token 单位、overrides min_prompt_tokens=272000、:batch SKU；与官方一致与不一致实例（glm-5.3-flash、kimi-k2.7-code 偏低） |
| S21 | [pydantic/genai-prices](https://github.com/pydantic/genai-prices)（MIT，推送 2026-09-25） | YAML 源 + v2 data.json/Schema、prices_checked、历史价/分层/按日变价、zhipuai 换算说明、zai/moonshotai 与官方一致 |
| S22 | libgen 检索记录（2026-09-25） | 未找到名为 libgen 的 LLM 价格目录；仅命中 Library Genesis 等无关结果 |
| S23 | publishPriceDocs 检索记录（2026-09-25） | 未检索到该名称的公开 Gemini 价格 JSON；官方机器可读价格文件未发现 |

未核实项：OpenAI GPT-6 长上下文阈值官方标注；OpenAI 1h 缓存档；
Anthropic 页面版本标识；Gemini 3.1 Pro preview 阶梯口径；
Kimi CN 站缓存写分项价与公开模型列表端点；z.ai 批处理折扣；
GLM 思考 token 计费口径；bigmodel 公开模型列表端点；
OpenRouter 数据许可。本轮未对本仓库写任何业务代码、未制作种子快照、
未发起带凭据的请求；调研完成不等于已具备自动计价能力。
