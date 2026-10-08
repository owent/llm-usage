---
title: Tests and acceptance results
description: Match each claim to the validator that actually exercises it.
sidebar:
  order: 4
---

Run from the repository root with the locked dependencies restored. Read `package.json`
for exact definitions; the table explains the scope rather than replacing those commands.

| Command | Checks performed |
| --- | --- |
| `npm run verify` | Markdown, assets, script/UI logic, Svelte types, Rust fmt/clippy/tests and frontend production build. |
| `npm run test:browser` | Five-page browser interactions through simulated IPC; Windows uses installed Edge. |
| `npm run build:desktop` | Platform-specific Tauri release build and packages. |
| `npm run test:headless` | Real executable and SQLite with isolated synthetic sources; requires a build. |
| `npm run test:desktop` | Windows WebView2 and actual IPC; requires a build and an available CDP endpoint. |
| `npm run test:import` | Windows first GUI import at scale and full-process resource measurement. |
| `npm run test:receiver` | Windows actual IPC/HTTP/system credentials with owned-token cleanup. |
| `npm run test:install:windows -- --help` | Side-effect-free prerequisites for real NSIS lifecycle acceptance. |
| `npm run test:install:linux -- --help` | Side-effect-free prerequisites for isolated Podman package/GTK/WebKit/FUSE/Orca acceptance. |
| `npm run test:credentials:linux` | Native Linux Secret Service round trip on independent D-Bus/keyring storage. |
| `npm run check:docs` / `build:docs` / `test:docs:browser` | Documentation translations/types, production output/links and browser behavior. |

## Regression expectations

Ingestion changes should exercise meaningful input/output behavior, old summaries and
consumed cursors, duplicate reads, conflicts, rollback, attribution and archive/retention
boundaries. Unknown fields stay unknown. Default-zero test data without verified field validity
must not become known zero. A source failure must not silently prevent other valid events.

Use a fresh test database per run and isolate source environment as well as app storage.
Temporary scripts, logs, probes, extracted records and test databases belong under root
`build/<task>/`. Do not reuse a PID-named directory that might reopen an old database.

## Report accurately

Record cwd, command, environment/toolchain, exact code revision or working-tree status,
exit code, time, first failure, retry result and scope. Keep static, synthetic, real-sample,
mock-browser, native, installation, CI and production results distinct. A successful
documentation build proves no application or provider behavior.

Use the [validation template](/reference/evidence/template/) for new records.
[Plan.md](https://github.com/owent/llm-usage/blob/main/Plan.md) owns active work; the
[current acceptance index](/reference/evidence/current-acceptance/) links detailed records.
Finish with `git diff --check` and inspect untracked files as well as the tracked diff.
