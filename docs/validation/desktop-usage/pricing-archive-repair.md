# Price archives, precision and model-row merging: Acceptance results

<a id="价格归档精度与模型合并修复验收"></a>

Date 2026-10-05; Windows x64, Node.js 24.21.0, Rust 1.98.1. Implementation/rules:
[pricing](../../design/desktop-usage/pricing.md), [dashboard repairs](../../design/desktop-usage/dashboard-repair.md).

<a id="根因与修正"></a>

## Causes and corrections

| Problem | Confirmed cause | Correction |
| --- | --- | --- |
| Large historical usage displays only $0.01 | Usage reads retained daily summaries; current prices read only surviving usage_events, shrinking priced coverage after detail cleanup | Reuse usage-query archive selection; choose details/archives exclusively by instance/agent/original provider-model/call classification/quality/time |
| Some rows for the same model lack prices | Priced events archived; official GPT snapshot IDs and backup/custom routing prefixes unresolved | Restore archive pricing, add source-supported aliases, strip only custom namespaces matching the record's provider |
| Small costs disappear | Each event component rounds to cents before accumulation | Retain i128 integer products; sum by display day/provider/model/currency or indivisible archive period, then round |
| Duplicate model rows | One row per price tier, displaying reference supplier instead of source provider | One source provider/model row with tiers/snapshots within; group case/separator variants and outer provider whitespace consistently |
| Old/new tiers mixed | Candidate selection chooses greatest threshold across snapshots, mixing old long-context and new base rates | Choose preferred snapshot first, then its tier |

Current reference prices read usage/prices/revisions in one SQLite read transaction, without
changing sources or historical archived amounts. Occurrence-time estimate rule becomes
official-reference-5, correcting unarchived dates with retained details once. Later price
updates still do not rewrite history. Hourly archive selection now uses complete source
partitions, so another active model cannot hide an archived model on the same day. Weekly/
monthly archives reuse usage-selection rules, without invented daily curve points.

Archives lack individual context lengths. Fixed rates can be calculated directly; multiple
tiers show lower/upper amounts for known components. Old hourly/weekly/monthly summaries
without uncached fields remain partial; subtracting input/cache sums from different samples
cannot infer missing components. Known totals stay in the coverage denominator, preventing
output-only pricing from appearing complete. Indivisible archives spanning dynamic-alias
changes retain ambiguous model identity.

<a id="官方依据"></a>

## Official references

Official pages checked on 2026-10-05. Standard API reference USD per million tokens,
without claiming actual subscription charges:

| Model | Uncached input / cache read / output | Reference and handling |
| --- | --- | --- |
| gpt-5.5, gpt-5.5-2026-04-23 | 5 / 0.5 / 30 | [Official model page](https://developers.openai.com/api/docs/models/gpt-5.5) identifies snapshot and long-context multipliers |
| gpt-6-sol | 2 / 0.2 / 10 | [Official model page](https://developers.openai.com/api/docs/models/gpt-6-sol), cache write 2.5; long-context input/cache ×2, output ×1.5 |
| glm-5.3-flash | 0.15 / 0.03 / 0.50 | [Official Z.ai prices](https://docs.z.ai/guides/overview/pricing), existing seed reconfirmed |
| glm-5.3 | 1.4 / 0.26 / 4.4 | [Official Z.ai prices](https://docs.z.ai/guides/overview/pricing), existing seed reconfirmed |
| k3, k3-256k | 3 / 0.3 / 15 | [Kimi Code models](https://www.kimi.com/code/docs/en/kimi-code/models.html) establishes K3 identity; [Kimi API pricing](https://platform.kimi.ai/docs/pricing/chat) and Markdown list rates |
| k28-agent-preview | Substitute reference 0.95 / 0.19 / 4.00 | User confirms K2.8 Preview and authorizes K2.7 reference when exact prices absent; official kimi-k2.7-code standard API rates, substitute model labeled |

New seed-2026-10-05 preserves complete GPT-5.5/GPT-6 Sol short/long tiers, effective from
verification date without replacing old snapshots. Official threshold is input **greater
than** 272000, represented by integer 272001. Unknown GPT date suffixes/namespaces/channel
or currency conflicts still prevent pricing. kimi-for-coding as provider does not change
k3-256k identity; its dated model alias is a different mapping.

<a id="本机只读复算"></a>

## Read-only local recalculation

One read-only SQLite transaction extracts permitted model/source-class/date/token-component/
quality/count fields. Instance IDs hashed; no prompts/source paths/account credentials/session
text. Imported 146 daily summaries into a temporary in-memory DB, simulated complete archival
in the copy across 26 provider/model groups. Temporary data/scripts/independent calculations/
logs remain in ignored build/pricing-root-cause/.

- kimi-code/k3-256k: archived native tokens 992,415,571; surviving detail only one event of
  9,080 tokens. Original query priced only this event, rounding to USD 0.01.
- Asia/Shanghai retained daily summaries from 2026-01-01 through 2026-10-05 contain this
  combination on 12 days, 992,424,651 tokens. New engine USD 409.65 exactly matches independent
  integer multiplication and identical daily-partition rounding. This range does not establish
  the unspecified selection in the user's screenshot.
- k3-256k under kimi-code-owent/kimi-for-coding, plus kimi-code/k3-256k and
  kimi-for-coding-backup/k3-256k, all match official K3 reference. Local GLM-5.3/Flash
  provider groups regain prices; GPT snapshots/Sol archives display applicable ranges.
- Some GPT Copilot records lack priceable tokens and remain no_known_usage. A known rate
  cannot fill missing tokens with zero or invented values. Other unverified models retain missing prices.

<a id="执行结果与限制"></a>

## Results and limitations

| Check | Exit | Result |
| --- | --- | --- |
| npm run verify | 101 | Documents/assets/scripts 3/UI 21/Svelte/fmt pass; dependency resolution fails entering root-workspace Clippy |
| cargo test --manifest-path build/pricing-root-cause/rust/crates/core/Cargo.toml --offline --no-fail-fast | 0 | Isolated core 804 pass, none fail/ignored, including 10 new price tests |
| cargo clippy --manifest-path build/pricing-root-cause/rust/crates/core/Cargo.toml --offline --all-targets -- -D warnings | 0 | Isolated core clean |
| cargo run --manifest-path build/pricing-root-cause/rust/crates/core/Cargo.toml --offline --example live_price_review | 0 | Redacted daily-summary calculation matches independent expectations |
| Temporary desktop copy: cargo check --workspace --all-targets --locked --offline | 0 | Desktop command layer/core compile |
| Temporary desktop copy: cargo clippy --workspace --all-targets --locked --offline -- -D warnings | 0 | Whole workspace clean |
| Temporary desktop copy: cargo test -p llm-usage-desktop --locked --offline | 0 | App 95 pass, six existing explicit external/native tests ignored; Rust total 899 pass |
| npm run build:web | 0 | Production frontend build |
| npm run test:browser | 0 | Edge simulated IPC: one multi-tier model row, range/upper-bound curves, narrow-window overflow and existing interactions |
| git diff --check | 0 | No whitespace errors or untracked task temp files |

Root lockfile's foldhash 0.2.1 was unavailable in the current index/cache, before product
compilation. Existing user dependencies/lockfile retained. Copied Rust source into an isolated
core workspace under root build/, resolving the same declared dependencies offline; only
new version versus original lockfile was foldhash 0.2.0. This does not verify a complete desktop
build with the original lockfile. Further comparison found third-party core-graphics-types/
foldhash/option-ext/powerfmt versions changed from 0.2.0 to 0.2.1 without checksum changes.
Only in build/pricing-root-cause/app-validation/, restored those four to 0.2.0, preserving
app/core 0.2.1 and other locked records. The three desktop compilation/Clippy/app-test commands
add --manifest-path build/pricing-root-cause/app-validation/Cargo.toml --target-dir
desktop/src-tauri/target, reusing compilation cache. Source Cargo.lock unchanged. No release
installer/new desktop installation/native WebView2/actual IPC acceptance this round.

Regressions cover 600 million cached tokens before/after retention, exclusive detail/archive
pricing, 1,000 small calls summed before rounding, 272000 threshold, archive tier bounds,
original-partition isolation, hourly selection, weekly/monthly archives, aliases across dates,
unknown-component coverage, mixed old/new price snapshots and same provider/model across agents.
Screenshots in build/browser-smoke/: unit-prices.png and model-archive-range-narrow.png check
merged rows/archive ranges. Simulated IPC and native-data core recalculation remain separate
from native GUI acceptance.

<a id="用户确认后的-k28-替代参考"></a>

## User-authorized K2.8 substitute reference

On 2026-10-05, user explicitly confirmed k28-agent-preview as Kimi K2.8 Preview and authorized
K2.7 prices when official K2.8 rates are absent. This authorizes identity mapping and this
specific cross-model exception, without claiming official K2.8 pay-as-you-go prices. Kimi
official Markdown rechecked successfully: kimi-k2.7-code standard API USD per million
uncached input 0.95/cache read 0.19/output 4.00 matches existing versioned seed. highspeed
1.90/0.38/8.00 is not selected.

Identity/substitution remain separate: source model k28-agent-preview is preserved and
recognized as kimi-k2.8-preview. Only current reference pricing may use official kimi-k2.7-code
when no exact channel/same-model official row exists. Exact model price takes priority when
available. Channel/currency ambiguity, missing components in an existing price and unknown
tokens remain protected. Other similar model names do not match. kimi-for-coding resolves
identity by date; only dates confirmed as K2.8 receive this exception. Occurrence-time estimates
never use cross-model substitution; no new historical correction rule or old-price snapshot edits.

Substitute model propagates with amounts through daily curves/model subtotals/currency totals.
Amount cards label the substitute reference; model details/unit-price table show K2.8 identity
and actual K2.7 Code rate, in ten languages. Four pricing_substitutes.rs regressions cover
bare/routed IDs, exact-price priority, official-provider/unknown/ambiguity rules, and consistent
amount/substitution basis before/after archival. One million each uncached input/cache read/output
totals USD 5.14; amount and identity remain in aggregates.

Additional checks: isolated desktop copy pricing_substitutes/pricing_archives/model_reference/
pricing_v29/dashboard_repair, 42 pass, exit 0; whole-workspace Clippy clean, Svelte clean,
UI 21/frontend build exit 0. Browser exit 0 verifies short subtotal labels, original model
name, separate identity/substitute prices and narrow layout; build/browser-smoke/cost-substitute-narrow.png.
Initial test data changed the cost model to k28 while usage stayed GPT, failing association
assertion. Consistent identities on both paths pass without weakening assertions. Same four
temporary lock-record repairs as above; source lockfile unchanged.
