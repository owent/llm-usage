# Version 0.3.1 dependency and release checks

## Scope and sources

On 2026-10-09 the user authorized upgrading all dependencies as far as compatible,
version 0.3.1, the v0.3.1 tag and a push to trigger release. The user also authorized
including pnpm configuration and synchronizing its lock. The update-check repair is
a separate prerequisite commit, `28952cb9`; its original evidence remains in
[the repair record](update-check-repair.md).

Every direct npm dependency was checked against the official npm registry's latest
tag; every direct Cargo dependency was checked against crates.io's latest stable
version. Actions were checked against their upstream release tags and resolved
commit SHAs. Lockfiles select compatible transitive versions.

| Component | Selected version |
| --- | --- |
| Astro / Starlight | 7.3.8 / 0.42.6 |
| Svelte / Vite / Playwright | 5.57.2 / 8.3.4 / 1.64.0 |
| Tauri API and crate / CLI / build crate | 2.12.2 / 2.12.1 / 2.7.1 |
| zstd / fs4 / SHA-2 / Unix getrandom | 0.14.0 / 1.1.0 / 0.11.0 / 0.4.3 |
| TypeScript | 6.0.3; compiler API compatibility exception |
| pnpm / local lock-generation npm | 12.10.1 / 12.2.0 |
| CI and repository Rust toolchain | 1.99.0; desktop MSRV 1.90 |

TypeScript 7.0.2 is the registry's latest version, but Astro and Svelte checkers declare
support for TypeScript 5 or 6 and consume its compiler API. TypeScript 7 has no such
API; see [the official announcement](https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/).
Both manifests and all locks retain 6.0.3 without forcing peer compatibility.

Cargo retains four older indirect packages required by upstream constraints.
`crypto-common` pins `generic-array` to 0.14.7. GTK's `proc-macro-crate` pins
`toml_datetime` to 0.6.3 and `toml_edit` to 0.20.2, keeping `toml` at 0.8.2.
An isolated attempt to resolve `toml` 0.8.23 failed on that exact datetime constraint;
the product lock was not changed by the probe.

## Compatibility repairs and package managers

Rust 1.99 exposed a needless borrow and deprecated a test counter's atomic update.
The borrow was removed and the counter now uses a compare-exchange loop supported
by the declared desktop MSRV. SHA-2 0.11 returns an array without `LowerHex`;
explicit padded lowercase byte formatting preserves file, tree and cache digests.
A fixed-vector test checks empty and nonempty file hashes and rejection of a wrong digest.

The root and desktop npm locks were regenerated from manifests using npm 12.2.0
and the official registry. The first clean install rejected a preexisting Tencent
mirror URL under npm 12's download policy; fresh locks contain only official npm
registry URLs. No forced peer install or global policy relaxation was used.

The pnpm workspace includes the root and desktop. Its lock records both importers
and the pinned package manager. An isolated frozen install executed approved esbuild
scripts and skipped the nested npm postinstall. CI also checks a frozen pnpm install,
frontend types/build and documentation types after the npm frontend checks.

The official Tauri CLI 2.12.0 and 2.12.1 NSIS templates are byte-identical:
32,061 bytes, SHA-256 `dabed59013b1d78b879a1a85bc7f2eed2993b33a9a90cdabe5946de3d3950597`.
Existing installation/rollback customizations remain applicable.

## Local verification

Environment: Windows 11 x64, Node 24.21.0, Rust 1.99.0, npm 12.2.0 for clean
restores and pnpm 12.10.1. Task logs and independent synthetic databases are under
root `build/`; personal Agent data was not used in this dependency round.

| Command or check | Result |
| --- | --- |
| Root and desktop npm clean restores | Both pass with official-registry locks |
| pnpm frozen install, strict peers, frontend check/build | Pass; both importers match every manifest dependency |
| `npm run verify` | Pass: 1,054 Rust tests, 8 explicitly ignored platform tests, 13 script tests, 22 frontend tests, no type warnings |
| `npm run test:browser` | Pass: update/settings/cost layout, ten locales and existing interactions |
| `npm run build:desktop` | Pass: 0.3.1 Windows x64 executable and NSIS installer |
| `npm run test:headless` | 11 pass, isolated executable and SQLite |
| `npm run test:update:windows` | Pass: three native synthetic portable replacement/recovery groups |
| `npm run test:update:check` | Two pass: real public metadata check from an unknown package and blocked downloads without an identity |
| Embedded debug build and development IPC | Two pass: `package_kind=development`, real public check without error or asset selection; direct download refused |
| `npm run test:receiver` | Eight pass, no owned credentials remain |
| `npm run test:desktop` | 19 native WebView2/IPC checks pass |
| Windows portable packaging | Pass: extracted headless check, zstd integrity, size and SHA-256 report |
| Documentation checks/build/browser | Pass: clean Astro types, 39 unit tests, 1,379 pages/2,889 files, 31 browser checks |

The source update snapshot was refreshed from the real public v0.3.0 release on
2026-10-09; the draft v0.3.1 is not a public stable update. Remote results below are
separate from these local checks. The existing tag workflow
creates a draft release after all platform jobs pass; it does not establish signing,
notarization, a new NSIS installation lifecycle or other-platform update GUI acceptance.

## Remote release and deployment

Source: `55b440e379a020e991c501f74b84d6b7d2ef0706`, annotated tag `v0.3.1`, pushed
atomically with main on 2026-10-09. The tag was not moved after validation.

- [Tag CI 37879955917](https://github.com/owent/llm-usage/actions/runs/37879955917):
  all 12 jobs passed, including the release asset checks. Six native archives passed
  extracted executable checks; both Linux archives also passed GTK/WebKit GUI checks.
- [Main CI 37879956064](https://github.com/owent/llm-usage/actions/runs/37879956064):
  all 11 applicable jobs passed; the tag-only release job was skipped. Both Windows
  Rust runs passed 1,054 tests with eight explicit ignores across 105 test binaries.
- Release ID `407495650` is a draft with 13 assets: six portable archives, the Windows
  x64 NSIS installer and six platform reports. All were downloaded and matched API
  size/SHA-256 values. Six zstd streams/TAR layouts and the NSIS compressed archive
  passed integrity checks. Every report matches the exact tag revision and its packages.
- The downloaded Windows x64 portable package passed 19 native WebView2/IPC checks,
  with 20 query samples and a local P95 of 912.61 ms. This result concerns the CI-built
  executable; the local build's native checks are recorded separately above.
- [Documentation 37879956018](https://github.com/owent/llm-usage/actions/runs/37879956018)
  built and published successfully. The public publication marker matched the source
  revision. `/updates/latest.json` changed from HTTP 404 to 200; its fresh v0.3.0
  snapshot matched all seven public package names, sizes and digests.

The first draft asset probe used GitHub's published-release tag endpoint and received
404. Paginated release enumeration located the exact draft ID and the final checks
succeeded. No release or tag was recreated. Draft notes now describe the update-check
fix, dependency upgrades, pnpm workspace and verified assets.

The acceptance-record follow-up changes only documentation and the plan. Its documentation
workflow validates and publishes the updated records independently of the tag's product CI.
Public Release-triggered metadata refresh and a newer public-version update remain
unobserved while v0.3.1 is a draft.
