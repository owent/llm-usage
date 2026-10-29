# Windows Agent extraction

<a id="windows-agent-extraction"></a>

Windows 11 x64, Node 24.22.1 and Rust/Cargo 1.99.1; application source version 0.3.7.
The user authorized installation and minimal requests to the specified Zhipu/Kimi Coding
endpoints. No commit, publication or existing statistics-database reset was performed.
macOS execution and the user's original macOS diagnostic files were not available.

## Installation and requests

Official npm `latest` resolved to Claude Code 2.1.312 and DSH 0.2.1-rc.2. Installed both
under ignored `build/agent-native-20261029/tools`; package locks retain npm integrity
references. Both native version commands pass. Installation scripts were reviewed and
only Claude's native-binary placement and DSH's spawn-helper permission script were run;
unrelated blocked lifecycle scripts were not broadly approved. Registry integrity checking
does not establish independent signature verification. DSH's `next`/`alpha` tags were not selected.

Credentials were read with yq inside local launch scripts and passed only through process
environments. No key was stored in ordinary configuration, prompts, arguments or logs.
Claude used the suppliers' documented Anthropic endpoints; DSH used the provided
OpenAI-compatible URLs. Tests ran in isolated workspaces/configuration/data directories.
Claude tools were disabled for the three simple prompts; a separate permitted Write
sample produced a two-call tool loop. DSH title generation and telemetry were disabled;
its samples therefore do not verify default auxiliary-call coverage.

| Native sample | Calls | Uncached input | Cache read | Output |
| --- | ---: | ---: | ---: | ---: |
| Claude / glm-5.3-flash | 1 | 1,710 | Unknown | 55 |
| Claude / glm-5.3 | 1 | 1,707 | Unknown | 14 |
| Claude / kimi-for-coding | 1 | 1,257 | 512 | 36 |
| Claude / glm-5.3-flash, tool loop | 2 | 2,845 | 2,432 | 244 |
| DSH / glm-5.3-flash | 1 | 11,190 | Unknown | 69 |
| DSH / glm-5.3 | 1 | 11,187 | Unknown | 34 |
| DSH / kimi-for-coding | 1 | 10,760 | Unknown | 60 |

All seven CLI invocations exited 0. Claude CLI usage and deduplicated native assistant
usage agree; ten content-block records represent five calls. Default zero cache write
and complete Claude totals remain unknown. Native transcripts do not establish billing
channels or actual cost. DSH's three settlements contain 33,137 uncached input and 163
output tokens; reconstructed totals are 33,300, without treating them as source totals.
No independent HTTP response capture was added in this round.

## Parser changes and checks

The old Claude parser reproduced the fault on an unchanged native file: it stopped at
atis-latch, recorded undocumented_record_type and imported zero calls. Native 2.1.312
writers and samples distinguish request-shape/blob/request, atis, tools-used and cost-state
metadata from assistant usage. Their validated shapes can be skipped without adding
usage. File-history-snapshot and progress have distribution-writer references and synthetic
shape tests; this round's real tool sample did not produce those two types.

Unknown types, malformed shapes and metadata carrying usage still reject the file.
Detection accepts a valid metadata first record. A separate metadata-policy marker forces
consumed unchanged files to be checked once; bounded continuation, incomplete final lines, transaction rollback,
diagnostic history, original observation times and real conflicts remain covered. Existing
per-record version/default-zero rules remain unchanged; tested new records retain their
internal compatibility references. Native 2.1.312 events use known_version without
latest_fallback warnings, and malformed metadata diagnostics identify the actual field.

DSH still writes v4. Installed pi-ai 1.4.0 independently confirms uncached input,
reasoning-inclusive output, omitted/default-zero cache and computed totals. Its installed
event catalog adds job/registered, job/settled and working-directory/change, whose source
definitions contain execution/directory metadata. Synthetic tests accept complete declared
shapes, including optional fields, without inventing calls. Malformed or usage-carrying
metadata and unknown required events reject without advancing checkpoints. Their actual
job and directory-change execution remains outside the native three-prompt samples.

The rebuilt Windows release executable scanned the original seven local files through
the application registry: eight calls, seven active files and no degraded files. A complete
repeat retained events and data_revision. An independent SQLite backup of the old failed
Claude import recovered its one call, changed the file to active and preserved the original
undocumented_record_type diagnostic. Source-file digests were unchanged throughout.
Headless executable/SQLite results do not establish application GUI or macOS acceptance.

| Check | Command or entry | Result |
| --- | --- | --- |
| Product checks | `npm run verify`; final Cargo Clippy/tests | Exit 0; 1,160 Rust tests pass, 9 platform ignores; lint, script/UI tests, types, Clippy and frontend build pass |
| Windows release binary | `node node_modules/@tauri-apps/cli/tauri.js build --no-bundle`, cwd `desktop` | Exit 0; final parser source rebuilt; no installer or publication |
| Native extraction/old DB | Task `native-readback.mjs` | Exit 0; eight calls/seven active files, identical repeat, old failure recovered, source digests unchanged |
| Zed native extraction/old DB | Task `acceptance.mjs` | Exit 0; one interval/164 output/two turn buckets; fresh import, repeat and old skip recovery; source database/WAL/logical rows unchanged |
| Headless regressions | `npm run test:headless` | Exit 0; 11 checks, independent synthetic sources/database |
| Documentation | `npm run check:docs`, `npm run test:docs`, `npm run build:docs` | Exit 0; paired content/types, 41 tests and 2,961 published-output files checked locally |
| Redaction/Git | Task public/private credential checks; `git diff --check` | Exit 0; no actual-key matches in changed public files or local test artifacts |

First failures are retained in task logs. A regression helper initially passed ScanLimits
by reference; the corrected test passes. Redaction initially omitted DSH message.id,
correctly triggering ownership rejection; the sample now retains its hashed identity.
The synthetic unknown-type test now uses an actually unknown name instead of the newly
recognized file-history-snapshot. The documentation inventory expectation was updated
for the added validation page. Sandbox runs encountered Windows CredReadW access denied,
Pagefind directory access denied and a headless temporary-directory EPERM; corresponding
normal-user checks pass. The release command was corrected to use the desktop working
directory and direct Tauri CLI after npm argument forwarding/root-cwd failures.

## Zed native threads

The user generated two turns with custom glm-5.3 and built-in DeepSeek V4.1 Flash in
installed Zed 1.26.2, commit c4273271092b0287f7d181f616bd5171e0a2f70c. Read-only access
to the default threads.db found one zstd DbThread 0.3.0: cumulative input_tokens=47,748,
output_tokens=164, two request buckets and no cache fields. Its latest model is
deepseek/deepseek-flash. Messages retain no per-call model or usage. The redacted fixture
keeps counters, relationships and schema while removing content, tools and original IDs.
The installed version and user-generated sample establish this run's provenance; the
serialized schema does not identify every historical record's client version.

Fixed-source checks distinguish the mappers: OpenAI chat subtracts cache from prompt_tokens;
built-in DeepSeek keeps inclusive prompt_tokens and writes zero cache buckets. Thread
counters accumulate across provider/model changes, and DbThread.model is mutable. Thus
the native input cannot establish either uncached or inclusive input. Keep positive output/
cache buckets; default zeros and complete totals remain unknown. Compare request and cumulative
output only; do not sum ambiguous input/cache buckets or assign usage to the latest model.
The two buckets count observed turns, without establishing exact underlying requests.

The old release executable reproduced the provider-filter failure: an active source file
with non_zed_dev_thread_skipped and no aggregate. The fix reads complete native 0.3.0
structures independently of custom provider names, including llm-usage-zhipu. Complete
native structures take precedence over the old statistics-only provider fallback. Existing
input/quality disagreements retain ordinary aggregate conflicts and the stored summary;
they are not parser-reference corrections. Historical hosted rules remain separate. Other providers have structural compatibility
reads, without real-call certification. An independent policy marker replays consumed old
cursors, preserves checkpoints through transaction failures and marks completion only after
a valid full scan. Invalid rows retain diagnostics and do not hide other valid threads.

The Windows executable reads the original source into one native session interval:
164 reported output tokens and two observed turn buckets, with input/cache/totals unknown.
An independent old failed-import database recovers the same aggregate and retains the old
skip diagnostic; repeat scans retain data_revision. Native intervals remain separate from
daily/model call statistics, so this sample does not add per-call events or daily totals.
Regressions cover real discovery/routing, old consumed cursors, checkpoint and late
aggregate rollback, actual capped continuation, invalid rows, positive cache, unknown zeros,
ambiguous-total/output overflow, output-only reconciliation and existing aggregate conflicts.
They also cover switching back to legacy custom or hosted provider names. The 1.22.0
statistics-only regression passes.

First test failures exposed an unrouted manual-root helper, an incorrect daily-summary
expectation, and eager checkpoint deletion after a parser-version bump. The tests now use
application routing and native interval queries; the independent policy marker preserves
old checkpoints. Earlier isolated GUI attempts failed known-directory lookup and API-key
forwarding; the user then supplied these successful native calls. This round uses read-only
executable/SQLite validation. GUI rendering and the reported macOS problem remain unverified.
A translation-manifest write returned Windows UNKNOWN; an unchanged retry succeeded.

Sources: [Claude npm](https://www.npmjs.com/package/@anthropic-ai/claude-code),
[DSH npm](https://www.npmjs.com/package/@deepseek-ai/dsh),
[Zhipu Claude configuration](https://docs.bigmodel.cn/cn/coding-plan/tool/claude),
[Kimi Claude configuration](https://www.kimi.com/code/docs/en/third-party-tools/claude-code.html),
[fixed Zed database writer](https://github.com/zed-industries/zed/blob/c4273271092b0287f7d181f616bd5171e0a2f70c/crates/agent/src/db.rs),
[fixed Zed usage mapper](https://github.com/zed-industries/zed/blob/c4273271092b0287f7d181f616bd5171e0a2f70c/crates/language_model_core/src/chat_completion.rs),
[fixed DeepSeek mapper](https://github.com/zed-industries/zed/blob/c4273271092b0287f7d181f616bd5171e0a2f70c/crates/language_models/src/provider/deepseek.rs),
[fixed thread accumulation](https://github.com/zed-industries/zed/blob/c4273271092b0287f7d181f616bd5171e0a2f70c/crates/agent/src/thread.rs).
