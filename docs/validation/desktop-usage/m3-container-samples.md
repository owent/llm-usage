# MiMo, Zoo and DSH native-source and artifact acceptance

<a id="mimozoo-与-dsh-真实来源及成品验收"></a>

2026-10-06, version 0.2.1; [implementation prerequisites](../../design/desktop-usage/implementation-readiness.md)
and [three-source field rules](../../design/desktop-usage/m3-runtime-samples.md). Official clients
called a real local model in isolated rootless Podman. Native files, model API and application
SQLite were compared independently. No personal accounts/paid cloud requests were used. Runtime
had no external network/host mounts; ordinary UID 1000, zero effective capabilities, default
seccomp. Downloads/image preparation were separate from offline runs; installation/empty
sessions do not verify usage.

<a id="固定分发物与实际源码"></a>

## Fixed distributions and actual source

| Product | Official reference | Actual distribution |
| --- | --- | --- |
| Zoo Code 3.86.0 | [Release](https://github.com/Zoo-Code-Org/Zoo-Code/releases/tag/v3.86.0)，commit `6aa9d0174a9ecae155c6c5db9134bead4b67197d` | VSIX 34,635,449 bytes，SHA256 `25c338c866bf7dacde840d8039093de867c2b0f70070d68d50031a04fe3b5f73` |
| MiMo Code 0.1.15 | [Release](https://github.com/XiaomiMiMo/MiMo-Code/releases/tag/v0.1.15)，commit `14dfe68a1c121f859544ba810b3c308e8501bfb2` | tar 46,617,071 bytes，SHA256 `3530927a2d69eb0f809c11f663ecd6939b82c0b598ff0fbe1f82f431d764ca0c`；ELF 134,998,144 bytes，SHA256 `872728c1547b9910bb8d98b5d2fe9b1a461649b6a3e147232188f020f0ddc2e8` |
| DSH 0.2.0-rc.2 | [Official npm manifest](https://registry.npmjs.org/@deepseek-ai%2fdsh/0.2.0-rc.2), installed lockfile and compiled modules | SHA1 `dfc8f7e09cfa96b854d6f0cf3a973ce7f2948925`; SHA512 integrity below; current GitHub commits do not verify rc.2 |

DSH integrity:
sha512-EAJ3gPNcVt/uv8X19PMm9NkVhWgT7xXNMk0UKCVm+IQ5rpSQOcsMUa0HWlnYYVybKMsccjcRB21vVVsaXQ6IdA==.
Checked installed session-format, JSONL persistence, LLM codec, token-meter and pi-ai 0.87.1
individually; current GitHub code does not establish npm package behavior. Zoo checks covered
the full message enumeration/provider defaults; MiMo checks covered that release's session
getUsage, Global/DB paths and SDK 6 normalization.

The official Qwen3.5-0.8B GGUF BF16 model is 1,557,662,496 bytes, SHA-256
9a7bed4041b7975e0f71fa34670d1e9025213bc92905ac0db75d36c4fa3fa623. Local llama-server used
context 64,000; a transparent proxy forwarded actual SSE/usage unchanged without constructing
responses/tokens. This sample-generation environment does not establish hardware performance.

<a id="真实样本与覆盖"></a>

## Native samples and coverage

| Source | Native/API comparison | Limits |
| --- | --- | --- |
| Zoo | One primary call: input 6,118/output 53/derived total 6,171; five native UI messages, public extension usage callback and API matched | Actual VS Code 1.140.0 GUI/public API; cancelled after receiving usage, task completion unverified. Default-zero cache/cost unknown; uncached input/model/provider/source total unknown. CLI/deletion/compaction need separate checks |
| MiMo | Initial/resumed session: eight step-finish records/eight API calls; input 25,588/output 1,024/total 26,612; positive uncached input 4,855/cache read 20,733 | Both CLI runs exited 0; all eight finishes were length, so task success is unverified. Read part only, without adding message usage; zero cache write/reasoning/cost unknown; each owning session's version remains latest_fallback |
| DSH | Headless/resumed runs completed; two native settlements matched main-loop API: uncached 5,606/cache read 5,568/total input 11,174/output 105/total 11,279 | Another title API call used 205 tokens but no native result was saved, so none is invented. Stream usage is a duplicate; native computed total is not source_total. Other protocols and native seed/retry/fail scenarios unverified |

Original roots are under the independent WSL workspace's build/plan-final-push/:
Zoo zoo-real-1791290947, MiMo mimo-real-1791291189, DSH dsh-real-1791291190.
Zoo UI: 986 bytes, SHA-256 402e3148ae2242dfa0840193071fc922c78bf34316c6324c779420dc95bd5fec.
MiMo main DB: 401,408 bytes, SHA-256
0a60be4932cb9b3cd543479a713885ca9ad9045fc16991f488b26681299641cd;
WAL: 1,516,192 bytes, SHA-256 ca0343b73d1c1e281ed06fdd321ca046aad640bd8e4cce6c58b43478f53726df.
The last four MiMo records were still in WAL; the main DB alone does not verify all eight.
DSH zstd: 17,575 bytes, SHA-256 bfe960a2d99eb2726c08d839a261dbe3ce645801e10b69af205e07159d20fee7;
decompressed: 51,114 bytes, one v4 header/27 consecutive-seq events.

Test data under core/tests/fixtures: zoo/real-3.86.0, mimo-code/real-0.1.15 and dsh/real-0.2.0-rc.2.
Only per-type allowlisted fields remain, with anonymized identities; bodies/requests/configuration/
private paths/credentials/original DBs are not committed. DSH redacted records preserve event
types/order/time/usage and reduce other payloads; they do not claim complete upstream restoration.
New/prior sample audit: 68 files/171 JSON objects, zero credential keys/private paths.

<a id="修正与专项回归"></a>

## Corrections and targeted regressions

Zoo supports actual ask/say enumerations, rejecting the whole file for unknown enumerations;
default-zero corrections match complete old event summaries. MiMo uses product-specific
normalization/environment paths and protects version ownership/quality/model/real conflicts.
DSH adds bounded independent v4 JSONL/zstd reading under token-meter final settlement/retry/
inheritance boundaries. Future generation versions are rejected without falling back to old
generations; partial writes/limits retain cursors. Default/manual-root/physical-file deduplication
was checked through the full registry. Complete native headers may restore old incorrect
ownership only without statistical history; owned files/checkpoints transfer transactionally,
while other invalid files/diagnostics remain. Kimi Work's observed absolute root applies only
to the current process home; other-user test contexts do not probe it. Native tests must still
isolate the actual child process's USERPROFILE/HOME/source environment.

New Zoo five/MiMo five/DSH v4 nine requirement tests passed, alongside legacy Zoo five/MiMo
five/DSH four and OpenCode/Roo/Kimi Work/routing regressions. Cases cover complete canonical/
legacy old summaries, consumed cursors/processing positions, repeat reads, concurrency,
transaction rollback, same-batch conflicts, protected fields, missing/invalid buckets,
inheritance/retries, snapshot decreases, zstd window/decompression limits and unknown events.
Constructed boundary cases are explicitly synthetic regressions.

<a id="本批成品验收"></a>

## Artifact acceptance for this batch

Commands/exit codes/failure logs remain under build/plan-final-push, including historical first failures.

- Windows npm run verify exited 0: 1,007 Rust tests, core 906/app 101, eight ignored by default;
  21 frontend/four script tests; Markdown 201 files at that stage. Svelte had no errors/warnings;
  fmt, Clippy -D warnings, assets/frontend build passed. Later documentation changes were checked separately.
- Debian: 1,004 Rust tests, core 906/app 98, nine ignored by default; native Clippy and release/
  deb/AppImage builds exited 0.
- Windows NSIS 12/headless 11/WebView2-IPC 17 checks with 20 startups/receiver eight passed.
  Initial-page P95 751.36 ms; no owned credentials/installation integrations remained.
- Linux root 1791295818331: nine groups/47 checks covering deb install/upgrade/rollback/
  uninstall/reinstall/purge and FUSE/GTK/Orca, exit 0. Actual focus/speech were checked across
  five pages/ten languages. Only the owned container received SYS_ADMIN; application-user
  capabilities remained zero with default seccomp. Actual FUSE mounts/release on exit were
  checked. Host login/logout, complete assistive technology, language pronunciation and
  physical audio remain unverified.
- Final Markdown 202 files; 44 affected Markdown files/262 local references; fmt, Skill
  quick_validate and git diff --check exited 0. Skill checks explicitly used Python UTF-8.
  Read-only cleanup found no Podman containers; an old owned, never-started Roo reread
  container was removed.

| Artifact | Bytes | SHA-256 |
| --- | --- | --- |
| Windows exe | 10,106,880 | `faec1e22d20756c41a301bc5727e68ed9b7325bd53d76ff3a183ecab2169b56c` |
| Windows NSIS | 3,985,629 | `cba1db1a17432b3ab56625f3d55d7b5b692b303c685ee8139e2a7e9b1cbaa7ab` |
| Linux deb | 5,527,382 | `4afb1bd7958983de49a121966055a297f5ee8e50432e70b3c6511db71bd22854` |
| Linux AppImage | 111,204,856 | `c08b20ad16569ce1225360426ef515b6fc7c1bbfb5ee78f1e95ffbb01501bc99` |

Linux m3-readback-1791295919 and Windows m3-windows-1791296331720 created old databases with
the previous OpenClaw-stage package, then upgraded and repeatedly read unchanged native sources.
Old package: Zoo zero/MiMo eight/DSH zero; new: one/eight/two. Three physical files/checkpoints,
zero conflicts, health ok. Old event identity/revision/first observation and two prior audit
records remained. Repeated scans added/changed no committed events. Main DB/WAL/JSON/zstd
hashes remained unchanged; SHM bookkeeping was excluded independently. Windows verifies
application reading of the Linux client files, without verifying official Windows clients.

The same package reread 15 prior sources: Aider, AtomCode, Continue, Crush, gajae-code, Goose,
Hermes, jcode, Junie, OpenCode, Qwen, Roo, Xum, Cline and OpenClaw. All exited 0; summary logs:
m3-prior-readbacks-1791296018. Values, unknown components, old-database upgrades and existing
coverage gaps remained unchanged.

<a id="首次失败与纠正"></a>

## First failures and corrections

- DSH development failures included duplicate entry/file probe APIs, TokenQuality lacking Eq,
  wrong SQL table names/expected counts. Logs remain; fixes use actual interfaces. Full checks
  found Option::is_none_or newer than MSRV 1.77.2; compatible code passed without raising MSRV.
- Full-registry discovery initially let Pi claim a manual DSH type=session file. Recovery now
  requires a complete native fingerprint and no statistical history; protected-history/other-file tests passed.
- Initial Linux reread copied MiMo main DB without WAL, so only four records were readable;
  SQLite's new empty WAL also caused the hash assertion to fail. Copying the complete source
  produced eight records matching API. A separate temporary-script assertion used dsh instead
  of deepseek-harness; that failure remains recorded.
- Initial Windows scripts broadcast manual roots, producing legacy Cline/other-product
  misidentification diagnostics; invalid TMP caused SQLite commit failure. Independent
  single-variable checks showed exit 1 with absent TMP and exit 0/eight committed records
  after creating it. Scripts now use verified product-specific isolated source paths and valid
  temporary directories. A later omitted repository isolation helper let libuv add USERPROFILE
  and scan other default sources. That result was invalidated, its extracts not committed;
  corrected startup isolation discovered only the three test sources.
- The fourth native GUI startup chose port 6669. Node fetch rejected it before connecting,
  producing a CDP timeout. Local fetch reproduced bad port, and the [Fetch standard](https://fetch.spec.whatwg.org/#port-blocking)
  blocks it. The shared HTTP-port probe now completes a real local 204 request through the
  same fetch, retries rejected ports within bounds and closes its owned server before CDP use.
  Twenty probes, seven script syntax checks and 20 actual GUI startups passed. Later passes
  retain the first-failure explanation.

No macOS desktop/specific-hardware acceptance occurred. Other clients/native mixed versions/
protocols/host and remote-CI conditions remain in [Plan.md](../../../Plan.md); this batch does
not complete every milestone. On 2026-10-06, a read-only check confirmed [baseline CI 37330787284](https://github.com/owent/llm-usage/actions/runs/37330787284)
had all eight jobs successful for HEAD c2c8f6f on 2026-10-05, with three-platform artifacts.
This batch's uncommitted changes are absent from that revision. Official repository references
were checked for all five current Action tags/branches. The user then authorized an independent
test branch, commits/pushes and three-platform CI; results are in [this batch's CI record](ci-plan-validation.md).
