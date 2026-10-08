# Third feedback round: today's curves, cost layout and Junie sources

<a id="第三轮反馈今日曲线费用布局与-junie-来源"></a>

2026-10-03, following [second-round corrections](feedback-round2.md). Source inspection
and read-only local database snapshots identified the issues. No production-database
or Agent-file writes. Scripts, snapshots, replay databases and logs are in ignored
build/feedback-round3/; browser screenshots are in ignored build/browser-smoke/.

<a id="原因与修复"></a>

## Causes and corrections

- Today's hourly grouping branch used calls; a single hour used a separate call bar
  chart. Changing only the overview curve could not fix grouped views. Model, Agent
  and Agent-plus-model now share hourly token curves, including a single hour's axis.
  Known zero displays zero; missing complete totals remain gaps, without replacing
  them with calls or input lower bounds.
- Token tooltips repeated the whole Copilot explanation and input/output after every
  series missing total. Shared compact forms are name: value, name: ≥ value, or name: —.
  Explain lower bounds once; full explanations remain in the short note's tooltip.
  Shared chart code limits width and wraps long names.
- Cost summary cards repeated partial-estimate counts while panels expanded every
  coverage explanation, adding height. Cards show compact original-currency amounts
  and applicable USD reference, with coverage counts in tooltips. Panels retain amounts,
  gap counts and curves; coverage/model prices expand on demand and wrap in narrow windows.
- Junie errors came from an empty false registration created by the old telemetry-root
  broadcast. The actual snapshot's instance directory was the application telemetry
  parent, with no files, usage or diagnostics. Both failures were source_files.file_id
  uniqueness conflicts. Junie discover promotes the supplied events.jsonl directory
  to its parent. Last round's exact-original-root recovery missed this transformed
  instance; it was not failure to read a newly discovered native Junie session.

Recovery now runs discover across the whole adapter registry to reproduce candidates
from old managed roots, then repairs false registrations without usage history by exact
adapter/path. Retain settings, files, diagnostics and period history; real-source errors
remain visible. Regressions cover parent promotion and repeated recovery rather than
adding another Junie-specific exception. Requirements are synchronized in
[dashboard interaction](../../design/desktop-usage/dashboard-polish.md),
[Copilot telemetry](../../design/desktop-usage/copilot-otel.md) and root AGENTS.md.

<a id="验证"></a>

## Validation

Windows x64, Node.js 24.21.0, installed Edge and locked dependencies.

| Check | Result and scope |
| --- | --- |
| npm run verify | Exit 0; Markdown/assets, three script tests, 20 frontend tests, Svelte, fmt, Clippy, 820 Rust tests and web build passed |
| source_routing | Five passed: actual discover candidates, targeted managed-root routing, physical-file ownership, history retention and repeat recovery |
| Local snapshot-copy recovery | Both runs exited 0; visible erroneous Junie instances 1→0, false registration retained as not_applicable; event/file/diagnostic counts unchanged |
| npm run test:browser | Exit 0; actual Edge with simulated IPC; multi-hour/single-hour curves for all three groups, actual token data, zeros and gaps |
| Tooltips/layout | Tooltip≤482 px with no repeated long explanation; dual-currency summary card<115 px, panel 478 px; 760 px window has no horizontal overflow with coverage/prices expanded |
| npm run check | Exit 0 again after final layout adjustment; Svelte zero errors/warnings |
| npm run build:desktop | Exit 0; Windows x64 release executable and NSIS 0.2.1 installer, about 3.59 MiB |

Complete validation log: verify.log. After final layout adjustment, browser-final.log
and the desktop build's web build checked the result. Junie-copy recovery: probe.log.
Screenshots visually checked: today-token-groups.png, token-tooltip-compact.png,
cost-summary-compact.png, cost-panel-compact.png, cost-details-narrow.png.
Desktop build log: build-desktop.log. Installer:
desktop/src-tauri/target/release/bundle/nsis/LLMUsage_0.2.1_x64-setup.exe.

Browser regressions do not verify native GUI/IPC. This batch installed/started no new
application and did not write the production database. Normal collection with the
updated application restores false registrations without clearing the database.
No commit, push or deployment.
