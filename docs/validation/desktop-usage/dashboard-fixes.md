# Dashboard feedback repairs and uncommitted-code review, 2026-10-03

<a id="看板用户反馈修复与未提交代码复审2026-10-03"></a>

Reviewed eight fixes in the 2026-10-02 tree against source/tests/official protocols/native data.
Corrected earlier claims about no currency conversion, the 658-count composition and whole
Kilo database readiness. Production database/settings read-only; write isolated copies only.
No commit/push/deployment this round.

<a id="元信息"></a>

## Run information

| Item | Value |
| --- | --- |
| Date | 2026-10-03 |
| Environment | Windows 11 x64; Node 24.21.0; Rust 1.98.1; headless Edge. Desktop dependencies restored from lockfile: Tauri API/CLI 2.12.0, Svelte 5.57.1, TypeScript 6.0.3, Vite 8.3.1 |
| Revision | Uncommitted frontend charts/settings/cost cards and core pricing/adapters/health changes |
| Design references | [Interactions](../../design/desktop-usage/dashboard-polish.md), [repairs](../../design/desktop-usage/dashboard-repair.md), [pricing](../../design/desktop-usage/pricing.md), [data rules](../../design/desktop-usage/data-contract.md), [adapters](../../design/desktop-usage/adapters.md), [architecture](../../design/desktop-usage/architecture.md) |

<a id="问题与修复"></a>

## Problems and repairs

| # | Feedback | Cause | Repair |
| --- | --- | --- | --- |
| 1 | Today chart became bars | TodayHourly.renderTotal() calls series type:bar | Calls/tokens both smooth lines; agent comparison within one hour retains bars |
| 2 | Cache lifetime days shows 0 | Derived PricingSettings Default sets online_cache_ttl_days=0; 0 ?? 3 stays 0 | Manual default=3, DEFAULT_TTL_DAYS; load_settings resets zero/out-of-range to 3 |
| 3 | API reference is text only | Previous repair left it outside metric grid | Eighth bordered today/trend card; short ten-language API reference cost heading, full hover explanation. Eight/four/two/one columns, quota separate. Widths 1920/1280/1024/760/480: no overflow, equal-width columns, one-line headings |
| 4 | Are all currencies USD; is conversion needed? | models.dev and native CNY rates differ | models.dev USD/million tokens; native snapshots USD/CNY. Rates stored in hundredths of a cent/million tokens; 1M input+1M output regressions yield USD 18/CNY 120. CNY amounts/rates show approximate USD alongside, with ECB date/source; currencies/history retained |
| 5 | k3-256k has no applicable rate | Native provider=kimi-code-owent, bare model_raw=k3-256k; reference_model_key only knew prefixed kimi-code/k3-256k | reference_model_key/official_providers also map bare k3/k3-256k to kimi-k3/moonshot, using official USD reference |
| 6 | 658 unpriced/9 partial estimates | Earlier explanation mixed current reference/occurrence-time prices and treated known-zero usage as unpriced | Distinguish no tokens/missing price/partial coverage; known zero prices as zero with applicable rate. Original snapshot after repair: 622 unpriced/9 partial, detailed below |
| 7 | Source needs 38 file checks | Cumulative mismatch treated as read failure; valid null info treated as invalid fields | codex-rollout-3 replays old cursors automatically; mismatch/regression retained without independently worsening read health. New individual records and legacy required total/last fields handled separately. Invalid rows/times survive batches. Copy's 38 files all reevaluated active |
| 8 | Kilo reads one file compatibly | 7.8.1 unregistered; highest DB version incorrectly applied to every message | Native 34-call sample registers 7.8.1 and reevaluates old version metadata/cursors. kilo-message-tokens-2 uses each message's session.version, preserving mixed-DB compatibility across incremental reads. Empty 7.8.3 session cannot verify format; notice explains automatic checks/review |

<a id="658-与-9-的准确解释"></a>

## Accurate explanation of 658 and 9

Counts refer to **current API reference** in the original application snapshot, without
substituting occurrence-time price history.

- 564 codex-auto-review records: route name is not a known actual model; no guessed GPT-series price.
- 34 k3-256k records: missing bare profile alias, fixed; all priced with official USD reference.
- 58 Copilot round markers: no token usage, retain calls without zeros/invented amounts.
- Two GLM known-zero records: old condition misclassified zero amounts as no priceable components, fixed.

Read-only native route files contain turn_context codex-auto-review plus individual call
identity/usage, without underlying model fields. Token counts/quota models cannot establish
the actual model. Same snapshot after repair: 622 unpriced, 564+58. Nine partial estimates
are five VS Code usage observations and four Visual Studio calls: output/some cache known,
without complete uncached-input split. Whole-turn input_total cannot use uncached rates
directly. Unknowns remain unknown; newly collected records can change these counts.

<a id="官方来源与自动核对边界"></a>

## Official references and automatic-check limits

Sources reread on 2026-10-03:

| Reference | Applied fact |
| --- | --- |
| [Kimi Code models](https://www.kimi.com/code/docs/en/kimi-code/models.html), [global prices](https://platform.kimi.ai/docs/pricing/chat) | k3/k3-256k are K3 profiles; per million input USD 3/cache read 0.30/output 15/cache writes 5m 3, 1h 6, tax excluded; membership quota is not API rate |
| [models.dev maintainers](https://github.com/anomalyco/models.dev) | cost is USD/million tokens; a provider's CN name does not justify converting again |
| [ECB daily XML](https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml) | Effective 2026-10-02, EUR/USD=1.1225, EUR/CNY=7.5259; CNY→USD=1.1225/7.5259 |
| [Codex 0.144 official protocol](https://github.com/openai/codex/blob/rust-v0.144.0/codex-rs/protocol/src/protocol.rs), [quota-event issue](https://github.com/openai/codex/issues/14489) | TokenCountEvent.info is Option; local 0.144.0-alpha.4 info:null/rate_limits:object valid. token_count can repeat during quota refresh, without another call |

Exchange rates are code-versioned reference snapshots, without automatic network requests.
Amounts/unit prices use their integer units and same-date cross rates/rounding; original
value/approximation/date/source shown. Historical values/original-currency subtotals unchanged.
Historical pricing rule official-reference-3 corrects only retained, unarchived dates.

Original 38 Codex files all reevaluated active; mismatch retained. Healthy reading does not
establish complete source coverage. New 0.159.0-alpha.12.1 still latest-compatible, without
verification based on same-shaped fields. Five regressions cover consumed unchanged bytes,
monotonic revisions/history, reads split by byte limits, null info and invalid records.

Kilo 7.8.1 native sample's 34 calls independently match 3,189,308 tokens; automatic replay
does not double count. Live DB includes empty 7.8.3 session without assistant usage. Full
check of 256 message-containing sessions still has one historical cumulative mismatch,
retaining diagnostics/degraded health. Whole database readiness is not established.
Empty/mixed-version tests ensure highest version does not verify other sessions or replace
the basis of already verified messages.

<a id="命令与结果"></a>

## Commands and results

Repository-root cwd unless specified:

| # | Command | Exit | Result |
| --- | --- | --- | --- |
| 1 | npm --prefix desktop ci --ignore-scripts | 0 | Restore locked dependencies, no lockfile changes |
| 2 | npm run verify | 0 | Documents/assets/scripts/UI/Svelte/fmt/Clippy/Rust/frontend build all pass |
| 3 | npm run test:rust within verify | 0 | 71 test binaries; 811 pass/0 fail/3 ignored; dashboard_repair 14, Codex replay 5, Kilo 8, pricing settings 5 |
| 4 | npm run check, svelte-check | 0 | Zero errors/warnings |
| 5 | npm run build:web | 0 | Frontend builds |
| 6 | npm run test:ui | 0 | 16 pass; ten languages, exchange units/rounding, coverage reasons |
| 7 | npm run test:browser, Edge | 0 | Curves/cost cards/resolutions/TTL=3/CNY amount-rate/reasons/compatibility/existing regressions |
| 8 | npm run fmt:check / npm run clippy | 0/0 | Formatting/lint pass |
| 9 | npm run lint:md | 0 | 171 files/0 issues |
| 10 | python -X utf8 build/review-feedback/run_live_probe.py | 0 | Native read-only sources into isolated DB; 38 files reevaluated, 34 K3 calls priced, version/history diagnostics retained |
| 11 | python -X utf8 build/review-feedback/audit_summary.py | 0 | Permitted-field redaction passes; 34 native backup records and sample have equal independent five-component totals |
| 12 | git diff --check | 0 | Final tree; untracked code/test data inspected separately |

<a id="失败与未执行项"></a>

## Failures and unexecuted checks

| Item | Status | Reason | Next condition |
| --- | --- | --- | --- |
| codex-auto-review pricing | Unexecuted | Route has no public rate, actual model unknown | User mapping/configuration can enable pricing; no guessing |
| Production degraded/compatible file replay | Isolated copy verified, production unchanged | Original configuration/DB read-only | First collection after update automatically replays, no clearing required |
| New native GUI acceptance | Unexecuted | New GUI not launched here | Desktop checks alongside build:desktop |

Initial assets check failed installed Tauri 2.11.5 versus lockfile. Restoring dependencies
passed 67 derived assets/82-file checks, without regenerating/editing icons. Node sandbox
child-process EPERM retried through authorized execution. fmt found missing --all; Clippy
found new syntax incompatible with repository MSRV; both fixed. Final logs:
build/review-feedback/verify-final.log and browser.log. No release bundling, default-ignored
remote models.dev smoke or actual telemetry-configuration write checks.

<a id="验证产物"></a>

<a id="证据文件"></a>

## Validation files

- Read-only consistent snapshots/scripts/isolated DBs/logs in ignored build/review-feedback/.
  SQLite mode=ro plus Online Backup, without immutable skipping WAL.
- New desktop/src-tauri/crates/core/tests/fixtures/kilo/session-7.8.1-k3.sanitized.json:
  IDs/paths/message text removed; tokens/time/finish/modelID/providerID fields preserved.
  session-7.8.1-k3._expectations.md contains independent calculations.
