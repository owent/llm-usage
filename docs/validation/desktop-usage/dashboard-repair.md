# Dashboard repair validation, 2026-10-02

<a id="2026-10-02-看板修正验证"></a>

Status: this implementation, complete project checks, browser regressions and local database
copy validation finished. Environment: Windows x64, PowerShell 7.6.6, Node.js 24.21.0,
Cargo 1.98.1, Python 3.14.7. Default directory: repository root. Existing user changes
retained; no commit/push/dependency-range changes. Restricted-terminal
CreateProcessAsUserW failed: 5 was retried through harness permissions; read-only checks passed.

<a id="已确认根因"></a>

## Confirmed causes

- Total charts also drew input/output when total was unknown; filtering pie charts only by
  total removed Copilot names.
- One-click configuration exported to telemetry/, outside automatic roots. Actual spans
  were named chat plus model; parser accepted only bare chat. Files mixed logs/metrics,
  requiring structural filtering.
- Today's native database contained Copilot usage observations and call rounds. Claude
  spellings 4-8/4.8 split groups; token rows lacked calls and call rows lacked tokens.
- Pricing returned no_provider/channel_unknown before fallback, leaving almost everything
  unpriced after enabling costs. Local Kimi profile kimi-code/k3-256k maps to K3 in the
  official model table; GLM official seeds use zai, not solely api. Verified Opus 4.8 and
  dated GPT-4o mini were also added.
- Trend costs occupied three columns before a full-width heatmap; two-column weekly
  distribution and two three-column pies left gaps.

<a id="本机只读核验结果"></a>

<a id="本机只读证据"></a>

## Read-only local checks

The tool returned only models, statistics and field names, excluding prompts, responses,
tool content and real identities. Configured Copilot file had 1,196 mixed records and
30 CLIENT chat spans: input 937508, output 9480, cache_read 786070, cache_creation 149192,
reasoning 4702. Thirteen span session identities directly matched native records. No shared
turn/call ID existed; timestamps were not used for deduplication. These values describe
the file snapshot, not complete history or final daily usage.

<a id="验证结果"></a>

## Validation results

<a id="工程与界面"></a>

### Project and interface

| Command | Directory | Exit | Result |
| --- | --- | --- | --- |
| npm run verify | Root | 0 | 166 Markdown files, 82 assets, 3 script/13 UI tests; svelte-check clean; fmt/Clippy/Rust 787 pass, 2 explicit local audits ignored; frontend builds |
| cargo test --locked -p llm-usage-core --test dashboard_repair --test pricing_v29 | desktop/src-tauri | 0 | 11 new boundary regressions and all 10 V29 cases pass, included above |
| npm run test:ui / npm run check / npm run build:web | Root | 0 | Rechecked after final text fix; 13 UI tests, clean type check and successful build |
| npm run test:browser | Root | 0 | Headless Edge: single total series, agent names, one-period hover, partial shares, unknown detail calls, overview/trend API references, dense eight-metric layout, telemetry batch retry/revoke, ten languages, themes, narrow screens, filters/paging |

Screenshots in ignored build/browser-smoke/. Wide light/dark and narrow interactions passed;
trend light screenshot inspected. Browser uses a simulated bridge, not native Tauri GUI.
Final Markdown/link/diff checks are below.

<a id="本机副本端到端"></a>

### End-to-end local copy

Python SQLite backup API created an independent copy from a mode=ro connection to the
real application database, rather than copying a bare DB. Old copies remain in
build/dashboard-repair/. cargo run --locked -p llm-usage-core --example real_verify_dashboard
(desktop/src-tauri, exit 0) requires targets under repository-root build/, reads original
agent files and writes only the copy; output limited to statistics/unpriced reasons.

| Stage | Calls | Usage observations | Known input | Known output | Known total | Excluded records |
| --- | --- | --- | --- | --- | --- | --- |
| Initial native copy | 235 | 13 | 3,832,423 | 487,818 | Unknown | 0 |
| After OTel supplement and format selection | 252 | 11 | 4,403,216 | 334,829 | 946,988 | 15 |
| After another native-file scan | 252 | 11 | 4,403,216 | 334,829 | 946,988 | 15 |

All 30 valid OTel calls added; second OTel scan added zero with zero errors/conflicts.
Fifteen native records lose statistical contribution only; originals/revision history remain.
Replaying old native cursors advances revision 41→42 without changing statistics. Total
includes observable values only; native input lower bounds are not added to whole-turn output.

One-time price-rule backfill matched 2,577 events to USD official references, totaling
12,062 cents; fallback_event_count=2,577. Another 777 occurrence-time estimates remained
unpriced. Amount is an API reference for priceable components, not a subscription bill;
priced events are not equivalent to calls. Current-price simulation for 2026-09-01 through
2026-10-02 left channel_unknown=527 and no_price_row=226. Occurrence-time amounts remain
separate. codex-auto-review has no verified public model mapping and receives no guessed
GPT price. Missing tokens or historical effective price rows are not filled in.

<a id="兼容与限制"></a>

### Compatibility and limitations

Synthetic regressions cover consumed unchanged cursor upgrades, old trace+span identities,
same span ID across traces, repeated exports, collection order, host/user separation,
invalid records not claiming sources, sealed partitions, native rescans, model spellings,
exact Kimi profile price priority, official-provider boundaries, channel/currency ambiguity
and background price updates preserving occurrence-time estimates. Earlier pricing assertions
were updated for the explicitly authorized reference rules; unknown/ambiguous cases remain.

No IDE configuration was changed again; no model calls or production database writes.
Copilot exports cover only exported records and may miss earlier calls on activation day.
Updated clients apply scan rules on collection refresh without clearing the database.
Latest CLI/JetBrains telemetry, trace SQLite, automatic supplementary exports for other
agents, macOS/Linux desktop and native GUI packaging/installation were not accepted here.

<a id="收尾检查"></a>

### Final checks

npm run lint:md: exit 0, 166 clean files. python build/dashboard-repair/check-links.py:
exit 0, 113 relative file links present. git diff --check and git status --short -- build:
exit 0, no whitespace errors or temporary outputs in changes. Final review included
untracked source/tests/language resources/price JSON/design and validation documents.
Dependency versions and prior user changes retained.
