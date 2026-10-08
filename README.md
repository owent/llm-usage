# llm-usage

[![status](https://img.shields.io/badge/status-Pre--Alpha-orange)](Plan.md)
[![platform](https://img.shields.io/badge/platform-Windows_11_x64_first-0078D6)](docs/design/desktop-usage/platform-ci.md)
[![CI targets](https://img.shields.io/badge/CI-Windows_%C2%B7_macOS_%C2%B7_Linux-informational)](docs/design/desktop-usage/platform-ci.md)
[![scope](https://img.shields.io/badge/scope-Local_Agent_data_only-blue)](docs/design/desktop-usage/data-contract.md)
[![license](https://img.shields.io/badge/license-Not_specified-lightgrey)](https://github.com/owent/llm-usage)

[![Tauri](https://img.shields.io/badge/Tauri-2.12-FFC131?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-1.98.x-DEA584?logo=rust)](https://www.rust-lang.org)
[![Svelte](https://img.shields.io/badge/Svelte-5.57-FF3E00?logo=svelte)](https://svelte.dev)
[![TypeScript](https://img.shields.io/badge/TypeScript-6.0-3178C6?logo=typescript)](https://www.typescriptlang.org)
[![Node.js](https://img.shields.io/badge/node.js-%E2%89%A522_%C2%B7_24-339933?logo=nodedotjs)](https://nodejs.org)
[![Vite](https://img.shields.io/badge/Vite-8-646CFF?logo=vite)](https://vite.dev)
[![SQLite](https://img.shields.io/badge/SQLite-rusqlite_0.40-003B57?logo=sqlite)](https://www.sqlite.org)
[![ECharts](https://img.shields.io/badge/ECharts-6.1-AA344D?logo=apacheecharts)](https://echarts.apache.org)

[![verify](https://img.shields.io/badge/npm_run_verify-Passed_2026--10--07-brightgreen)](docs/validation/desktop-usage/current-acceptance.md)
[![Git LFS](https://img.shields.io/badge/Git_LFS-Static_assets-blue?logo=git)](desktop/assets/README.md)
[![repo size](https://img.shields.io/github/repo-size/owent/llm-usage)](https://github.com/owent/llm-usage)
[![last commit](https://img.shields.io/github/last-commit/owent/llm-usage)](https://github.com/owent/llm-usage/commits)
[![issues](https://img.shields.io/github/issues/owent/llm-usage)](https://github.com/owent/llm-usage/issues)
[![languages](https://img.shields.io/github/languages/count/owent/llm-usage)](https://github.com/owent/llm-usage)

LLM Usage is an AI usage dashboard: model tokens, requests and caching.
It is currently pre-alpha, with Overview, Trends, Details, Sources and Settings implemented.
It supports complete normalized detail import/merge and optional daily/monthly token or
single-currency estimated-cost reminders.

[Download the latest Release](https://github.com/owent/llm-usage/releases/latest) for Windows,
Linux or macOS in x64 or arm64. Portable packages use `.tar.zst`; a Windows x64 installer
is also available. See [installation instructions](https://llm-usage.atframe.work/start/installation/).

Collection reads local agent records and can be scheduled in the UI. The
[adapter matrix](docs/design/desktop-usage/adapters.md) lists supported formats, versions
and source limitations.

[![English Overview with today's calls, token totals and cache-read share](docs/site/public/screenshots/en/overview-light.png)](docs/site/public/screenshots/en/overview-light.png)

*Overview in version 0.2.1: actual Windows desktop UI with isolated synthetic demo data.
Select the image to view the original 2880×2000 PNG. The documentation homepage also
illustrates trends, record details, source management and settings in both themes.*

<a id="文档"></a>

## Documentation

[English documentation](https://llm-usage.atframe.work/) ·
[Chinese documentation](https://llm-usage.atframe.work/zh-cn/) ·
[Chinese repository README](docs/zh-CN/README.md)

The Astro/Starlight site includes user and developer guides, design specifications and validation records.
English is the default; the entry page selects a supported browser language and stores explicit choices.
Both light and dark themes are available. Repository documents and source comments use English,
with Chinese counterparts maintained alongside them.

- [Execution plan](Plan.md): current results, execution scope and deferred conditions.
- [Implementation readiness](docs/design/desktop-usage/implementation-readiness.md): confirmed scope and real-data validation procedure.
- [Detailed design](docs/design/desktop-usage/README.md): architecture, statistics, adapters, configuration and tests.
- [Research sources](docs/design/desktop-usage/research.md): official sources, fixed code and static prototype review.
- [Collection scheduling](docs/design/desktop-usage/scheduling.md) and [platform/CI](docs/design/desktop-usage/platform-ci.md): background and platform requirements.
- [Validation records](docs/validation/desktop-usage/): actual execution records starting at M0.
- [Application assets](desktop/assets/README.md): Usage U design, previews, regeneration and Git LFS.
- [previous-draft](previous-draft/README.md): historical reference; runtime behavior has not been accepted.
- [AGENTS.md](AGENTS.md): shared engineering rules.
- [Skills](.agents/skills/README.md): maintenance workflow and resources loaded as needed.

<a id="常用命令"></a>

## Common commands

Use Node.js 22+ (24 in current CI) and Rust (1.98.x in current CI). The Astro site requires Node 22.12+.
Run commands from the repository root:

```powershell
git lfs install --local  # Enable LFS for this clone once.
git lfs pull            # Download actual icons and static/binary assets.
npm ci                  # Restore root documentation/tool dependencies.
npm --prefix desktop ci # Restore desktop tool dependencies.
npm run assets:check    # Check asset formats, derivatives and LFS attributes.
npm run lint:md         # Check Markdown.
npm run dev:web         # Vite frontend only, http://127.0.0.1:1421, without a backend.
npm run dev:desktop     # Debug GUI and hot reload, without packaging.
npm run check           # Frontend types with svelte-check.
npm run build:web       # Build frontend output.
npm run test:rust       # Rust tests.
npm run test:ui         # Pure frontend logic regressions.
npm run test:browser    # Five-page mock-IPC browser checks; installed Edge on Windows.
npm run clippy          # Rust static checks with -D warnings.
npm run fmt:check       # Rust formatting.
npm run build:desktop   # Release desktop packages, according to platform.
npm run test:headless   # Real executable/SQLite, isolated synthetic sources; build first.
npm run test:desktop    # Windows native WebView2/IPC, requires CDP; build first.
npm run verify          # Docs, types, script/UI tests, Rust checks/tests and frontend build.
npm run dev:docs        # Astro documentation development server.
npm run check:docs      # Translation/content checks and Astro types.
npm run build:docs      # Production site and local-link checks.
npm run test:docs       # Documentation behavior regressions.
npm run test:docs:browser # Production-site browser checks.
```

`test:browser` starts and stops its own Vite server and writes screenshots to `build/browser-smoke/`.
On other platforms, first run `npx playwright install chromium` inside `desktop`.
Mock IPC does not replace native desktop, system-task or installation acceptance; see
[current acceptance](docs/validation/desktop-usage/current-acceptance.md) for results and gaps.
Native/headless harnesses use isolated synthetic sources and write to root `build/plan-completion/`.
`test:desktop` defaults to release; a debug build can use `-- --dev --exe <debug executable>`.
Use `dev:desktop` for ordinary functional checks without packaging. Business commands and resolved
versions come from the desktop/Cargo manifests and locks. Documentation checks do not verify runtime
behavior; M0 measurements are retained in the validation records.

Windows background collection is off by default. Settings can enable the current user's minute task
and display requested/effective state. `LLMUsage.exe --headless` follows saved intent and due rules;
`--scan-once` manually scans all enabled sources. `--data-dir <absolute directory>` changes application
storage and exports, not discovery. Enable **Only scan manually added directories** to restrict both
GUI and background discovery to the saved roots.

Images (including SVG), fonts, media and binaries use Git LFS. Download real assets before building.
Regenerate icons with `npm run assets:generate`. For the asset preview, run `npm run dev:web` and open
`http://127.0.0.1:1421/asset-preview.html`. Documentation output is in `build/documentation-site/dist/`.
