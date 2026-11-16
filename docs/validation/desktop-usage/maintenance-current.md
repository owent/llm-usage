# Current maintenance validation

Reviewed: 2026-11-16. Maintenance validation source: 0.3.9; release preparation: 0.3.10.
Environment: Windows x64, Node 26.13.0,
npm 12.0.2, pnpm 12.21.0 and Rust 1.99.1. Commands run from the repository root;
disposable scripts, logs, raw public catalog and isolated databases are under
`build/maintenance-20261116/`. The user subsequently authorized the version bump, commit,
annotated `v0.3.10` tag with changelog, push and release workflow. All manifest/lock versions
are synchronized; the existing workflow creates a draft after platform checks. The local
executable and full maintenance checks below precede the version-only bump; tagged CI
validates 0.3.10 separately. Release preparation artifacts use `build/release-0310/`.

## Plan assessment

Core storage, collection, desktop queries/settings/scheduling, costs, exchange, updates,
languages and documentation have implementations and existing scoped checks. The user
confirmed documentation deployment. [Plan.md](../../../Plan.md) now keeps current work
and unresolved conditions; [execution requirements](../../design/desktop-usage/execution.md)
consolidates completed delivery scope. Standalone Qoder IDE/JetBrains token readers and
other F1 candidates remain unimplemented/unverified. Native public-release/installer and
other-platform GUI update acceptance remain separate from code completion.

## Dependencies

Direct package versions were checked against the npm and crates.io registries. npm and
Cargo updates also refresh compatible transitive versions; both npm locks and the pnpm
workspace lock are synchronized. New versions include Starlight 0.43.1, Astro 7.4.12,
markdownlint-cli2 0.24.1, Playwright 1.65.1, svelte-check 4.7.8, Vite 8.3.8, rusqlite 0.43.0,
Jiff 1.0.0, rfd 0.18.0, notify-rust 4.19.0, flate2 1.1.11, toml_edit 0.25.19,
ureq 3.5.2 and zstd 0.14.4.
Other direct libraries were already current. TypeScript stays at 6.0.3: both
`@astrojs/check` and `svelte-check` require TypeScript 5 or 6, excluding registry latest 7.0.2.
No peer checks were disabled. rfd raises the application MSRV to 1.92; core remains 1.90.
The pinned/CI toolchain remains 1.99.1. Jiff consumers use the officially renamed
`ZonedDateTime`/`to_zoned_datetime` API; calendar/DST/scheduling tests remain required.

Root npm audit has zero findings. Desktop npm reports two low-severity findings through
the current Svelte Vite plugin's `deepmerge` dependency, with no compatible fix available
in its registry metadata. The [upstream advisory](https://github.com/advisories/GHSA-gcqx-w48m-w2h4)
is retained as a dependency limitation; neither overrides nor peer exceptions conceal it.

## Offline models

The bundled catalogue contains eight independently identified models, including
`qwen4-max-preview` and its documented `qwen4.0-max-preview` alias. Preview pricing stays
unavailable; it is distinct from `qwen4-max`. The [official Token Plan reference](https://docs.qwencloud.com/token-plan/personal/token-plan-personal-overview)
and [product updates](https://docs.qwencloud.com/token-plan/token-plan-product-updates)
do not turn Credits into currency. Qoder's `qmodel_latest` remains an unresolved routing key.

The new snapshot has nine price rows for seven models: GPT-6.1 Terra short/long,
Claude Fable 5.2, Gemini 4 Carbon current/December, Gemini 4 Hydrogen, and Qwen4-Max,
Qwen3.8-Max/Flash in native CNY. Sources are the official
[OpenAI](https://developers.openai.com/api/docs/pricing),
[Terra context tiers](https://developers.openai.com/api/docs/models/gpt-6.1-terra),
[Anthropic](https://platform.claude.com/docs/en/about-claude/pricing),
[Google](https://aistudio.google.com/docs/pricing) and
[Qwen](https://platform.qianwenai.com/docs/developer-guides/getting-started/model-prices)
price pages. Unrepresented/unknown cache TTLs remain unpriced. Existing snapshots and
historical estimates are preserved; new/old database imports, current references,
future intervals and unknown-preview rejection have dedicated regressions.

Public models.dev was used to find candidates, not to copy incompatible TTLs or plan
prices. The downloaded catalog has 324 providers, 6,655,210 bytes and SHA-256
`d94a595fb045db95d8be9705958cb97b0c0a2c51dfa6dd6ac8a74e23a49349b8`.

## Qoder CN

Installed VS Code extension: `Alibaba-Cloud.tongyi-lingma` 2.6.11, displayed as Qoder CN.
Its native database is `~/.lingma/vscode/sharedClientCache/cache/db/local.db`; earlier
discovery only covered CLI projects. Native tables contain one format-2 chat session,
two `FREE_INPUT` chat records and four messages, with one saved assistant per request.
Neither request metadata nor the inspected schema contains token usage. `extra.modelConfig.key`
is `qmodel_latest`; it does not identify the served model or billing provider. Extension
request construction confirms that the key is copied from the selected model configuration.
Session format version is not a historical extension-version marker.

The independent reader selects metadata only with a read-only SQLite/WAL connection.
It imports two usage observations with unknown tokens, costs and underlying model-call
counts; it does not read questions, answers, context, tools or accounts. Full-registry
repeat reads import no duplicates. Synthetic tests cover WAL-only mutation, default/manual
discovery, parent roots/arbitrary filenames, coexisting CLI/database files, pagination past invalid rows, old cursor
replay, empty misrouted ownership recovery, preserved diagnostics and transaction rollback.
CLI transcript regressions remain separate. Agent/edit modes, token-bearing persistence,
standalone IDE and JetBrains are not verified by these chat samples.

## Verification

| Command | Result |
| --- | --- |
| `npm ci` and `npm --prefix desktop ci` | Exit 0; both frozen npm installs pass. |
| `pnpm install --frozen-lockfile --strict-peer-dependencies` | Exit 0 with task-local store; supply-chain policy and build approvals retained. |
| Qoder regression coverage | Exit 0: four CN regressions and sixteen CLI transcript tests. |
| Explicit `cn_read_only_local_acceptance` with `QODER_CN_NATIVE_DB` | Exit 0: two native observations, repeat unchanged, token/cost unknown. |
| Targeted November/Kimi price tests | Exit 0: seven tests. |
| `npm run verify` | Exit 0: 1,167 Rust tests, ten explicitly ignored native/platform cases, 20 script tests and 32 frontend tests; clean Svelte/fmt/clippy and frontend build. The CN native case is run separately above. |
| `npm run test:browser` | Exit 0; Edge synthetic-IPC interaction checks. |
| `npm run check:docs` / `test:docs` / `build:docs` / `test:docs:browser` | Exit 0: 213 repository pairs, 21 guide pairs, 41 unit tests, 31 browser checks; 1,417 pages and 2,997 checked output files. |
| Windows release/executable checks | Exit 0: Tauri release build, eleven headless checks, three real CN collection checks and 27 native WebView2/IPC checks with twenty launches. CN imports two observations with unknown tokens/cost, no duplicates and no other sources. GUI tests use isolated synthetic Codex/OpenCode/Qoder CLI inputs. |

The final executable is `desktop/src-tauri/target/release/LLMUsage.exe`, SHA-256
`424aa933a355a6f87e7e1370061060d6cafa2f95b06061d87b625e76d9c3c9e5`.
Native first-screen P95 was 1,622.98 ms across twenty launches on this machine; this
does not repeat scale/resource acceptance or other-platform GUI checks.

First checks exposed the Jiff API rename, a missing native-envelope progress marker in an
invalid-first-page CN test, and fixed old seed-count assertions. The API, progress marker
and seed registration checks were corrected. The new reference document also required
updating the sidebar's fixed source-inventory assertion. pnpm's old transitive selection
used astro2tsx 0.1.3 with emnapi 2 beta and failed at runtime; a compatible recursive update
selects upstream astro2tsx 0.1.4, which explicitly declares emnapi runtime 1.11.3. No
override or peer bypass is needed. Sandbox npm, SQLite SHM and Pagefind output access failures
were rerun with the authorized filesystem access; they do not establish product failures.
These local checks do not establish remote CI, release publication, installer lifecycle or
Linux/macOS GUI acceptance for this source revision.

The initial plain Cargo release binary passed headless collection but lacked Tauri's
embedded-frontend build context, so the native GUI test could not find an application page.
The corrected command uses the installed Tauri CLI from `desktop`:
`node node_modules/@tauri-apps/cli/tauri.js build --no-bundle`. npm 12 does not forward the
extra `--no-bundle` flag through the nested root script. This command builds without installing.

The next GUI run correctly rejected October synthetic records under the default seven-day
detail retention, reporting `expired_by_retention` rather than importing them. The GUI
test now shifts its synthetic OpenCode/Qoder copies together into a recent historical
window, preserving every token/identity/version field and relative timestamp. Queries
use that window. Product retention and original fixtures are unchanged.
