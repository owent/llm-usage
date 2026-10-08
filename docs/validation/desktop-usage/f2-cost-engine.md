# F2 cost-estimation implementation checks, 2026-09-30

<a id="f2-费用估算引擎实施验证记录2026-09-30"></a>

Scope: [pricing specification](../../design/desktop-usage/pricing.md) tasks 1–4/6–7;
optional online refresh, task 5, explicitly deferred. Schema v8/seed snapshots/estimation/
daily-cost backfill/summary queries/commands/UI/V29 format tests. No commit/push/deployment.
This record preserves the initial implementation. Later [archive/precision repairs](pricing-archive-repair.md)
supersede early per-component rounding and detail-only current-reference limitations;
current rules remain in the pricing specification.

<a id="实施内容"></a>

## Implemented features

<a id="1-价格-schema任务-1schema_version-78"></a>

### 1. Price schema: Task 1, SCHEMA_VERSION 7→8

crates/core/src/storage/schema.rs adds price_snapshots metadata: source type/URL/retrieval
and verification times/FNV-1a hash/license/reviewer. Redefines price_versions with required
region/channel, service_tier/context_threshold_tokens/five component rates/cache_storage
hourly rate; units one hundredth of the smallest currency unit per million tokens.
daily_cost_usage backfills by timezone/day/source/model/currency/kind, estimate_at_time/
reported/source_estimate. Unpriced count/reason histogram in currency='' rows; sealed
behavior matches daily_usage. Prerelease rule, user decision 2026-09-26: no per-version
migration; mismatch uses existing backup→rebuild→rescan path, prompting local v7 upgrades.

retention_tiered.rs seals daily costs when details expire, preserving historical amounts;
clearing daily data also clears daily costs according to daily_days retention.

<a id="2-种子快照与导入任务-2"></a>

### 2. Seed snapshots and import: Task 2

- crates/core/prices/seed-2026-09-25.json: 19 prices from official page text, pricing.md
  S01/S03/S07/S09/S10/S15/S17, retrieved 2026-09-25. Includes OpenAI gpt-6-astra/gpt-6-sol,
  excluding then-unverified GPT-6 long-context threshold; Moonshot two channels/currencies,
  kimi-k3/kimi-k2.7-code, without undisclosed CN cache-write rows; Zhipu bigmodel GLM-5.3/
  5.2/5.3-Flash limited half-price/5.1 two tiers, limited free cache storage explicitly zero
  as policy; international Z.ai flashx cache read USD 0.075/M, represented in hundredths;
  Anthropic IDs not yet matched to local files, noted in comments; Google 3.8 Flash prices
  through 2026-12-31 and announced 2027-01-01 future interval. 2027 input-only disclosed,
  other components NULL.
- pricing.rs PriceSnapshot::from_file validates format tag, USD/CNY currency, service-tier
  enumeration, nonnegative prices/legal effective intervals/nonoverlapping same-key intervals.
  to=NULL is open-ended, preventing later same-key rows. price_id unique within snapshot;
  seed/community require source URLs.
- A10 repeat import: same snapshot ID/hash skips; same ID/different content errors, requiring
  a new ID for correction. Cross-snapshot price_id conflict rejected. AppState::init imports
  seeds without duplicates; import failure does not block collection.

<a id="3-估算引擎任务-3纯函数-pricingrs"></a>

### 3. Estimation: Task 3, pricing.rs pure functions

Matching order at this stage: casefold provider→explicit user region/channel, otherwise
channel_unknown and unpriced→model, canonical first then casefold exact, no alias inference→
standard service, without mixing batch/flex/fast, A4→half-open effective interval, at_time
uses occurred_at/current_sim evaluation time→context tier. Multiple tiers with unknown
input produce tier_ambiguous, otherwise largest satisfied threshold.

Price uncached input, explicit or total−read−write when all known; cache read; cache write
with user-default TTL because events lack TTL, absent default leaves write unpriced, A6;
and output. Initial amount round_half_up(tokens×rate/1e8), i128 intermediates, each component
rounded to smallest currency unit before addition. Reasoning subset has no separate rate,
never double priced, A3. Reject whole record with negative tokens/cache sum above known input,
A8, without max(0,...). Coverage priced_tokens/known_tokens plus has_unknown_components;
unknown output/uncached input means partial estimate, A2.

<a id="4-日成本回填与汇总任务-4"></a>

### 4. Daily backfill and summaries: Task 4

- storage/pricing.rs recompute_cost_day rewrites unsealed days only. Without retained detail
  events, leave historical rows that may represent expired detail. After event batches,
  scanner backfills days changed this round when pricing.enabled. Explicit background
  recalculation processes all unsealed days still with detail. Snapshot import does not
  trigger recalculation or rewrite existing estimates.
- cost_summary occurrence-time estimates and source amounts read daily_cost_usage, with
  price_basis snapshot set/data_revision. Initial current_sim prices retained detail live,
  marking detail_limited after expiration. Currencies/subtotals remain separate, E8;
  unpriced-reason histograms shown separately.

<a id="6-命令与界面任务-6"></a>

### 6. Commands and UI: Task 6

- cost_summary/recompute_costs, latter background worker plus operation log;
  list_price_snapshots/import_price_snapshot from manual file path.
- AppSettings.pricing off by default; enabled plus provider defaults provider/region/channel/
  TTL 5m or 1h, persisted with settings.
- Settings Cost page: switch/notices/snapshot list-import-recalculate/provider defaults.
  Trend Cost estimate reference panel hidden until enabled, existing saved layout retained.
  Three columns separated by currency, coverage/partial/unpriced labels and snapshot basis.
  26 new i18n keys across ten languages.

<a id="7-v29-验收任务-7"></a>

### 7. V29 acceptance: Task 7

- pricing.rs nine unit tests: seed parsing, negative/overlapping rejection, rounding
  987.6536→988/2.8→3/26.2144→26, unconfigured channels, invalid tokens, E6/E7 tiers, TTL
  default, derived uncached input, events before snapshot effective date.
- Initial tests/pricing_v29.rs three format tests: P1–P6 plus A4 batch row through import→
  ingest→backfill→summary; E1 2056 cents/E3 714 US cents/E4 604/E5 312/E6 29 cents/E7 22 cents;
  A1/A2/A3/A4/A5/A6/A8/A10 behaviors; E8 two currencies; A7 occurrence-time/current prices
  separate, 09-20 event unpriced at_time/current_sim 803 cents; unknown channels and repeat imports.
- Original E2 input totals 1.3M and selects P3 at ≥272K; its manual calculation is
  2000+40+250+1875=4165 US cents. Current pricing_v29.rs checks scaled 130K-input E2′
  under P2 (100+2+13+125=240 cents) and P2/P3 threshold boundaries. The current file
  has no separate original-1.3M case; do not describe this calculation as an executed test.
  pricing.md now states the checked scope.
- Unexecuted A9 refresh-failure fallback because online refresh not implemented. Native
  read-only price checks require per-agent GLM/Kimi provider spelling/channel verification
  before snapshot selection. All default channel_unknown/unpriced at this stage, awaiting
  provider defaults configured by user.

<a id="测试与命令windows-11-x64仓库根desktopsrc-tauri"></a>

## Tests and commands: Windows 11 x64, root/desktop/src-tauri

| Command | Exit | Result |
| --- | --- | --- |
| cargo test -p llm-usage-core --lib pricing | 0 | Nine pass |
| cargo test -p llm-usage-core --test pricing_v29 | 0 | Five pass, including grouping filters/selected-day revision limits |
| cargo test -p llm-usage-core | 0 | All 66 test targets pass |
| cargo test -p llm-usage-desktop | 0 | 15 pass, including timezone-partition rebuild |
| cargo clippy --workspace --all-targets -- -D warnings | 0 | Passed |
| cargo fmt --all --check | 0 | Passed |
| npm --prefix desktop run check, svelte-check | 0 | Zero errors/warnings |
| npm run test:ui / test:scripts | 0 | Eight UI tests plus script tests pass |

Full npm run verify and npm run test:browser rerun after these changes, below.

<a id="缺口与后置项如实登记"></a>

## Remaining work at this stage

1. Optional task 5 online refresh not implemented, off by default. Manual imports/seeds
   only; official pages lack machine-readable APIs except Moonshot Markdown. Then-proposed
   implementation needed per-channel parsers; A9 to run with that task.
2. Limited snapshot freshness display: panel shows price_basis IDs, fetched_at visible in
   settings snapshot list, without dates on trend panel.
3. No native end-to-end estimates yet. User must configure provider defaults in Settings→
   Costs; local GLM provider spelling/bigmodel-versus-Z.ai channel unverified per agent.
4. No cache-storage cost in estimate rows. cache_storage hourly column exists; events lack
   storage duration, so pricing needs a storage timeline and is deferred.
5. Hermes-like interval usage_observation not priced: no input split, total alone insufficient
   per data rules; explicitly excluded.
6. Schema v8 upgrade prompts backup→rebuild→rescan for existing v7 databases under prerelease rules.

<a id="复核修复2026-09-30-第二轮"></a>

## Review repairs: Second round, 2026-09-30

Self-review found and fixed three defects with tests/document corrections:

1. Automatic postscan backfill selected no dates. It matched current data_revision equality,
   but tiered retention increments revisions again after scanning, leaving an empty set.
   Capture revision_before at refresh start, then cost_backfill_days_since uses data_revision
   greater than that floor to select this round's rewritten unsealed days. New
   v29_backfill_day_selection_uses_revision_floor covers sealed exclusion/no new writes.
2. Empty-value filter semantics differed from query::Filters unknown handling. cost_summary/
   collect_events_for_pricing always included empty providers/models through OR provider_id='',
   incorrectly including unknown-provider events in specific-provider filters. Shared
   fold_filter_condition now matches casefold allowed values; empty/NULL only when filter
   includes unknown. v29_dimension_filters_do_not_include_empty_values: openai 9500 US cents
   excluding missing provider, unknown selects no_provider/unpriced, glm-5.3 model 3600 cents.
3. TTL select formerly used bind:value={null} relying on runtime types; changed to explicit
   string onchange conversion. set_settings validates nonempty provider-default fields and
   TTL 5/60 minutes only; rejects invalid saves.

Saving settings with pricing enabled triggers one background recalculation with stable
results, so existing detail need not wait for collection. Timezone rebuild clears other
timezones' unsealed daily costs, retaining sealed history like daily partitions. All-unpriced
panel now shows counts/reasons instead of hiding them under no-data.

Updated pricing.md finalized snapshot format/import JSON/validation, validation.md two V29
status entries, README two cost rows in scope map, i18n.md key count 292→329, M6 baseline
292 plus later additions including F2's 26.

<a id="门禁补跑2026-09-30本轮改动后"></a>

<a id="统一检查补跑2026-09-30本轮改动后"></a>

## Full checks rerun after changes, 2026-09-30

Repository-root npm run verify exit 0: Markdown/assets/scripts/UI/Svelte zero errors or warnings/
Cargo fmt/Clippy -D warnings/full Rust/Vite build. npm run test:browser exit 0: calendar
heatmap/ten languages/themes/timezones/stale responses/user isolation/members/pagination/idle polling.
