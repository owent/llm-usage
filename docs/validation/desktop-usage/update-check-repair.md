# Software update check repairs

Validated on Windows 11 x64 with Node 24.21.0, Rust 1.98, Tauri CLI 2.12.0 and
WebView2. Base revision: `cfaf3b64fbea044f39579b6c43ec7f862b745b1a`, version 0.3.0.
These are two independent repairs; neither requires changing the release tag.

## Development version checks

The reported launch command was `npm run dev:desktop`. Package detection verifies NSIS
registry identity or a portable package marker against the running executable. A development
executable has neither. The old manual check fetched release metadata and then required a
package identity before comparing versions, producing `update_package_identity_unknown`.
Automatic checks were also skipped for unidentified executables.

Version discovery now runs independently. Development and unidentified executables can
report a newer stable version without selecting an asset. Automatic checks follow saved
settings; automatic and direct downloads still require verified identity. The ten locales
include a development-mode explanation. The browser regression verifies a newer version
with automatic download enabled, no error and no download/install action.

An isolated copy of the original 0.3.0 executable reproduced the reported error through
real WebView2 IPC. A fixed embedded debug build completed the same public check with
`package_kind=development`, `phase=up_to_date`, no error and no asset. Direct downloading
was refused. Independent application data and source environments were used; the user's
running application and database were preserved.

| Check | Command | Result |
| --- | --- | --- |
| Before repair | node desktop/tests/native-update-check.mjs | Exit 1; reproduced update_package_identity_unknown |
| Identity unit regressions | cargo test --manifest-path desktop/src-tauri/Cargo.toml --bin LLMUsage updates::tests --locked | Exit 0; six tests before the fallback change |
| Embedded debug build | node node_modules/@tauri-apps/cli/tauri.js build --debug --no-bundle (desktop working directory) | Exit 0 |
| Fixed development IPC | node desktop/tests/native-update-check.mjs --exe desktop/src-tauri/target/debug/LLMUsage.exe --development | Exit 0; two checks before the fallback change |
| Browser interactions | npm run test:browser | Exit 0; development availability, no error/actions/automatic download and existing update interactions |

The first fixed native probe overlapped the new startup check and correctly received
`update_operation_running`. The probe now waits for that operation before requesting a
manual check. Its first browser assertion ran before the status poll; advancing the controlled
clock through the poll fixed the probe. Neither change relaxed the product's operation guard.
The reviewed screenshot is `build/browser-smoke/update-development-check.png`, with simulated
version 0.3.0 over 0.2.2; it is separate from public release results.

## Documentation metadata fallback

The reported identity failure was not caused by GitHub rate limiting. Separately, GitHub
metadata request failures now try the exact HTTPS documentation endpoint
`https://llm-usage.atframe.work/updates/latest.json` once. A successful API response with
invalid versions/assets does not use fallback to bypass validation. Cancellation is checked
between requests, metadata is bounded to 1 MiB, and both failures retain their error codes.

The schema-1 snapshot contains repository, UTC generation time and minimal stable-release
fields. Clients reject age over seven days or future time over five minutes. Both sources
use the same exact package name, platform, architecture, uploaded state, size and SHA-256
checks. The site references GitHub downloads; it does not mirror packages.

Documentation CI refreshes metadata on main builds and daily at 04:17 UTC. Release publication,
editing, unpublication and deletion dispatch main, preserving the existing main-only Pages
environment and source revision checks. Only the GitHub read receives the CI token. Fresh
site/committed snapshots may be reused without renewing their timestamps; otherwise the
build fails and existing publication remains unchanged. Offline local builds use the committed
snapshot, while desktop clients still enforce expiry.

| Check | Command | Result |
| --- | --- | --- |
| Native fallback regressions | cargo test --manifest-path desktop/src-tauri/Cargo.toml --bin LLMUsage updates::tests --locked | Exit 0; 11 tests, including both identity tests; request failures, cancellation, freshness, package checks and source restrictions |
| Publisher/transport/CI regressions | node --test docs/site/tests/update-feed.test.mjs | Exit 0; eight tests, including token isolation, bounded responses and release dispatch to main |
| Real public metadata extraction | node docs/site/scripts/update-feed.mjs --output docs/site/public/updates/latest.json | Exit 0; stable v0.2.2, seven packages with API sizes/digests; generated 2026-10-09T02:07:18.849Z |

The public latest release returned v0.2.2 during this check. Local tag v0.3.0 does not establish
a public Release. The runtime test uses the real generated snapshot to check all six portable
platform/architecture selections; the publisher checks all seven packages, including NSIS.
No release payload body, uploader, credential, usage or local path enters the snapshot.

## Final local verification

| Check | Command | Result |
| --- | --- | --- |
| Unified product checks | npm run verify | Exit 0; Rust 1,053 passed, eight platform tests ignored; scripts 13, frontend 22; clean formatting, clippy, assets and Svelte types; frontend built |
| Windows release | npm run build:desktop | Exit 0; executable and NSIS 0.3.0 package built |
| Final unpackaged release IPC | npm run test:update:check | Exit 0; two checks, up_to_date, unknown package type, no error/asset; direct download refused |
| Final embedded development IPC | node desktop/tests/native-update-check.mjs --exe desktop/src-tauri/target/debug/LLMUsage.exe --development | Exit 0; two checks, up_to_date, development package type, no error/asset |
| Headless executable | npm run test:headless | Exit 0; 11 checks |
| Portable native update | npm run test:update:windows | Exit 0; three groups: public check/settings, synthetic in-place replacement preserving files/SQLite, interrupted replacement recovery |
| Documentation types/content | npm run check:docs | Exit 0; 203 repository pairs, 21 guide pairs; 35 Astro files with zero errors/warnings/hints |
| Documentation units | npm run test:docs | Exit 0; 39 tests |
| Static documentation build | npm run build:docs | Exit 0; 1,377 pages and 2,885 files, including validated update metadata |
| Documentation browser | npm run test:docs:browser | Exit 0; 31 checks, including the loopback JSON endpoint |

The final development build was rebuilt after both fixes. npm rejected forwarded custom flags
with EUNKNOWNCONFIG before launching the probe; the documented direct Node command succeeded.
The first failure log is retained. Final verification uses isolated owned executables and data.

The new public endpoint, remote Release-triggered refresh and a real newer public upgrade
need publication and separate remote observation. Local fixtures and the loopback static
endpoint do not establish deployed fallback acceptance. Actual NSIS execution and Linux/macOS
desktop update acceptance retain the [original limits](application-updates.md).
The direct public endpoint probe on 2026-10-09 returned HTTP 404; the new snapshot has not
been deployed. Markdown lint checked 459 files with zero issues, exit 0.

## References and artifacts

Rules: [software updates](../../design/desktop-usage/application-updates.md).
Primary sources checked on 2026-10-09:

- [GitHub REST rate limits](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api).
- [Latest Release](https://docs.github.com/en/rest/releases/releases#get-the-latest-release).
- [Release workflow events](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#release).
- [Workflow triggering](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/trigger-a-workflow)
  and [dispatch permissions](https://docs.github.com/en/rest/actions/workflows#create-a-workflow-dispatch-event).

Logs, independent databases and reports stay under ignored `build/update-check/`; browser
artifacts stay under root `build/browser-smoke/` and `build/documentation-site/`.
