# llm-usage

[![status](https://img.shields.io/badge/status-Pre--Alpha-orange)](Plan.md)
[![release](https://img.shields.io/github/v/release/owent/llm-usage)](https://github.com/owent/llm-usage/releases/latest)
[![CI](https://github.com/owent/llm-usage/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/owent/llm-usage/actions/workflows/ci.yml)
[![license](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

[![total release downloads](https://img.shields.io/github/downloads/owent/llm-usage/total)](https://github.com/owent/llm-usage/releases)
[![Windows x64 portable size](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fgithub.com%2Fowent%2Fllm-usage%2Freleases%2Flatest%2Fdownload%2Fbundle-report-windows-x64.json&query=%24.bundles%5B0%5D.bytes&label=Windows%20x64%20portable&suffix=%20B&color=blue)](https://github.com/owent/llm-usage/releases/latest)
[![repo size](https://img.shields.io/github/repo-size/owent/llm-usage)](https://github.com/owent/llm-usage)

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

Use Node.js 22+ (24 in current CI) and Rust (1.99.0, selected by `rust-toolchain.toml`). The Astro site requires Node 22.12+.
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

Alternatively, `pnpm install --frozen-lockfile` restores the root and desktop workspace
using the pinned pnpm 12.10.1. The same commands support `pnpm run`; keep both npm locks
and the shared pnpm lock synchronized when changing dependencies.

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
