# Total-usage charts, additional Copilot collection and official price references

<a id="总量图表copilot-补充采集与官方价格参考修正"></a>

Before repairs, inspect current source/lockfiles and permitted local fields. Results:
[repair record](../../validation/desktop-usage/dashboard-repair.md).

<a id="行为与兼容"></a>

## Behavior and compatibility

Current reference prices must cover the same retained range as usage. Select individual
details or archived summaries per source partition, using identical daily/weekly/monthly
archive rules. Hourly selection excludes whole-day totals. Archived summaries use saved
components only: no zero-filled splits or per-call context tiers inferred from daily input.
Multiple tiers without recoverable per-call tier show lower/upper costs and reasons;
known-component intervals do not represent full cost. Current references never overwrite
historical prices/archives. Retain integer-product precision, sum then round to avoid losing
small costs. Model details merge by source provider/spelling key; reference supplier/tiers/
snapshots appear within that model row.

Dashboard rules:

- Managed telemetry roots go only to OTel. Shared manual roots honor confirmed file ownership;
  another adapter cannot collect an already assigned physical file. Reevaluate old wrong
  registrations without deleting usage/revisions/diagnostics. Incorrect registrations without
  usage history in that directory do not appear as installed agents. Actual unknown manual
  formats keep diagnostics; missing historical sources gray out, permission errors remain read failures.
- Keep original model name, spelling comparison key and verified API reference identity separate.
  Spaces/underscores/hyphens may normalize for comparison; retain version decimals/unknown
  suffixes. Dynamic profiles preserve effective dates. Original source/channel/historical prices
  independent; model rows explain missing rates/usage.
- Missing cost-curve points stay gaps, known zero stays zero. ECharts missing sentinel never
  enters money formatting.
- Sum Copilot input/output only with complete matching coverage. Native turn's last-call
  input and whole-turn output provide a display-only observed-total lower bound, with explanation;
  never redefine it as complete total tokens or include in full-total statistics.
- Setting switches share visuals/focus/keyboard behavior; multiple choices retain checkbox
  semantics and clickable labels.

API reference cost card belongs in today/trend metric grid: eight columns wide, four medium,
two/one narrow. Account quota remains separate. Distinguish missing priceable source tokens
from no applicable price; partial estimates explain missing input splits/output/rate components.
Unknown tokens remain unknown; known-zero usage may price to zero with an applicable rate.

User-requested CNY→USD display preserves original CNY amount/rate alongside approximate USD,
without currency merging/history rewriting. Versioned ECB 2026-10-02 snapshot EUR/USD=1.1225,
EUR/CNY=7.5259, CNY→USD=1.1225/7.5259; display date/source, no automatic network refresh.
Unknown currencies/unsafe numbers unconverted. Amounts/rates converted and rounded in their
respective integer units. [ECB reference rates](https://www.ecb.europa.eu/stats/policy_and_exchange_rates/euro_reference_exchange_rates/html/index.en.html).

Codex automatic file reevaluation retains individual usage/revisions/comparison diagnostics.
Cumulative mismatch stays visible without independently lowering read health. New individual
records do not depend on snapshots; invalid legacy total/last fields still worsen health.
Official TokenCountEvent.info may be absent/null: no usage/call/zero filling. Invalid rows,
individual usage shape/time errors remain after later good batches. Register Kilo versions
only with redacted per-version samples; empty sessions without assistant usage cannot verify
format or a version range. Updated parser/version verification automatically rescans old
cursors. Compatibility notices explain automatic readable-usage checks and review after support
updates. Missing version verification and actual read errors stay distinct; notices cannot
be cleared through unsupported version claims. Kilo uses each message's session.version;
highest database version never replaces another session's basis. Mixed compatibility survives
incremental rounds, reevaluated after rule-upgrade replay.

- Total view draws total tokens only per group; unknown gaps remain, input/output in their
  own charts/tooltips. Share chart switches total/input/output; agents without totals keep
  names and explicit unknown values.
- Call charts show agent names/counts, including hoverable points for a single period.
  Native Copilot usage observations are not calls; verified hyphen/decimal model spellings group together.
- Automatically discover locally configured Copilot file exports. Accept only verified CLIENT
  chat spans, including chat followed by model; skip metrics/logs/invoke_agent/noncall records.
  Calls without reported tokens still count; cache fields unknown, reasoning subset of output.
  Small overview text says VS Code/Agent Host/latest CLI/JetBrains exports supplement per-call
  observations. CodeBuddy CLI already has local sessions; export is optional supplementation.
- Without common call IDs, choose native session or OTel contributions for each original
  host/user/session/statistical local day. Observed Copilot OTel replaces matching native
  contribution; other dates/sessions retain native data. No deduplication by equal time/tokens.
  Enabling-day exports may cover only later calls; show limited coverage. Preserve native
  records/revision history; later native replay cannot add them again. Missing session identity
  cannot replace native records. Same-source file/HTTP copies deduplicate by trace+span and
  original host/user. Equal span IDs in different traces remain different calls. Replay old
  span-only cursors, retaining old records/removing old contribution without clearing the DB.
- Costs off by default. Exact provider/model/user channel prices first. Without exact rows,
  missing provider/channel can still use verified official same-model API reference rates.
  Without channel preference choose official global, preferring api labels but also official
  provider-name channel labels. Otherwise choose official channel with definite currency;
  remaining channel/currency ambiguity stays unpriced. Preserve real provider/currency/
  fallback_event_count. Never price another model from the same series; subscription-only
  models without API rates stay unknown.
  Official Kimi model table maps k3/k3-256k to K3 for kimi-k3 reference only. Alias applies
  to bare model_raw k3/k3-256k from native Kilo/oh-my-pi custom Kimi providers and prefixed
  forms, official moonshot assignment. Keep original profile; exact profile-channel rates first.
  Strip custom namespaces only when prefix matches the record provider or verified prefix
  table. Verified backup profiles follow the same rule; unknown namespaces/suffixes retained.
  Official GPT-5.5 snapshot gpt-5.5-2026-04-23 may reference gpt-5.5; no inferred other dates.
  Official Kimi release notes map kimi-for-coding from 2026-09-11 to kimi-k2.8-preview,
  without changing earlier identity. No verified original-provider public API rate yet;
  current reference follows the explicit exception below, occurrence-time estimate remains missing.
  Tencent/other-channel prices are not labeled Moonshot official rates. Native official CodeBuddy
  catalog identifies hy4-preview-f as Hy4 preview, using Tencent official CNY reference.
  Centralize alias/spelling rules; exact catalog ID before spelling normalization/context tier,
  conflicts unpriced. All tiers from one preferred snapshot, without mixing new base/old long rates.
  User confirmed k28-agent-preview as Kimi K2.8 Preview on 2026-10-05 and authorized K2.7
  when official prices absent. Identity stays kimi-k2.8-preview. Current API reference uses
  official kimi-k2.7-code only without exact channel/same-model official rate. This listed
  cross-model exception does not apply to similar models or channel/tier ambiguity. Amounts/
  summaries/rate details identify substitute model. Occurrence-time estimates never use it;
  no historical amount rewriting.
- Overview/trend show API references/partial coverage/official fallback/unpriced reasons.
  Known usage observations can receive partial estimates; priced-event count is not call count.
  Background prices never rewrite occurrence-time estimates. Rule repairs recalculate retained
  detail on unarchived days once.
- Trend defaults: calls/usage side by side, costs full width, weekday/two share charts on one
  evenly divided row. User adjustments/old layouts retained; grid fills empty columns, narrow
  screens one column. Seven range metrics plus quota form eight cards, wide two rows/four
  columns, narrow two/one, without blank space beside quota.

<a id="官方供应商依据"></a>

<a id="官方供应商证据"></a>

## Official provider references

Page text checked 2026-10-02. Model series narrow candidate official suppliers only;
exact model must still have a price row, without invented series-based rates.

| Series | Official supplier/catalog ID | Source |
| --- | --- | --- |
| GPT/OpenAI o | OpenAI/openai | [OpenAI pricing](https://developers.openai.com/api/docs/pricing) |
| Claude | Anthropic/anthropic | [Claude pricing](https://platform.claude.com/docs/en/about-claude/pricing) |
| Gemini | Google/google | [Gemini pricing](https://ai.google.dev/gemini-api/docs/pricing) |
| Kimi/Moonshot | Moonshot AI/moonshot/moonshotai, including CN | [Kimi pricing](https://platform.kimi.ai/docs/pricing/chat) |
| GLM | Zhipu/zhipuai; international Z.ai/zai | [Z.ai pricing](https://docs.z.ai/guides/overview/pricing) |
| DeepSeek | DeepSeek/deepseek | [Official pricing](https://api-docs.deepseek.com/quick_start/pricing), public HTTP 200/23,982 bytes retained under ignored build/dashboard-repair/ |

Kimi profiles: [Kimi Code model table](https://www.kimi.com/code/docs/en/kimi-code/models.html);
dates: [release notes](https://www.kimi.com/code/docs/en/kimi-code/whats-new.html).
HY4 uses [Tencent official prices](https://cloud.tencent.com/document/product/1823/130055),
Guangzhou standard API CNY/million input 6/output 18/cache hit 0.3.
[2026-10-03 snapshot](../../../desktop/src-tauri/crates/core/prices/seed-2026-10-03.json)
effective from verification date; free CodeBuddy access does not establish free API pricing.
Local details/results: [second feedback record](../../validation/desktop-usage/feedback-round2.md).
[2026-10-02 snapshot](../../../desktop/src-tauri/crates/core/prices/seed-2026-10-02.json)
adds Claude Opus 4.8 and GPT-4o mini official 2024-07-18 snapshot. Standard USD/million:
Opus input 5/output 25/cache hit 0.50/write 5m 6.25/write 1h 10; GPT-4o mini input 0.15/
cache hit 0.075/output 0.60. Effective from verification date, without invented historical
dates or zero prices for absent cache components.

Copilot file shape: archived fixed [fileExporters source](https://github.com/microsoft/vscode/blob/dc546cc3c9979a19adafccd439889d7b64298def/extensions/copilot/src/platform/otel/node/fileExporters.ts)
and 30 native CLIENT chat spans. CLI individual/aggregate distinction:
[official OTel reference](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-command-reference#opentelemetry-monitoring).
Native VS Code acceptance does not verify new CLI/JetBrains versions. Input/cache inclusion
and reasoning output subset checked against [OTel GenAI semantics](https://opentelemetry.io/docs/specs/semconv/registry/attributes/gen-ai/).
SDK CLIENT=2 differs from OTLP CLIENT=3; receiver converts to text enums to avoid mixing them.

<a id="失败验证与回滚"></a>

## Failures, validation and rollback

Missing/damaged exports show existing observable coverage; no model calls to manufacture
samples. Original local files read-only; temporary validation databases/redacted output under
root build/dashboard-repair/. Regressions cover old cursors/rescans/read order, host/user
isolation, enabling-day range, hover, official reference with unknown channel, absent models
and multiple currencies. Commands/results in repair records. Rollback withdraws this round's
code only; original sources/price snapshots retained. Native records permit source reselection.
