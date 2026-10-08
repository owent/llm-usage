# Obtaining model API prices and estimating token costs

<a id="模型-api-按量价格获取与-token-费用估算方案"></a>

The cost engine and optional models.dev refresh are implemented and default off. See
[online refresh](#online-refresh), [cost-engine results](../../validation/desktop-usage/f2-cost-engine.md),
[refresh results](../../validation/desktop-usage/f2-online-refresh.md) and
[current API references](../../validation/desktop-usage/dashboard-reference.md).
Occurrence-time estimates on real data still require verified/configured provider channels;
reference-price comparison does not verify actual bills.
[Spending reminders](budget-reminders.md) are implemented/default off, using user daily/monthly
occurrence-time estimates per currency without zero-filling unknowns. Specified Coding Plans
do not verify actual pay-as-you-go payments; other scope remains in [Plan.md](../../../Plan.md).
Research completed 2026-09-25. [Data pricing rules](data-contract.md#pricing) are authoritative;
this document adds channel references/local snapshots. Prices below describe dated research
snapshots and test values, rather than a promise of current rates.

<a id="调研范围与方法"></a>

## Research scope and method

Prioritize locally observed providers: OpenAI Codex gpt-6-astra/gpt-6-sol; Zhipu GLM omp/pi/zcode
glm-5.3-flash/GLM-5.3 including synthetic glm-5.2; Moonshot kimi-code/kimi-for-coding/k3.
Anthropic/Google are specified integration targets without local data at the research date.
Compare official machine-readable sources/official pages/community catalogs and manual snapshots.
External lookup dates: 2026-09-25 unless a later dated entry says otherwise; source list below.
Absent verifiable information stays unverified. Page bodies/public curl JSON establish facts;
search summaries locate sources only. Most official pages lack versions/dates; snapshot identity
uses URL/retrieval date/visible updates, e.g. Gemini Last updated. Community catalogs use data
hash characteristics/repository push times.

<a id="价格获取渠道比较"></a>

## Price-source comparison

<a id="官方机器可读接口"></a>

### Official machine-readable interfaces

The checked OpenAI/Anthropic/Gemini model-list APIs require authentication and contain no price
fields. Moonshot's Mintlify platform supplies official .md/llms.txt from the same source as HTML.
No anonymous pricing JSON/YAML API was found for the researched providers; absence of a verified
endpoint does not prove one cannot exist.

| Provider | Model-list API | Price fields | Public machine-readable pricing | References |
| --- | --- | --- | --- | --- |
| OpenAI | GET /v1/models, Authorization: Bearer | No; id/created/object/owned_by/shutdown_date only | Not found; HTML docs, platform.openai.com redirects 301 to developers.openai.com | S01/S02 |
| Anthropic | GET /v1/models, X-Api-Key | No; id/display_name/created_at/max_tokens, etc. | Not found; HTML docs | S03/S04 |
| Google Gemini | GET /v1beta/models?key=, API key | No; Model has token limits, etc. | publishPriceDocs not found (S23); HTML with Last updated date | S07/S08 |
| Moonshot Kimi | Public model-list endpoint unverified | — | Anonymous docs/pricing/chat.md/docs/llms.txt via Mintlify; CN moonshot.cn/kimi.com and global moonshot.ai/kimi.ai separate | S09/S10 |
| Zhipu GLM | bigmodel public model-list endpoint unverified | — | Not found; bigmodel pricing is rendered JS SPA; international docs.z.ai static docs | S15/S17 |

OpenAI/Anthropic/z.ai/bigmodel pages lack date/version markers, so page age cannot be established
from them alone. Gemini dates updates but retrieval language can yield machine translations;
one fetched page was Polish with original USD figures (S07). No update frequency promised.
OpenAI mentions tier rename 2026-07-30/promotion end 2026-11-21, showing irregular revisions (S01).

<a id="社区目录"></a>

### Community catalogs

| Catalog | Data / license | Structure | Official comparison on 2026-09-25 | References |
| --- | --- | --- | --- | --- |
| models.dev | MIT, anomalyco/models.dev moved from sst; pushed 2026-09-25; api.json about 4.9 MiB/200+ providers | Provider groups; cost.input/output/cache_read/cache_write USD/million tokens; context tiers; reasoning/tools; many coding-plan entries | gpt-6-astra 10/50/1/12.5 and 272K tier 20/75 match; zai glm-5.3 1.4/4.4/0.26 matches docs.z.ai. zhipuai bigmodel endpoint links z.ai/USD without CN CNY. Coding-plan cost 0 denotes subscription placeholders | S18/S19 |
| OpenRouter | Anonymous GET /api/v1/models 200,460 models; USD/token decimal strings; license unverified | pricing.prompt/completion/input_cache_read/input_cache_write; overrides min_prompt_tokens long tiers; separate :batch SKU | gpt-6-astra/moonshotai/kimi-k3/z-ai/glm-5.3 match; Flash 0.045/0.14 below official 0.15/0.50; k2.7-code 0.6562/3.3 below 0.95/4.0. Aggregator rates are not official rates | S20 |
| pydantic/genai-prices | MIT; pushed 2026-09-25; provider YAML plus prices/new_data/v2/data.json/JSON Schema | prices_checked/pricing_urls; input/cache_read/output USD/million; historical/tiered/daily rates; provider conversion/selection notes | kimi-k3 3/0.3/15 and zai GLM-5.3 1.4/0.26/4.4 match. zhipuai explicitly converts CNY at 1 USD = 7.25 CNY/selects lowest flagship tier; an explicit CN approximation, not official data | S21 |
| libgen pricing | No LLM catalog with that name found after multiple searches; Library Genesis/unrelated results only | — | — | S22 |

Catalogs do not promise freshness. Cache-write TTL5m/1h, batch and subscription/API distinctions
vary. Aggregator rates are a different price category. Use catalogs for cross-checks/candidates,
without replacing official references.

<a id="手工维护本地快照"></a>

### Manually maintained local snapshots

See [versioned snapshots](#snapshot). Repository seeds are versioned JSON with source URLs/
lookup dates/effective intervals/extraction methods per row. UI imports local JSON to override/
supplement, validated by schema/interval conflicts. Work concentrates around flagship price
changes/new releases; the small locally used model set makes manual checking manageable.

<a id="渠道结论与选择"></a>

### Source selection

Prefer manually verified versioned snapshots: official machine-readable availability is limited,
while community catalogs have licensing/structural benefits and observed discrepancies. models.dev/
genai-prices cross-check official pages and supply candidates when pages are unavailable; adopted
rows retain community provenance. OpenRouter is comparison-only because aggregator rates differ.
Optional public-price metadata downloads default off. The implemented built-in refresh source
is models.dev api.json only, MIT with data-driven official-provider selection, raw caching/long
TTL/failure fallback; import official pay-as-you-go entries only, excluding plan placeholders.
Manual imports/repository seeds remain available; see [online-refresh design](#online-refresh).

<a id="billing-dimensions"></a>

<a id="计费维度核验"></a>

## Verified billing dimensions

The five researched providers use pages for observed/target models. Prices are per million
tokens; unverified means the page did not establish that dimension, without guessed values.

<a id="矩阵总览"></a>

### Dimension matrix

| Dimension | OpenAI | Anthropic | Gemini | Kimi | GLM |
| --- | --- | --- | --- | --- | --- |
| Ordinary input | Yes | Yes | Yes | Cache-miss price | Yes |
| Cache read | One tenth input | 0.1x, some0.025x/0.05x | Context-cache price | One tenth miss price | Cache-hit price |
| Cache write5m | 1.25x input | 1.25x input | No separate write price | Miss-input price | No separate write price |
| Cache write1h | Unverified/not found | 2x input | — | 2x miss-input | — |
| Cache storage | — | — | Million tokens/hour | Unverified | Temporarily free by policy |
| Output | Yes | Yes | Yes | Yes | Yes |
| Reasoning output | No separate rate; input/output rates | Thinking as output | Included in output | reasoning_content in input/output quotas | Official rule unspecified below |
| Batch | Batch/Flex 50% | 50%, combines cache discount | Batch/Flex 50% | BatchJob 60% | bigmodel50%; z.ai unverified |
| Long context | Two tiers; GPT-6 threshold absent on page, catalogs272K | 4.6+ standard at 1M, no tiers | Pro >200K | No tiers found | GLM-5.1 32K; none found5.2/5.3 |
| Currency | USD | USD | USD | CN CNY/global USD excluding tax | bigmodel CNY/z.ai USD |
| Region/channel | FedRAMP/data residency+10% | US-only inference1.1x for4.6+ | Not found/unverified | Two platforms/two rates | Two channels/two rates |

<a id="各家要点与本机在用模型价目"></a>

### Provider details and observed-model prices

OpenAI S01, undated page, locally observed Codex gpt-6-astra/gpt-6-sol: short astra input $10,
cache read $1, write $12.50, output $50; sol $2/$0.20/$2.50/$10. Long astra$20/$2/$25/$75.
The page does not show GPT-6's tier threshold although older families show272K; models.dev/
OpenRouter use272,000 (S18/S20), requiring official recheck before formal adoption. Fast,
renamed from Priority2026-07-30, costs2x; FedRAMP/residency+10% for models released after
2026-03-05. No separate reasoning rate; tokens use the chosen model's input/output rates.

Anthropic S03, research target with no local Claude data then: Opus 5.5 input $4/output $20,
write $5/$8, read $0.20 (0.05x); Sonnet 5$2/$10, Haiku 4.5$1/$5. Sonnet 5 promotional$2/$10
became permanent instead of the planned2026-09-01 increase. Batch 50% combines cache discounts
(S06). Thinking is output; usage.output_tokens_details.thinking_tokens breaks out billed
internal reasoning (S05). Retained historical thinking blocks count as input for Opus 4.5/4.6+.

Gemini S07, Last updated 2026-09-24 UTC:3.8 Flash standard input $0.75 through 2026-12-31,
$1.50 from 2027-01-01; output $3.75 including thinking; cache read $0.075/storage$0.50 per
million tokens/hour. Batch/Flex 50%;2.5 Pro >200K input $1.25/$2.50/output $10/$15.
English/machine-translated3.1Pro preview fetches disagree on >200K presentation; recheck before
adoption, as noted in S07.

Kimi S09–S13, observed Kimi Code/Work kimi-code/kimi-for-coding, wire inputOther/inputCacheRead/
inputCacheCreation align with dimensions. kimi-k3 CN hit¥2/miss¥20/output¥100; global $0.30/$3/$15,
excluding tax. kimi-k2.7-code CN¥1.30/¥6.50/¥27, global $0.19/$0.95/$4.
Separate cache-write policy S12: global k3 five-minute $3=miss input, one-hour $6=2x; hits renew
expiry. CN separate rates not found/unverified. BatchJob 60%, not50%(S13); reasoning_content
counts toward input/output quotas(S11).

GLM S15/S17, observed omp/pi/zcode glm-5.3-flash/GLM-5.3: bigmodelCN GLM-5.3 input¥8/cache¥2/
output¥28 at 1M context. Flash temporary50% rates¥0.4/¥0.115/¥1.4 versus standard¥0.8/¥0.23/¥2.8.
GLM-5.2 matches5.3;5.1 tiers[0,32K)/[32K+); Batch 50%; cache storage temporarily free.
International z.ai GLM-5.3$1.4/$0.26/$4.4, Flash $0.15/$0.03/$0.50, FlashX $0.37/$0.075/$1.25.
Batch/tier statements not found/unverified. Official thinking docs only mention additional tokens;
examples combine reasoning_content/content in completion_tokens without reasoning_tokens(S16).
Billing remains unverified; snapshots omit a separate reasoning price, and known output subsets
must not be billed again.

<a id="本机在用模型与价目映射"></a>

### Observed models and price mapping

| Observed model | Source | Pay-as-you-go availability | Treatment |
| --- | --- | --- | --- |
| gpt-6-astra/gpt-6-sol | Codex ChatGPT subscription | S01 | API reference estimate applied to subscription usage |
| GLM-5.3/glm-5.3-flash | omp/pi/zcode | S15/S17, two channels/currencies | Exact channel first; unknown channels permit unambiguous official same-model reference only, without actual-payment inference |
| kimi-code/k3 | Kimi Code/Work/omp | k3 S09/S10 | Coding Plan usage gets API reference estimate |
| kimi-for-coding | Kimi Code/Work | Official mapping since 2026-09-11 to K2.8 Preview; no verified original-vendor API price, S14 | Authorized K2.8 exception: current K2.7 Code official reference with substitute label; occurrence-time estimate still lacks price |
| k28-agent-preview | Kimi K2.8 Preview | User confirmed identity/substitution2026-10-05 | Identity kimi-k2.8-preview; only missing exact prices use kimi-k2.7-code, not arbitrary unknown K2/K3 |
| Synthetic glm-5.2 | Kilo test samples | S15/S17 | Same two-channel handling as GLM-5.3 |

Coding Plans/API usage are different billing systems. models.dev kimi-code-plan-*/zai-coding-plan
cost 0 is a subscription placeholder, not free API pricing; never import placeholder zeros.

<a id="snapshot"></a>

<a id="版本化价格要求"></a>

<a id="版本化价格合同"></a>

## Versioned price-snapshot requirements

<a id="快照格式"></a>

### Snapshot format

A metadata header and row array. Each billing context is provider×model×region/channel×service
tier×context tier×effective interval. Implemented2026-09-30/schema 8: price_snapshots metadata,
price_versions mandatory region/channel, service_tier/context_threshold_tokens/five
*_per_mtok_hundredths price columns/hourly cache_storage, matching these decisions:

| Dimension | Research schema | Implemented decision |
| --- | --- | --- |
| Cache-write TTL | One cache_write | cache_write_5m/cache_write_1h; absent tier NULL |
| Service tiers | Absent | service_tier standard default; separate tier rows; auto-estimation standard only |
| Context tiers | Absent | context_threshold_tokens minimum input, NULL=0; per-request input determines tier; unknown input with multiple tiers stays ambiguous |
| Cache storage | Absent | cache_storage_hour; temporary free0 with policy metadata; engine omits without storage duration |
| Precision | Integer minor currency units/million | Hundredths of minor units/million, i64; $0.075/M=750, supports ¥0.115/M |
| Provenance | price_version string only | Snapshot ID/source type/URLs/fetched_at/content_hash/license/checker/method |
| Reasoning price | Absent | No separately verified rate; GLM remains unverified; no second charge for output subsets |

Versioned seeds desktop/src-tauri/crates/core/prices/seed-2026-09-25.json and seed-2026-10-02.json
import once on startup. Later seed adds Opus 4.8/GPT-4o mini2024-07-18 standard rates effective
from verification date, without backdating. Settings costs imports use the same JSON/manual
source_type. Monetary values/prices never use binary floats; [data rules](data-contract.md#pricing).
Format llm-usage-price-snapshot/1; ISO UTC YYYY-MM-DD dates, half-open effective interval,
null end=open; prices hundredths of minor currency units/million; null component=no applicable rate:

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

Validate USD/CNY/service-tier enums/nonnegative prices/legal nonoverlapping same-key provider/
model/region/channel/tier/threshold intervals/unique price_id without imported-row conflicts.
Same snapshot ID skips only when price-field hashes/source metadata/notes all match; corrections
require a new snapshot ID. Optional official_vendor bool since schema 11 marks original-vendor
API prices as fallback candidates: seed/community default true, manual false; manual official
rows can explicitly set true. Exact provider_id/region/channel matches selected defaults
case-insensitively. Missing exact rows may use official same-model references below without
guessing the actual channel. Across snapshots: applicable date/threshold, manual>seed>community,
then newer fetched_at/import time within one class. Below-threshold inputs do not use a row;
unknown inputs with high-tier rows do not guess tiers.

<a id="生效区间与更新流程"></a>

### Effective intervals and updates

1. Half-open [effective_from_ms,effective_to_ms); same-key provider/model/channel/tier/context
   rows within a snapshot cannot overlap. Match occurred_at_ms.
2. Append rows only. New snapshots with later starts override old open intervals without
   changing imported old rows. Announced future changes, e.g. Gemini2027-01-01/GLM Flash discount
   expiry, can be registered for automatic date-based switching.
3. Prefer manual import/official-page or .md seeds with lookup dates. Optional downloads default
   off per [cost/reminder settings](data-contract.md#settings). HTTPS GET public price endpoints
   sends no local usage/host-source identity/session/account keys; no remote usage/billing APIs.
4. Validate JSON Schema/nonnegative values/currencies/intervals; preview additions/interval
   closures/price changes per row. Community/official discrepancies warn and permit rejection.
5. Persist locally; statistics/prior estimates work offline. Failed downloads/validation retain
   verified snapshots/show fetched_at/verified_at age; no silent amount changes.
6. Reproduce history through data_revision/price_version. Keep occurrence-time amounts distinct
   from current-price simulation. Dashboard displays current API pay-as-you-go references rather
   than duplicate historic estimates. Background updates do not recalculate saved amounts;
   explicit user requests/data revisions may. Matching sealed days mark limited detail coverage.

<a id="online-refresh"></a>

<a id="在线刷新设计modelsdev-社区目录"></a>

## Online refresh: models.dev community catalog

<a id="在线刷新设计modelsdev-社区目录2026-10-01-实施"></a>

Task5 implementation. Settings costs online refresh defaults off. Enabled requests only HTTPS
GET [models.dev api.json](https://models.dev/api.json) (MIT,S18/S19/S24); no local usage/identities/session bodies/keys;
User-Agent contains app/version only. Offline statistics/prior estimates and the no-remote-
usage/billing boundary remain unchanged.

<a id="缓存与失败回退"></a>

### Cache and failure fallback

<a id="缓存与失败回退2026-10-01-用户合同"></a>

- Raw response `<DB-directory>/price-cache/models-dev-api.json` and .meta.json URL/fetch time/
  content hash/bytes; files survive DB rebuild.
- Default TTL three days, configurable 1–365. Fresh cache imports without network; explicit
  refresh-now bypasses TTL.
- Download/validation failures reuse the last successful cached response. No cache returns
  error/preserves all snapshots(A9); invalid responses never replace cache.
- Successful download IDs models-dev-YYYYMMDD-hash8 from date/hash; unchanged content skips;
  snapshot imports never recalculate saved occurrence estimates.
- Check after collection; enabled/expired cache downloads in background. Failures produce
  operation logs/diagnostics without blocking collection.

<a id="目录过滤以官方按量价为准"></a>

### Catalog filtering: official pay-as-you-go prices

2026-10-01 S24 api.json:225 providers/8339 models; context-only tiers; cost fields input/output/
cache_read/cache_write/input_audio/output_audio/reasoning/tiers/context_over_200k. No currency
field: USD/million list prices throughout.

- Data-driven official selection: provider IDs prefixed in at least one canonical_model_id,
  plus corresponding `<id>-cn` variants. Measured 23+3: openai/anthropic/google/zhipuai/moonshotai/
  alibaba/deepseek/xai/mistral/etc., plus moonshotai-cn/minimax-cn/alibaba-cn. Exclude aggregators/
  gateways/subscription placeholders.
- Skip provider IDs containing -plan, including checked coding-plan/token-plan/step-plan all-zero
  placeholders. Skip models whose input/output are both zero/missing; zero is not free pricing.
- Zero cache/read/write/input/output components import NULL for unavailable rate.
- cost.input/output/cache_read map directly; cache_write=>cache_write_5m, one-hour NULL because
  catalog lacks TTL tiers. context tiers create context_threshold_tokens rows. Ignore duplicate
  context_over_200k and unsupported audio/reasoning dimensions; reasoning already in output.
- region cn for -cn/global otherwise, channel api,currencyUSD; CNY remains seed/manual. Only
  standard service; experimental.modes fast/ultrafast excluded. effective_from=fetch date without
  native price-effective intervals; freshness describes lookup time.
- USD/million×10^4 converts to hundredths of cents/million, rounded. Small prices such as
  DeepSeek cache $0.003625/M incur <=0.5 unit/<$0.00005/M rounding, noted in row metadata.
- Twenty-six eligible providers yield408 models/473 rows per snapshot. Empty-priced poolside/
  sarvam are skipped; persisted rows cover24 providers, as S24/test output distinguishes.

<a id="官方提供商回退匹配"></a>

### Official-provider fallback

<a id="官方提供商回退匹配2026-10-01-用户合同"></a>

official_vendor schema 11: seed/community true, manual false unless explicitly set. Only when
exact provider=>selected region/channel=>model=>standard=>interval=>tier finds no_price_row,
search official rows for the same model/standard/date; selected region/channel first, otherwise
manual>seed>community/newer within class. Unknown input/multiple tiers remains tier_ambiguous.
Mark official_fallback and daily/summary fallback_event_count; UI shows official-reference
count while retaining subscription-reference semantics. Missing provider/unconfigured channel
enters reference search without premature no_provider/channel_unknown. Vendor-family restriction
still requires an exact model: GPT/o OpenAI, Claude Anthropic, Gemini Google, Kimi/Moonshot
Moonshot, GLM Zhipu/Z.ai, DeepSeek DeepSeek; references/IDs in [dashboard rules](dashboard-repair.md).
Prefer selected channel, then global official api (including vendor-named compatible channels).
Ambiguous remaining regions/channels/currencies stay unpriced; no invented rates/conversion.

[Official Kimi model table](https://www.kimi.com/code/docs/en/kimi-code/models.html)
maps k3/k3-256k toK3. Local kimi-code/ profiles can reference kimi-k3 for price matching only,
without changing raw statistics; exact profile prices win. Missing official same-model row yields
no_price_row; unknown family plus missing provider yields no_provider. Real provider stays
unchanged; reference is not actual payment. Known usage_observation tokens can be partially priced;
priced-event counts are not call counts. One verified correction of existing unsealed/detail-
retained estimates is permitted; later background prices leave occurrence amounts unchanged.
Overview/trend show references/coverage; [results](../../validation/desktop-usage/dashboard-repair.md).

Queries retain historic responses and current model/currency subtotals/daily provider-model amounts/
matched price_id units. Dashboard model cost column/footer, currency/model curves and expandable
units share current references. One row per source-provider/model; show reference provider/snapshot/
context tier. Query the same retained details/sealed daily-weekly-monthly partitions as usage,
exclusively by complete source dimension; cleanup neither loses costs nor double-counts coexisting
details. Weekly/monthly archives cannot invent daily curve points. Price saved archive components
only; different sample-population sums cannot derive missing input components. Keep known totals
in coverage denominators. Without per-call tiers use applicable tiers from the same preferred
snapshot to bound known components; these intervals do not price missing components. Indivisible
archives crossing dynamic-alias changes retain model ambiguity. Usage/prices share one SQLite
read transaction. Today/range/selected-hour share references; local-hour filtering includes DST
repeated hours and does not substitute whole-day costs. Queries leave saved estimates/snapshots
unchanged. Hour-granularity usage may still show daily cost curves without prorating day costs.
Candidate reduction retains providers/priorities/dates/tiers; see
[interactions](dashboard-polish.md)/[results](../../validation/desktop-usage/dashboard-polish.md).
Never fallback after an exact-chain match, including tier_ambiguous.

<a id="费用规则细则"></a>

<a id="费用合同细则"></a>

## Detailed cost rules

These refine authoritative [data rules](data-contract.md#pricing):

1. Unpriced required components/model without API rate or authorized current substitute/
   ambiguous channel-tier/unknown writeTTL remain empty, labeled unpriced; no zero/average rate.
2. Known priceable components contribute subtotals even with missing others; coverage is priced
   tokens/known tokens, total marked partial estimate.
3. Store hundredths of minor currency units/million; sum i128 token×price products before
   dividing 100,000,000, round each billing component to minor units. Current references group
   day/provider/model/currency; indivisible archives by period, then combine model/grand totals
   consistently with tables. Occurrence-time daily cost partitions retain sealed estimates.
   Never discard tiny event fractions before aggregation. Negative values/cache exceeding
   known input diagnose; overflow explicitly fails instead of becoming zero/unknown.
4. Separate currencies. Preserve CNY originals/units, show approximate USD beside them with
   verified ECB2026-10-02 snapshot/date/source; no live exchange refresh/stored-value changes;
   [dashboard repair](dashboard-repair.md). User conversion estimates retain source/time;
   absent applicable rate displays separate currencies only.
5. API rates applied to ChatGPT/Coding Plan/GLM subscriptions are reference estimates; actual
   subscription payments differ. Estimated cache savings are not actual refunds.
6. Missing writeTTL, e.g. Kimi inputCacheCreation, defaults unpriced. Explicit per-provider
   default TTL such as5m permits estimated pricing under that tier.

<a id="v29-验收样本设计"></a>

## V29 acceptance samples

[Validation checklist](validation.md) V29 uses fixed snapshots/manual expected amounts/errors.
Values from 2026-09-25 official S01/S09–S13/S15 are fixed tests, not ongoing price guarantees;
version them with seeds.

<a id="固定价格样本"></a>

### Fixed price samples

| Row | Provider/model | Channel/currency | Tier | Threshold | Input | Cache read | Write5m | Write1h | Output |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| P1 | glm-5.3 | bigmodel-cn/CNY | standard | — | 800 | 200 | NULL | NULL | 2800 |
| P2 | gpt-6-astra | openai/USD | standard | 0 | 1000 | 100 | 1250 | NULL | 5000 |
| P3 | gpt-6-astra | openai/USD | standard | 272000 | 2000 | 200 | 2500 | NULL | 7500 |
| P4 | kimi-k3 | moonshot-global/USD | standard | — | 300 | 30 | 300 | 600 | 1500 |
| P5 | glm-5.1 | bigmodel-cn/CNY | standard | 0 | 600 | NULL | NULL | NULL | 2400 |
| P6 | glm-5.1 | bigmodel-cn/CNY | standard | 32768 | 800 | NULL | NULL | NULL | 2800 |

Table units: minor currency units/million (fen/cents): P1input800=¥8/M, P2input1000=$10/M.
Engine storage multiplies table values by100, e.g.P1input80000. NULL cache write means no
verified separate rate; temporary-free policies stay metadata, without substituting a zero rate.

<a id="人工期望金额"></a>

### Manually expected amounts

Single-event expected components round to fen/cents. Multi-event groups sum integer products
before rounding, rather than summing rounded event values. All events occur inside half-open
snapshot intervals. Archive/precision regression: [2026-10-05](../../validation/desktop-usage/pricing-archive-repair.md).

| Case | Tokens: uncached/read/write/output | Row | Expected |
| --- | --- | --- | --- |
| E1 mixed input | 1,234,567/500,000/200,000/345,678 | P1 | 988+100+968=2056 fen, ¥20.56; writes unpriced/coverage notice |
| E2 all components | 1,000,000/200,000/100,000/250,000 | P3 | Input 1.3M≥272K selects P3: manually calculated 2000+40+250+1875=4165 cents/$41.65. The original P2 calculation of 2395 cents does not apply at this input size. Current tests check scaled 130K-input E2′ under P2 (240 cents) and P2/P3 threshold boundaries; [results](../../validation/desktop-usage/f2-cost-engine.md) |
| E3 one-hour TTL | 400,000/300,000/600,000(1h)/150,000 | P4 | 120+9+360+225=714 cents/$7.14 |
| E4 upper-tier boundary | Input272,000/0/0/output 8,000 | P3 | 544+60=604 cents |
| E5 below boundary | Input271,999/0/0/output 8,000 | P2 | 272+40=312 cents |
| E6 GLM tier | Input32,768/0/0/output 1,000 | P6 | 26+3=29 fen |
| E7 below GLM tier | Input32,767/0/0/output 1,000 | P5 | 20+2=22 fen |
| E8 separate currencies | E1+original E2 example | P1+P2 | Original example2056CNYfen/2395USDcents separate; no conversion/combination; E2 tier correction remains applicable |

<a id="异常场景"></a>

### Error and boundary scenarios

| Case | Input | Expected |
| --- | --- | --- |
| A1 no occurrence-time price | kimi-for-coding usage | Empty occurrence estimate; current substitute tested separately |
| A2 partial pricing | Known input/unknown output | Price input; partial total/correct coverage |
| A3 no duplicate reasoning | reasoning50,000 subset of output 200,000 | Charge output_total once |
| A4 no wrong service tier | Ordinary realtime call | No batch price |
| A5 absent cache-read price | Known reads/NULL rate | Component unpriced; others count/coverage notice |
| A6 unknown TTL | Known writes/no TTL/default | Writes unpriced until explicit default, then estimated |
| A7 historical reproduction | Within/outside old interval | Occurrence rates; missing history gets separate current-price simulation, never mixed |
| A8 invalid tokens | Negative/cache exceeds known input | Reject cost calculation/diagnose; no max(0,…) |
| A9 refresh failure | Network/validation error | Retain snapshots/age/amounts; last successful cache fallback; no-cache error preserves snapshots |
| A10 immutable snapshot | Same snapshot repeated | Skip unchanged; corrected price uses new price_version |

<a id="后续实施任务清单"></a>

## Implementation checklist and dated progress

2026-09-30 [results](../../validation/desktop-usage/f2-cost-engine.md), with later completion
updates explicitly noted below:

1. Implemented schema 7=>8 price_snapshots/redefined price_versions/daily_cost_usage; prerelease
   backup/rebuild policy.
2. Implemented seed 2026-09-25/currency-tier/nonnegative/overlap/repeat-import validation.
   Official-page extraction script optional/unimplemented; seed manually authored.
3. Implemented matching/components/rounding/unpriced-partial coverage/i128/overflow. UI labels references.
4. Implemented saved occurrence-time/current simulation/history data_revision/price_basis sets.
5. Implemented optional models.dev-only/raw-cache defaultthree-day/failure fallback/official
   filter/fallback schema 11/A9; [design](#online-refresh)/[results](../../validation/desktop-usage/f2-online-refresh.md).
6. Implemented default-off cost UI/currency groups/snapshot view/manual import/explicit recompute.
   Reminders were deferred in the original2026-09-30 checklist; the opening update records later
   default-off implementation, with advisory behavior rather than blocking usage.
7. Executed V29 fixed P1–P6/E/A cases and manual expectations. Real occurrence estimates still
   require verified/configured channels; current-reference checks recorded separately.

<a id="风险与限制"></a>

## Risks and limits

- Undated official pages measure freshness by retrieval, not effective revision time. Community
  cross-checks/visible age reduce but cannot eliminate silent price changes.
- Region/channel differences matter: K3 CN¥20/global $3 input cannot substitute each other.
  Verify actual subscription endpoints per Agent; unknown channels may use unambiguous official
  same-model/currency references without endpoint inference.
- Resolve kimi-for-coding by usage date, never apply today's dynamic route retroactively.
  K2.8 Preview lacks verified Moonshot API prices; user authorizes current K2.7 Code official
  substitution with label. Future exact prices win without deleting the substitute rule.
  estimate_at_time never substitutes models or rewrites historical amounts. Other channels
  need independent labels/references; [spelling/aliases/channel rules](dashboard-repair.md).
- Observed catalog discrepancies include OpenRouter aggregator rates/models.dev zhipuai with
  international rates; retain recognizable community provenance.
- Limited-time GLM Flash discounts/free cache storage/Kimi separate-write policy may expire;
  rows express effective changes but detection depends on checking/refresh.
- Unverified/inconsistent: GPT-6 official threshold, GLM thinking billing, KimiCN write components,
  z.ai batch discount, Gemini3.1Pro preview tier presentation. Recheck officially before adoption.
- The research batch had no direct OpenAI/Anthropic/Gemini provider usage samples for API-bill
  comparison; page-level price checks do not establish local end-to-end billing matches.
- models.dev remains community data. zhipuai links z.aiUSD; -cn USD entries' derivation from CNY
  is unverified. Changed structures fail validation/fallback instead of silent import. Community
  snapshots containUSD only; CN CNY prices require seed/manual data.
- Seeds/engine/online refresh are implemented; V29/validation records establish checked scope.

<a id="调研记录"></a>

## Research record

Windows 11 x64/Git Bash/curl-jq public JSON/Markdown/WebFetch-web-reader page bodies; dates
2026-09-25 unless stated. Status: research references, not runtime acceptance.

| ID | Source | Adopted facts |
| --- | --- | --- |
| S01 | [OpenAI pricing](https://developers.openai.com/api/docs/pricing), old platform.openai.com/docs/pricing redirects301; undated | gpt-6-astra/sol/luna/gpt-5.6 rates/short-long/Batch-Flex50%/Fast2x renamed 2026-07-30/FedRAMP-residency+10%/no separate reasoning rate/promotions |
| S02 | [OpenAI models API](https://developers.openai.com/api/docs/api-reference/models/list) | Bearer; no price fields |
| S03 | [Anthropic pricing](https://platform.claude.com/docs/en/about-claude/pricing), no version | Input/output/write5m-1h/read/Sonnet 5 promotion permanent/4.6+ no long tier/US-only1.1x |
| S04 | [Anthropic models API](https://platform.claude.com/docs/en/api/models-list) | X-Api-Key; no price fields |
| S05 | [Extended thinking](https://platform.claude.com/docs/en/build-with-claude/extended-thinking) | Thinking as output/thinking_tokens billed subset/retained thinking as input |
| S06 | [Anthropic batch](https://platform.claude.com/docs/en/build-with-claude/batch-processing) | Standard API50%; cache discounts combine |
| S07 | [Gemini pricing](https://ai.google.dev/gemini-api/docs/pricing), Last updated 2026-09-24 UTC; one translated fetch, USD originals | Flash 3.8 input/output/cache/storage/2027changes; Batch-Flex50%;2.5 Pro >200K; thinking output;3.1 Pro preview inconsistent fetches |
| S08 | [Gemini models](https://ai.google.dev/api/models) | Key; no price fields |
| S09 | [Kimi CN](https://platform.moonshot.cn/docs/pricing/chat), canonical platform.kimi.com | k3/k2.7-code/highspeed/k2.6 CNY/1M/billing concepts |
| S10 | [Kimi global](https://platform.moonshot.ai/docs/pricing/chat), canonical platform.kimi.ai; anonymous .md | Same modelsUSD excluding tax; Mintlify .md/llms.txt |
| S11 | [Kimi thinking](https://platform.kimi.ai/docs/guide/use-thinking-models.md) | reasoning_content input/output quota |
| S12 | [Kimi caching](https://platform.kimi.ai/docs/guide/context-caching.md) | k3 write5m$3/1h$6/read $0.30/hit renewal/separation leaves total costs unchanged |
| S13 | [Kimi batch](https://platform.kimi.ai/docs/pricing/batch.md) | BatchJob 60% |
| S14 | [Kimi Coding Plan models](https://www.kimi.com/code/docs/en/kimi-code/models.html) | k3/k3-256k/kimi-for-coding K2.8 Preview/highspeed; subscription quotas without API rates |
| S15 | [Zhipu pricing](https://open.bigmodel.cn/pricing), rendered JS/undated | GLM-5.3/Flash/5.2/5.1 CNY/32K/Batch 50%/temporary free storage/hit rates |
| S16 | [Zhipu thinking](https://docs.bigmodel.cn/cn/guide/capabilities/thinking) | Additional tokens only; combined completion_tokens/no reasoning breakdown; billing unspecified |
| S17 | [Z.AI pricing](https://docs.z.ai/guides/overview/pricing), undated | GLM-5.3/Flash/FlashXUSD; batch/tier statements absent |
| S18 | [models.dev api.json](https://models.dev/api.json),4.9 MiB/2026-09-25 | cost/tiers/reasoning_options/200+providers/plan0/zhipuai z.ai rates/astra official match |
| S19 | [anomalyco/models.dev](https://github.com/anomalyco/models.dev), moved from sst | MIT/pushed 2026-09-25/community README/api.json/OpenCode use |
| S20 | [OpenRouter models](https://openrouter.ai/api/v1/models),anonymous200/460 models/2026-09-25 | USD/token/overrides min_prompt_tokens272000/:batch; matches and lower Flash/k2.7-code examples |
| S21 | [pydantic/genai-prices](https://github.com/pydantic/genai-prices),MIT/pushed 2026-09-25 | YAML/v2JSON-Schema/prices_checked/history-tiers-daily/zhipuai conversion/zai-moonshot official matches |
| S22 | libgen search2026-09-25 | No named LLM-price catalog found; unrelated Library Genesis |
| S23 | publishPriceDocs search2026-09-25 | No public Gemini pricing JSON found under that name |
| S24 | models.dev api.json2026-10-01,5.28 MB/HTTP200 | Provider{id,env,npm,name,doc,models}; cost{input,output,cache_read,cache_write,tiers,context_over_200k,input_audio,output_audio,reasoning}; canonical_model_id 5274/8339;658 context tiers; no currency,USD/million. Official canonical-prefix/-cn selection26 eligible providers/408 models/473 rows; empty-priced poolside/sarvam excluded, rows24 providers, verified by probe. Plans all0. Official matching astra$10/$50,K3 $3/$15,GLM-5.3$1.4/$4.4,Opus 5.5$4/$20,Flash 3.8$0.75/$3.75; zhipuai/zai same docs.z.ai price |
| S25 | [ECB daily rates XML](https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml),fetched 2026-10-03/effective 2026-10-02 | EUR/USD1.1225,EUR/CNY7.5259; CNY=>USD1.1225/7.5259 for reference display, not bills/trades |
| S26 | [Kimi model table](https://www.kimi.com/code/docs/en/kimi-code/models.html),[global API](https://platform.kimi.ai/docs/pricing/chat),[models.dev README](https://github.com/anomalyco/models.dev),reread2026-10-03 | k3/k3-256k mapK3; USD/million input 3/read 0.30/output 15/write5m-1h 3/6. models.dev alreadyUSD/million; CN provider names do not justify reconversion; exact bare-profile aliases leave statistics names unchanged |

Still unverified: official GPT-6 threshold/OpenAI1h/Anthropic page version/Gemini3.1Pro tiers/
KimiCN write rates/public model-list endpoint/z.ai batch/GLM thinking billing/bigmodel model
endpoint/OpenRouter license. The original research wrote no business code/seeds and made no
credentialed requests; research alone did not establish automated pricing. Later implementation
is explicitly recorded above.
