# Second feedback round: Price matching, source routing and controls

<a id="第二轮反馈费用匹配来源路由与控件"></a>

2026-10-03. Repairs followed review of ab647fd, de8727f, current implementation and read-only
local data. Rules: [dashboard](../../design/desktop-usage/dashboard-repair.md),
[pricing](../../design/desktop-usage/pricing.md), [Copilot telemetry](../../design/desktop-usage/copilot-otel.md).

<a id="本机核验结果"></a>

<a id="本机证据"></a>

## Local checks

Consistent backup from a read-only SQLite connection; no production database/agent-file writes.
Output limited to models/counts/token buckets/status/error categories, excluding conversations,
tool content, credentials and project paths. Temporary work: ignored build/feedback-round2/.

Asia/Shanghai 2026-10-03 snapshot had exactly 199 unpriced records:

| Model | Count | Finding |
| --- | ---: | --- |
| copilot-nes-lysithea-24 | 178 | Local OTel completion model ID, provider absent, no verified price row; cannot guess Claude/GPT mapping |
| codex-auto-review | 18 | Routing ID without actual underlying model; cannot use main Codex price |
| hy4-preview-f | 2 | Official CodeBuddy local catalog maps ID to Hy4 preview; missing reference alias/price, repaired |
| kimi-for-coding | 1 | Official Kimi identifies K2.8 Preview, original-vendor public usage rate not verified; model identified, price remains missing |

Hyphen matching did not cause all 199. Same snapshot after repair: **197** unpriced today.
All retained-detail dates: 770, comprising 582 automatic review, 178 NES, 9 Kimi and 1
token-free Opus record. Counts are snapshot-specific and change with later use.

Official name: **Hy4 preview**. [Tencent pricing](https://cloud.tencent.com/document/product/1823/130055)
verified Guangzhou standard API CNY per million tokens: input 6, output 18, cache hit 0.3.
New snapshot starts 2026-10-03 without invented historical prices. Temporary free CodeBuddy
access does not make API pricing free.

[Official Kimi models](https://www.kimi.com/code/docs/en/kimi-code/models.html) and
[2026-09-11 release notes](https://www.kimi.com/code/docs/en/kimi-code/whats-new.html) identify
kimi-for-coding's in-place K2.8 Preview upgrade. [Moonshot pricing](https://platform.kimi.ai/docs/pricing/chat)
did not provide a verified rate for that model. Tencent TokenHub's K2.8 rate is another
channel, not Moonshot's. This round retained original-vendor reference rules, without
substituting K3/K2.7 or another channel's rate.

All retained Opus 4.8 names matched: **five partial estimates and one unpriced record**:

- One native VS Code observation: input 67,162, output 14,715; input is last-call lower
  bound, cache split/complete total unknown; estimate known priceable components only.
- Four VS calls with known input/cache-read/output, unknown cache-write/uncached input.
- One native round marker, all token fields absent; counts a call, no priceable usage.

<a id="修复"></a>

## Fixes

1. Three model layers: preserve original names; normalized space/underscore/hyphen comparison
   key; date-specific verified API-reference aliases. Handle Claude decimal spelling separately;
   retain other decimal versions/unknown suffixes. Exact IDs first; reject conflicting catalog
   spellings; choose tiers after model resolution. SQL/grouping/cost cache/UI share rules;
   dynamic alias cache includes date resolution. official-reference-4 repairs retained-detail
   unsealed dates once; price refresh preserves history.
2. Show unpriced reasons/reference model per model. ECharts may pass '-' gaps into tooltip;
   amount formatting produced NaN. Format finite numbers only, show dash for missing, zero for known zero.
3. When Copilot total is missing but input/output known, tables/tooltips show their sum.
   Native turns only establish a lower bound: show ≥ and explanation, without writing a
   complete total or changing total-chart gaps/exports. Complete per-call formats still
   derive exact totals. Large integer addition preserves precision.
4. Scanner broadcast managed Copilot telemetry roots to all adapters: Pi registered
   events.jsonl as unknown, then others hit source_files.file_id uniqueness. Real OMP/VS Code
   roots were valid; another false source caused errors. Directed managed-root routing now
   gives each physical file one valid owner. Empty false registrations disappear; wrong
   file registrations without cursor/usage history can return to the correct adapter.
   Disabled settings/diagnostics/revisions/period history remain. Corrupt manual files still
   show errors; absent historical sources gray out; permission errors are not “not installed”.
5. otel reads exports; vscode-copilot-chat reads chatSessions. Select contribution by host/
   user/session/local day; file/HTTP copies deduplicate by trace+span, never close timestamps
   or equal tokens. First-line metrics identify format but do not count as calls.
6. Consistent toggle sizing/focus/keyboard/clickable labels; multiselect keeps checkbox
   semantics; respect high contrast/reduced motion.

<a id="验证与范围"></a>

## Validation and scope

Windows x64, Node.js 24.21.0, current npm locks. Failure logs retained. Sandbox Vite spawn
EPERM required permission to run existing npm checks outside the sandbox.

| Check | Result/scope |
| --- | --- |
| npm run verify | Final exit 0; Markdown/assets/3 script/19 frontend/Svelte/fmt/Clippy/819 Rust/Web build pass |
| npm run test:browser | Final exit 0; real Edge + mock IPC: missing/zero amounts, per-model reason/reference, Copilot bounds, gray sources, labels/keyboard toggles, themes/ten languages |
| model_reference/source_routing/pricing_v29 cargo tests | Exit 0; alias dates/ambiguity/tier priority/all-adapter routing/old registrations/period history/repeated snapshots |
| Two real-copy collections | One healthy source each for Pi/OMP/native VS Code/OTel; no read errors; all add zero second run |
| Historical comparison | Eight usage-free false registrations hidden; zero existing events deleted/revisions regressed; no unrecognized files in four sources |
| npm run build:desktop | Exit 0; Windows x64 EXE/NSIS 0.2.1, installer about 3.59 MiB |

Final logs: build/feedback-round2/verify-final.log, browser-final.log, build-desktop.log,
probe.log, replay-comparison.json. Light/dark toggle screenshots in build/browser-smoke/
visually checked. Installer: desktop/src-tauri/target/release/bundle/nsis/LLMUsage_0.2.1_x64-setup.exe;
executable: desktop/src-tauri/target/release/LLMUsage.exe.

First OMP run added two records relative to backup, second zero; other sources zero both
runs. No production rewrite/database clearing needed; normal collection after update repairs
state. Browser success is separate from native GUI/IPC acceptance. Running application was
not started/replaced; no installation/commit/push/deployment.
