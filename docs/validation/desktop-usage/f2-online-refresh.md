# F2 optional online price refresh, models.dev: Implementation validation, 2026-10-01

<a id="f2-可选在线刷新modelsdev实施验证记录2026-10-01"></a>

Scope: [online refresh design](../../design/desktop-usage/pricing.md#online-refresh), task 5:
official usage-based prices, excluding coding plans; long-lived models.dev cache; failed
downloads use the last successful cache; missing exact matches may use official-provider
usage prices. Includes schema v11, conversion/fetch/cache/fallback, commands/UI and A9
tests. No commit/push/deployment.

<a id="实施内容"></a>

## Implemented changes

<a id="1-数据源与过滤core-models_devrs"></a>

### 1. Source and filtering: core models_dev.rs

- Only built-in source: <https://models.dev/api.json>, MIT community catalog, S18/S19/S24.
  HTTPS GET through ureq 3.4.2/rustls sends no local usage/host identity/secrets.
- Official providers determined from data: ID matches a canonical_model_id prefix or its
  -cn variant. Exclude IDs containing -plan, including coding-plan/token-plan placeholders
  observed with all-zero cost. Skip models with both input/output zero or absent. Individual
  zero components become NULL; placeholder zero is not a free price.
- cache_write maps to cache_write_5m; cache_write_1h stays NULL because catalog lacks TTL
  tiers. Context tiers become context_threshold_tokens rows; region cn for -cn, otherwise
  global; channel api, currency USD. Dollars per million ×10⁴, rounded, become hundredths
  of a cent per million. Audio/reasoning prices not imported; duplicate context_over_200k ignored.
- Snapshot ID models-dev-YYYYMMDD-hash8 uses fetch date/raw FNV-1a; repeated content does not duplicate.

<a id="2-官方提供商回退匹配schema-v11"></a>

### 2. Official-provider fallback: schema v11

- price_versions.official_vendor defaults true for seed/community rows, false for manual;
  daily_cost_usage adds fallback_event_count.
- Existing exact matching remains. Only no_price_row adds one fallback among official_vendor
  rows for the same model, preferring configured region/channel and retaining tier/context/
  effective-date rules. channel_unknown does not fall back. Events mark official_fallback;
  daily/combined costs count fallback_event_count, labeled official fallback in UI.
- Prerelease behavior retained: no per-version migration; v10 uses existing backup→rebuild→rescan prompt.

<a id="3-抓取缓存失败回退desktop-price_refreshrs"></a>

### 3. Fetch/cache/failure recovery: desktop price_refresh.rs

- Raw cache: database-directory/price-cache/models-dev-api.json plus .meta.json. Temporary
  write+rename is atomic; corrupt metadata reconstructed from mtime. Default TTL three days,
  configurable 1–365; fresh cache imports without duplicate/network request. Refresh now bypasses TTL.
- A9: download/validation failure imports last successful cache. Without cache, report error
  and retain snapshots. Invalid responses never overwrite cache.
- refresh_prices_online(force) requires both cost/online options enabled and runs on a
  background thread; price_refresh_status exposes state. maybe_auto_refresh checks after
  collection, disabled by default; results logged as price_online_refresh.

<a id="4-界面与-i18n"></a>

### 4. Interface and languages

- Settings → Costs: Online price refresh panel with switch, validated 1–365-day TTL,
  Refresh now button polling status, cache freshness and latest result.
- Trend costs append official fallback count per currency.
- Thirteen keys across ten languages: twelve cost.refresh.* plus cost.fallbackCount;
  catalog 329→342.

<a id="真实数据验证2026-10-01windows-11-x64"></a>

## Real-data checks: 2026-10-01, Windows 11 x64

- GET <https://models.dev/api.json>: HTTP 200, 5,282,228 bytes in ignored
  build/price-refresh/models-dev-api.json.
- Temporary conversion probe, deleted afterward: production snapshot_from_models_dev
  yielded 473 rows/24 providers from 26 deemed official; poolside/sarvam lacked usage prices.
  Samples matched official pages: gpt-6-astra $10/$50/$1/$12.5, over-272K $20/$75; kimi-k3
  $3/$15/$0.3; glm-5.3 $1.4/$4.4/$0.26, zero cache_write→NULL. All rows official_vendor=true,
  USD/api; no -plan channels.
- Explicit ignored http_fetch_live_models_dev_smoke: real ureq/rustls HTTPS fetch and
  nonempty converted snapshot, exit 0.

<a id="测试与命令"></a>

## Tests and commands

| Command | Exit | Result |
| --- | --- | --- |
| cargo test -p llm-usage-core --lib | 0 | 254 pass, including 4 models_dev and 3 fallback cases |
| cargo test -p llm-usage-core --test pricing_v29 | 0 | 10 pass; v29_official_provider_fallback_end_to_end covers missing exact row fallback, exact match without fallback and daily/summary counts |
| cargo test -p llm-usage-core | 0 | All 67 test targets pass |
| cargo test -p llm-usage-desktop | 0 | 59 pass; 5 refresh cases: cached recovery, no-cache error preserving snapshots, fresh cache without network, invalid response preserving cache, successful repeated import |
| cargo test -p llm-usage-desktop http_fetch_live -- --ignored | 0 | Real HTTPS passes |
| cargo clippy --workspace --all-targets -- -D warnings | 0 | Pass after first items_after_test_module correction |
| cargo fmt --all --check | 0 | Pass |
| npm run verify (root) | 0 | Markdown, clean Svelte, fmt/Clippy/full Rust/Vite pass |
| npm run test:browser | 0 | Browser regressions pass |

<a id="缺口与后置项如实登记"></a>

## Limitations and deferred work

1. models.dev is community-maintained, not a primary official source. Observed differences
   include zhipuai using docs.z.ai USD prices. Spot checks do not verify every row. Content
   hashes distinguish corrected snapshots and repeated imports.
2. -cn entries remain USD: catalog lacks currency and uses docs.z.ai USD rates; CNY conversion
   not verified. Official CNY prices still require seed/manual snapshots.
3. Proxy/enterprise-network settings unsupported; ureq connects directly and failures use cache.
4. Freshness measures fetch time; TTL limits repeated local downloads, not upstream update frequency.
5. Estimates exclude cache-storage fees; usage_observation interval summaries remain unpriced,
   as in the F2 main limitation.
6. v11 uses prerelease backup→rebuild→rescan for existing local databases.
