# Cross-platform credentials and continued implementation

<a id="跨平台凭据与计划继续执行"></a>

Date 2026-10-05; application 0.2.1; Windows repository D:/workspace/projs/github/owent/llm-usage.
Initially clean tree; tests ran against uncommitted changes based on
0c5d63c19151e7d6f72a33c513f7ac9ddc0e2ad5, without commit/push/publication. References:
[receiver authentication](../../design/desktop-usage/receiver-auth.md),
[platform checks](../../design/desktop-usage/platform-ci.md), [Plan](../../../Plan.md).
This record preserves that stage. Windows missing readback after writing, bounded confirmation,
200 parallel rounds after cross-process handling confirmation, and the earlier unresolved revoke
anomaly are in [source-rule upgrades](source-policy-upgrades.md), 2026-10-06. Later final packages/
receiver checks are in [M8 native samples](m8-container-samples.md); hashes below identify these
historical packages, rather than the latest artifacts.

<a id="实现与核验依据"></a>

<a id="实现与核验结果"></a>

## Implementation and checked references

Linux previously rejected all system credentials. It now uses secret-service 5.2.0 with
a DH session, the current user's default collection and search across all collections.
Reject locked collections, duplicate items, a sole temporary session item, and a default alias
pointing to the session collection. See [Secret Service aliases](https://specifications.freedesktop.org/secret-service/latest/aliases.html)
for mutable aliases. No collection creation/unlocking; background reads do not prompt;
operations limited to three seconds. Synchronous calls run bounded asynchronous work on a
separate thread, avoiding nested Tauri-runtime panic. Errors return fixed codes only.
If a saved write receives a failed reply or fails readback, cleanup removes only items matching
the complete contents written by this operation; externally replaced items are retained.

macOS uses security-framework 3.7.0/security-framework-sys 2.17.0. All queries/writes/deletes
specify nonsynchronizing Keychain items and authentication UI Fail. One native symbol declaration
supplies the Apple Fail constant omitted by the library. aarch64-apple-darwin type checks pass;
no native run at this stage. Both platforms use getrandom 0.3.4; Windows native implementation retained.

Implementation followed inspection of published library metadata/downloaded source:
[Secret Service specification](https://specifications.freedesktop.org/secret-service/latest/),
[secret-service 5.2.0](https://docs.rs/secret-service/5.2.0/secret_service/),
[Keychain PasswordOptions](https://docs.rs/security-framework/3.7.0/security_framework/passwords/struct.PasswordOptions.html),
[Apple UI Fail](https://developer.apple.com/documentation/security/ksecuseauthenticationuifail).
These references do not verify actual agent exporters.

Rust CI expanded from Linux-only to Windows/macOS/Linux, keeping fail-fast=false. Native
Linux checks use isolated D-Bus, disposable encrypted keyring and random test passwords.
Passwords/bindings pass through stdin only. Service checks inspect name ownership before
contacting it, avoiding another daemon. Actual Linux artifact reporting incorrectly listed
Debian control.tar.gz/data.tar.gz as release artifacts. Reports/uploads now include only
installers, macOS .app.tar.gz and checksum reports; regression and real bundle-directory checks pass.

<a id="环境与结果"></a>

## Environment and results

Windows 11 x64, Node 24.21.0, Rust 1.98.0, Edge/WebView2. WSL Debian 13.7 x86_64,
kernel 6.18.40.1, Node 24.21.0, Rust 1.98.1, WebKitGTK 2.54.0, gnome-keyring 48.0.
User-authorized sudo apt installed missing system dependencies; rustup added Clippy/rustfmt.
Windows/WSL do not share npm/target/live databases. Isolated Linux copy includes uncommitted
source; revision/difference inventory under root build/platform-auth/.

| Command/check | Exit/result | Scope |
| --- | --- | --- |
| Windows npm run verify | 0; Rust 904, core 808/app 96; frontend 21/scripts 4; clean types | Markdown/assets/fmt/Clippy/SQLite/format/integration/build; 8 environment tests ignored by default |
| Windows npm run build:desktop | 0; release/NSIS produced | Not installed |
| Windows npm run test:headless | 0; 11 cases, 3 events/75 tokens | Isolated synthetic sources, actual executable/SQLite |
| Windows npm run test:desktop -- --runs 1 | 0; 17 checks, 3 first-screen samples, P95 850.1 ms | Actual WebView2/IPC/tray/pause/minute tasks; not full resource/assistive-technology acceptance |
| Windows npm run test:receiver | 0; 8 checks, zero owned credentials left | Synthetic exporter installation lists, actual IPC/HTTP/credential store |
| Windows explicit native credential tests | 0; one each for cross-process read/revoke and actual HTTP revocation/other-source continuation | Random owned credentials only; no actual IDE configuration changes |
| Windows npm run test:browser | 0 | Edge simulated IPC, ten languages/themes/selection/telemetry batch setup |
| Debian cargo clippy --workspace --all-targets --locked -- -D warnings | 0 | Final platform implementation and test targets |
| Debian cargo test --locked plus final app-specific rerun | 0; core 808/app 95; app ignores 9 by default | Full core formats/SQLite/adapters, Linux timeout/nested runtime; explicit native checks separate |
| Debian npm run test:credentials:linux | 0; 6 cases, five native tests plus missing-service case also testing temporary default collections | No service/listener, cross-process persistent read/revoke, actual HTTP/source isolation, duplicate/temporary/default-alias rejection, locked store; disposable keyring/daemon cleaned |
| Debian npm run build:desktop -- --bundles deb,appimage | 0; release/deb/AppImage produced | Standard system dependencies, no old sysroot/WebKit path patches; not installed |
| Debian ELF/extracted AppImage headless checks | Both 0; each 11 cases, 3 events/75 tokens | Unmodified extracted AppImage through AppRun; not FUSE/WSLg/native GUI acceptance |
| macOS module cross cargo check/clippy, aarch64-apple-darwin --locked | Both 0, Clippy -D warnings | Temporary minimal Windows project directly imports actual module; type checks, no native linking/execution |
| Artifact-report regressions/real Linux directory | 0; four cases; report contains only deb/AppImage | Debian staging files excluded as release packages |
| Final documents/diff | 0; 182 Markdown files, 97 local links/anchors in nine affected documents, git diff --check | All temporary files under root build/ |

Windows full output and Linux per-check logs are in build/platform-auth/; temporary Linux
databases are under its isolated copy's root build/. Initial PowerShell startup/child-process
restrictions were retried through allowed cmd/execution paths and are not product failures.

One parallel native Windows rerun returned credential_store_unavailable when creating the
second credential. Individual and --test-threads=1 reruns exited 0. First failure retained
as windows-native-final.log. A diagnostic copy of the actual module only expanded read/write
error codes; two threads over 40 create/revoke operations did not reproduce it, zero owned
items left. Failure timestamps identified the unique synthetic test directory. Complete
binding inspection found one owned credential for that directory still present; it was
precisely removed, preserving other applications/tests. See windows-vault-concurrency.log.
The failure path left a credential; its cause remained unknown, and cleanup reliability
after store failure still needed investigation. Nonreproduction does not establish a fix.

Two Debian rerun failures came from existing test setup: source_intervals reused a PID-named
database with disabled sources; process_guard reused one with an active primary key.
Read-only inspection found five disabled sources and five existing runs respectively.
Both now create unique directories each run, reject reuse and close/clean after success.
Original state is retained in pid-fixture-evidence.log. App's 95 tests then passed.

<a id="制品"></a>

## Artifacts

| Artifact | Bytes | SHA-256 |
| --- | --- | --- |
| Windows executable | 9,901,568 | 09de6cfa17d37e9195e910db3647ea58cab2417fe059f352b4d9d62835eae1a8 |
| Windows NSIS | 3,913,043 | 993e255bff157f8e980180503bde5053378f3411f37333674ea587ba226919d8 |
| Linux deb | 5,440,738 | 3895d451211bf69beb87e570168431dc460831e1d157c701199a4891cd8b1a9f |
| Linux AppImage | 111,122,936 | 9caebe7f4507d78190a504f0ce5a19645e006c99135a87d48ebb38fbea3ccea3 |

Windows bundle under desktop/src-tauri/target/release/bundle/nsis/; Linux packages copied
to root build/platform-auth/artifacts/. deb Depends/architecture checked; no install/upgrade/
uninstall here. Current Windows executable/NSIS copied separately into artifacts/windows/;
reports include only this stage's packages, not older bundle files marked with its revision.
Reports record baseline SHA and uncommitted state.

<a id="真实样本与剩余条件"></a>

## Native samples and remaining checks

After reviewing existing local_availability, Windows/Debian ran read-only actual registry
discover/detect, probing at most three files per root and printing only counts/version metadata.
Windows Gemini/Qwen/Junie and other missing-sample sources were absent; Zed database threads
had zero rows, insufficient to verify usage format. Debian default discovery found no usable
sources. No agents/model requests were launched to fabricate data. Existing Codex/Pi/OMP/Kilo/
ZCode/Kimi/Copilot results were not counted again as newly accepted sources.

No remote CI or installation acceptance was triggered at this record's date. Later Windows
installation, Debian Podman GTK/WebKit/FUSE/Orca and additional native sources are in
[latest acceptance](current-acceptance.md); macOS native Keychain/HTTP's two explicit tests
are in [batch CI](ci-plan-validation.md). macOS desktop/specific hardware requirements were
removed for this round. Other source versions, telemetry, DPI/assistive technology, host
logout/login, outbound audit and F1/detail Merge/usage-cost alerts are in [Plan](../../../Plan.md).
