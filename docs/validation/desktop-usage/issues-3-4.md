# Qoder detection and settings invalidation repairs

2026-10-23, version 0.3.4 on base `d9c776a6c0706618e044b0f03d1532e641ce8605`.
Both [issue 3](https://github.com/owent/llm-usage/issues/3) and
[issue 4](https://github.com/owent/llm-usage/issues/4) have valid root causes.

## Findings and changes

Issue 3's Qoder marker scan ran before the unchanged-file path and inside the writer
callback. Manual routing and parser windows repeated it. Deep routing probes now run before
storage callbacks; lightweight metadata work retains its owner-first order. Format detection
suspends file preparation until it can run outside the
writer. Ownership changes still check the current database under the writer. The cache
holds at most 4,096 paths, matching length, modification/creation time and sampled head/tail
fingerprints. A second fingerprint check prevents storing an unstable result. Completed
positive and negative results are shared across routing and parser windows; cancellation,
timeouts and I/O errors do not become cached negative evidence. Eviction/restart may repeat
bounded detection. Existing history, diagnostics and old-owner recovery rules are retained.

Issue 4 reproduced extra summary queries after unrelated autosaves. Saving now updates only
changed reactive fields and retains unchanged nested objects. Only timezone, week-start or
language changes increment the statistics reload key. Alias, source-root, theme and update
changes retain trend/today selections; language changes still refresh statistics and reset
both selections. Pricing invalidation/recomputation remains separate.

## Regression evidence

Windows 11 x64, Node 24.22.0, Rust 1.99.0; locked dependencies. Temporary scripts, synthetic
sources, independent databases and logs are under `build/issues-3-4/`.

| Check | Result and scope |
| --- | --- |
| Qoder probe unit regressions | Three passed: unchanged negative detection runs once; same-size/same-mtime tail replacement and truncation invalidate the result; cancellation does not cache a negative result; manual routing and bounded parser windows share one deep probe outside the writer |
| Qoder transcript and source-routing integration tests | 26 passed, including full-registry discovery, long initial messages, malformed/partial records, old checkpoints, history-protected ownership, diagnostics and rollback |
| Focused browser regression | Passed with mocked IPC: alias/source roots/update/theme changes issue no statistics or cost queries and preserve trend/today selections; language changes still refresh and clear selections |
| Cache mutation | Disabling cache hits makes the unchanged-file regression fail with repeated deep probes |
| Writer mutation | Moving manual evidence inspection into the storage callback makes the writer assertion fail |
| Settings mutation | Restoring the committed App component makes the alias-save query-count assertion fail |
| `npm run verify` | Exit 0: 1,103 Rust tests passed/nine ignored, 32 frontend and 20 script tests passed; Markdown, assets, types, fmt, Clippy and frontend build passed |
| `npm run test:browser` | Exit 0: the complete browser suite includes the added settings invalidation/selection checks and existing dashboard, update, telemetry, locale and layout checks |
| `npm run check:docs` | Exit 0: 208 repository pairs, 21 guide pairs, reviewed source comments and no Astro diagnostics |
| `npm run test:docs`, `npm run build:docs` | Exit 0: 39 tests and 2,935 published files with checked links, search, language roots and update metadata |

Every mutation restored the exact repaired source afterward. The tests count actual probe
invocations and IPC calls rather than enforcing machine-dependent elapsed-time limits.
The first sandboxed routing run hit Windows rename permissions; the normal-process rerun
with a separate temporary directory passed. The added record changed the sidebar inventory
from 227 to 228; the test now requires the new route and exact count. An overlapping
intermediate verification hit Windows executable-sharing error 32 before a test could start;
that run is not counted as successful verification. This task did not install or query a native Qoder client;
these results do not certify real-client token formats, actual large-history timings,
Linux/macOS runtime performance or a packaged GUI lifecycle. No issue comments, commits,
pushes or releases were created.
