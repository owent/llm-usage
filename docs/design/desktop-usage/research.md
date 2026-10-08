# Research references and validation entry points

<a id="调研依据与验证入口"></a>

This file retains design sources/applicable versions. Workspace D:/workspace/projs/github/owent/llm-usage.
Base references checked 2026-09-24, extended formats2026-09-29, scheduling/native references 2026-10-03;
individual rows record later updates. Prototype reviewed statically without running collect.py
or verifying runtime behavior. Prefer official bodies/fixed maintainer source/local prototype;
search summaries locate references only. Rolling docs do not identify installed versions;
source fields alone do not verify complete runtime paths. Current product implementation and native
acceptance follows the [adapter matrix](adapters.md). Windows 11 x64 first release, three-platform CI.
Local real-data extraction authorized; IDEs with unverified local formats remain F1. Local sources
only; enterprise APIs/account reports explain exclusions. Hermes means Nous Research's product.

<a id="prototype"></a>

<a id="原型的静态核对"></a>

## Static prototype review

[previous-draft](../../../previous-draft/README.md): entry/storage/registry/seven collectors/
dashboard/index.html inspected; no business dependency declarations/test requirements found.

| Location | Observed behavior | New design |
| --- | --- | --- |
| collect.py | Loads seven collectors, materializes full lists before import, exports day/week/month/today-hour | Bounded batches/backend UI refresh; retain dimensions without making Python the desktop runtime |
| dashboard/index.html | Refresh fetches data.json only; errors/mismatches display SAMPLE | Real collection; retain old results/status on failure without substituting demos |
| store.py/dashboard | input+cache_read total excludes cache_write | Normalize three exclusive inputs per adapter; distinguish unknown/zero denominator |
| store.py | Globally unique request_id/INSERT OR IGNORE; daily no provider | Stable identity namespaces/corrections/provider; rows do not all imply calls |
| collectors/__init__.py | iter_new_lines state_set precedes parse; truncation by size only | Atomic cursor/context/events/aggregates; detect same-size replacement/rename/generations |
| collect.py/Store | Separate event/aggregate commits; shared connection can later commit failed-source state | Per-source atomic replay; never persist failed-batch cursor |
| kilo_code.py/copilot.py | Sequential copies of running DB/WAL/SHM | Read-only short transactions/consistent backup; consecutive file copies are not consistent snapshots |
| copilot.py | Outside-window condition only pass, traverses all rows; request_id includes DB path/changing values | Separate source identity/revision; deduplicate DB copies; old cursor does not establish effective incrementality |
| codex.py | Regex arbitrary model-bearing nonusage lines; backfills first model if earlier context absent; missing response IDs collide | Structured model context only; unknown ownership stays unknown; verified fallback IDs |
| kimi_code.py/kimi_work.py | New wire candidates but versions from comments; Work fixed machine D-drive path | Official paths still need schema samples; no machine paths as defaults |
| zcode.py | Missing requestId/traceId becomes zcode:None | Handle absent stable IDs without merging every record |
| oh_my_pi.py | assistant usage plus title-generator success logs | Check overlap; compaction/branch/independent usage coverage |
| store.py | Fixed 366days/machine timezone/date cutoff | Configurable explicit included days/fixed timezone; coordinated cleanup/replay/sealed totals |

These describe static paths/failure conditions, rather than reproduced production errors;
the prototype is not directly repaired. SHA-256 identifies original research inputs, before
later comment-language migration:

| File | SHA-256 |
| --- | --- |
| previous-draft/store.py | 81a6bb9e3a7f7943b40438ed642823ab3969f073e68164ce7eb26133cdc7d6d4 |
| previous-draft/collect.py | aa8a406329afd21b4e49ba230fe150ffeeaf173247409d9f70992a8e256cf945 |
| previous-draft/collectors/__init__.py | c4412b8fef0b7220e93c82901414e3833a681c57ed11480311f15d81cb2c0daa |

<a id="技术选型依据"></a>

## Technology-selection references

| ID | Verified reference | Facts and limits used |
| --- | --- | --- |
| T01 | [Tauri WebView](https://v2.tauri.app/reference/webview-versions/), [Windows installer](https://v2.tauri.app/distribute/windows-installer/) | OS WebViews differ; Windows runtime distribution affects size. Official examples do not measure this app's package. |
| T02 | [SQLite WAL](https://sqlite.org/wal.html), [Online backup](https://sqlite.org/backup.html) | WAL concurrency/read-only conditions/checkpoints/consistent backups; verify WAL-reset fixes by SQLite version. |
| T03 | [Electron process model](https://www.electronjs.org/docs/latest/tutorial/process-model) | Chromium process/main-renderer boundaries; do not cite unmeasured memory differences. |
| T04 | [DuckDB concurrency](https://duckdb.org/docs/lts/connect/concurrency) | Many small transactions are not DuckDB's primary goal; this app needs incremental writes/corrections first. |
| T05 | [ECharts import](https://echarts.apache.org/handbook/en/basics/import/) | On-demand chart/component/renderer imports; this project's trimming effect was unmeasured at research time. |
| T06 | [Tauri GitHub CI](https://v2.tauri.app/distribute/pipelines/github/), [Prerequisites](https://v2.tauri.app/start/prerequisites/) | Build separately per OS; Linux WebKitGTK/system dependencies and macOS/Windows toolchains differ. Do not copy automatic-publication examples blindly. |
| T07 | [GitHub runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners) | Versioned runner/architecture candidates windows-2022/ubuntu-22.04/macos-15; M0 rechecks availability. Runner builds do not verify target desktops. |
| T08 | [Microsoft WSLg](https://learn.microsoft.com/en-us/windows/wsl/tutorials/gui-apps) | Linux GUI requires WSL2; WSLg is not a complete Linux desktop. Prior environments/builds are recorded; each newer revision needs separate acceptance. |
| T09 | [Task Scheduler](https://learn.microsoft.com/en-us/windows/win32/taskschd/task-scheduler-start-page), [Task identity](https://learn.microsoft.com/en-us/windows/win32/taskschd/security-contexts-for-running-tasks), [Missed start times](https://learn.microsoft.com/en-us/windows/win32/taskschd/tasksettings-startwhenavailable) | Bodies rechecked 2026-10-03. COM uses current interactive user/LUA/minute repetition/no wake. Ordinary-user registration/deletion tested; actual OS launch checked separately. |
| T10 | [Tauri WebDriver](https://v2.tauri.app/develop/tests/webdriver/) | Current documentation includes embedded cross-platform services; direct tauri-driver retains Windows/Linux/macOS distinctions. Test plugins must not enter releases. |
| T11 | [WebView2 debugging](https://learn.microsoft.com/en-us/microsoft-edge/webview2/how-to/debug-visual-studio-code), [Playwright WebView2](https://github.com/microsoft/playwright/blob/main/docs/src/webview2.md) | Docs/source rechecked 2026-10-03. Environment flags provide CDP; external tests connect to real IPC without app-added debug listeners. |
| T12 | [libuv Windows spawn](https://github.com/libuv/libuv/blob/v1.x/src/win/process.c) | required_vars read2026-10-03 and tested with Node 24.21.0: omitted child USERPROFILE is copied from parent; isolation helpers must prevent that path. |
| T13 | [Tauri CLI 2.12.0](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.0/crates/tauri-cli/src/interface/rust.rs), [Asset code](https://github.com/tauri-apps/tauri/blob/tauri-codegen-v2.7.0/crates/tauri-codegen/src/embedded_assets.rs) | Docs/locked local source cross-checked 2026-10-03. CLI includes custom-protocol; existing asset caches are reused. A built package does not prove embedded assets decompress/render. |
| T14 | [RegGetValueW](https://learn.microsoft.com/en-us/windows/win32/api/winreg/nf-winreg-reggetvaluew), [Delete value](https://learn.microsoft.com/en-us/windows/win32/api/winreg/nf-winreg-regdeletekeyvaluew) | Docs/windows0.62.2 signatures checked 2026-10-03: value type/length/Unicode, distinguish absent values/access errors; native reads require no reg.exe. |

<a id="agents"></a>

<a id="agent-官方与源码索引"></a>

## Official Agent documentation and source index

Shared initial metadata: verified_at=2026-09-24; method official page/fixed source/read-only;
installed_version not probed; owner project maintainer; status research, not runtime acceptance.
Later dated rows state their own scope. Recheck at implementation stage/upgrades/schema changes/
numerical mismatches; no automatic tool upgrades. Original Cursor A45 remains; later VS discovery
uses distinct A49, correcting the former duplicate identifier.

| ID | Official reference / fixed source | Findings and limits |
| --- | --- | --- |
| A01 | [Claude directories](https://code.claude.com/docs/en/claude-directory), [Monitoring](https://code.claude.com/docs/en/monitoring-usage) | Session/sub-Agent transcript paths/token classes/query_source; native Zhipu main-loop calls checked for2.1.197, [sample](../../validation/desktop-usage/claude-container-sample.md); other interfaces separate. |
| A02 | [Codex monitoring](https://learn.chatgpt.com/docs/agent-approvals-security) | Optional OTel/request-response completion/prompt privacy; prototype token_usage_record lacks cross-version guarantees. |
| A03 | [Legacy Cline metrics](https://github.com/cline/cline/blob/dcf8c3c33596e3d561a941202297c564a1cbcd49/apps/vscode/src/shared/getApiMetrics.ts), [Legacy storage](https://github.com/cline/cline/blob/dcf8c3c33596e3d561a941202297c564a1cbcd49/apps/vscode/src/core/storage/disk.ts), [4.1.22 SDK writer](https://github.com/cline/cline/blob/f58bc118bdeef1bd2813cd08e00d98bdcda96475/sdk/packages/core/src/services/session-data.ts), [codec](https://github.com/cline/cline/blob/f58bc118bdeef1bd2813cd08e00d98bdcda96475/sdk/packages/core/src/runtime/config/agent-message-codec.ts) | 2026-10-06 official VSIX/real VS Code GUI-API/three native SDK records: input includes cache, default zero unknown, metrics merge runs/retries as observations; origin.version can change, SDK latest_fallback. Legacy UI documentation-only; CLI/migration/other paths separate; [results](../../validation/desktop-usage/cline-container-sample.md). |
| A04 | [CodeBuddy monitoring](https://www.codebuddy.ai/docs/cli/monitoring), [Official directories](https://www.codebuddy.ai/docs/cli/codebuddy-dir), [aiusage v1.5.8](https://github.com/juliantanx/aiusage/blob/main/CHANGELOG.md) | Third-party-source integration of CLI ~/.codebuddy/projects JSONL. message.usage.input_tokens includes cache read; prefer rawUsage buckets. Optional OTLP-HTTP protobuf overlaps native files without established deduplication; IDE statsSnapshot fields/local samples unverified. |
| A05 | [Copilot CLI reference](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-command-reference) Copilot CLI OTel checked 2026-09-29: COPILOT_OTEL_FILE_EXPORTER_PATH JSON-lines, default OTLP http/json, nested chat/invoke_agent spans/double-count warning; native assistant_usage_events checked. | OTLP JSON/protobuf/file JSONL/chat spans. Checked native assistant_usage_events; do not promise a stable private SQLite schema. |
| A06 | [VS Code monitoring](https://code.visualstudio.com/docs/agents/guides/monitoring-agents) microsoft/vscode extensions/copilot agent_monitoring.md bdc5ebe,2026-09-29: github.copilot.chat.otel.*, exporterNDJSON rather than OTLP, startTime[seconds,nanoseconds], chat gen_ai.usage.*. | chat/invoke_agent represent different levels; optional cache/reasoning/TTFT; only documented Agents covered. |
| A07 | [JetBrains API v2](https://www.jetbrains.com/help/jetbrains-console/analytics-api-v2.html), [Session explorer](https://www.jetbrains.com/help/jetbrains-console/session-explorer.html) | Verifies remote enterprise analytics only, excluded by local-source policy. Local schema remains unverified/F1. |
| A08 | [DSH npm rc.2](https://registry.npmjs.org/@deepseek-ai%2fdsh/0.2.0-rc.2), [Legacy token-meter documentation](https://github.com/deepseek-ai/deepseek-harness/blob/46a7f68b0922371ce7144b668b90e377d8e799f4/packages/llm/token-meter/README.md) | 2026-10-06 installed locked modules/real v4 JSONL-zstd: settlement/retry/seed/pi-ai input reversal/computed totals/title gap. Old doc1 does not verify rc.2; fields/package hashes in [three-source acceptance](../../validation/desktop-usage/m3-container-samples.md). |
| A09 | [OpenClaw store](https://docs.openclaw.ai/reference/session-management-compaction/store), [token use](https://docs.openclaw.ai/reference/token-use), [npm provenance](https://registry.npmjs.org/-/npm/v1/attestations/openclaw@2026.9.8), [Declared-commit schema](https://github.com/openclaw/openclaw/blob/aa6008ad198ef99c43f9d89dbd01694708712974/src/state/openclaw-agent-schema.sql), [CLI-reported commit](https://github.com/openclaw/openclaw/tree/fc23bc864e4553c2d215e479eeec47b67a0bf943) | 2026-10-06 npm 2026.9.8 provenance declares aa6008ad; CLI reports fc23bc8. Installed normalization lacks declared-commit contextUsage additions: use installed/native rules. Public declarations/hashes checked, signatures not independently verified. Schema 24 TEXT-zstd hot transcripts implemented/native CLI-resume-API positive buckets agree. schema_meta cannot identify historical row versions; cold archives/other protocols-interfaces need real acceptance. |
| A10 | [Gemini sessions](https://geminicli.com/docs/cli/session-management/), [telemetry](https://geminicli.com/docs/cli/telemetry/) | Local sessions retain tokens; official requests/token categories/latency metrics; concrete mapping requires further checks. |
| A11 | [Kilo core schema](https://github.com/Kilo-Org/kilocode/blob/9e3f350767477864f7504fab3e01bf6c4d2a7644/packages/core/src/session/schema.ts), [Usage-index migration](https://github.com/Kilo-Org/kilocode/blob/9e3f350767477864f7504fab3e01bf6c4d2a7644/packages/core/src/database/migration/20260907102000_kilocode_model_usage_index.ts) | Checked source tree/core paths; prototype old message table is inapplicable. Full usage-index SQL not audited. |
| A12 | [Kimi sessions](https://www.kimi.com/code/docs/en/kimi-code-cli/guides/sessions), [Legacy source types](https://github.com/MoonshotAI/kimi-cli/blob/9ab1286b8fe4e6bcd116949a27ce5e0ac3389c82/src/kimi_cli/wire/types.py) | Official new paths read; fixed Python types do not establish prototype camelCase usage.record. Fields remain unverified. |
| A13 | [Prototype kimi_work.py](../../../previous-draft/collectors/kimi_work.py) | Local clue only, without official newer stable storage rules/real samples. |
| A14 | [MiMo 0.1.15](https://github.com/XiaomiMiMo/MiMo-Code/releases/tag/v0.1.15), [Release getUsage](https://github.com/XiaomiMiMo/MiMo-Code/blob/14dfe68a1c121f859544ba810b3c308e8501bfb2/packages/opencode/src/session/session.ts), [Independent paths](https://github.com/XiaomiMiMo/MiMo-Code/blob/14dfe68a1c121f859544ba810b3c308e8501bfb2/packages/shared/src/global.ts) | 2026-10-06 official ELF/real SQLite-WAL, eight part-API calls; SDK default zeros/positive-total reversal, per-session versions/MIMOCODE_HOME/DB. Shared forks/highest DB version do not verify historical rows. |
| A15 | [oh-my-pi session](https://github.com/can1357/oh-my-pi/blob/62bc57be1b03ef0802a33cf7f5f530e534527531/docs/session.md) | Session paths/entry model/persistence/branches; auxiliary logs require versioned samples. |
| A16 | [pi Usage](https://github.com/badlogic/pi-mono/blob/b45597504eeaba1f11a9920a1d1048c361ed4b8e/packages/ai/src/types.ts), [session manager](https://github.com/badlogic/pi-mono/blob/b45597504eeaba1f11a9920a1d1048c361ed4b8e/packages/coding-agent/src/core/session-manager.ts) | Reasoning is an output subset; standalone usage/compaction/branch summaries carry usage. |
| A17 | [OpenCode SQL](https://github.com/anomalyco/opencode/blob/0027387dc5c59793c12dfc531abc78f825ed6868/packages/core/src/session/sql.ts), [1.18.34 processor](https://github.com/anomalyco/opencode/blob/aec0b9a6d8898f68f923aaf08b7306d931fd9d76/packages/opencode/src/session/processor.ts), [Title generation](https://github.com/anomalyco/opencode/blob/aec0b9a6d8898f68f923aaf08b7306d931fd9d76/packages/opencode/src/session/prompt.ts) | step-finish parts count observed main-loop calls; session totals reconcile only.2026-10-05 official CLI/local-model input/cache-read/repeat checks pass; title API absent from parts/totals. [Container results](../../validation/desktop-usage/container-sources.md) and [later per-record upgrade](../../validation/desktop-usage/source-policy-upgrades.md) distinguish historical/current checks. |
| A18 | [Qwen recording](https://github.com/QwenLM/qwen-code/blob/085e98c00cac2f8dd29eb39c760409bc6da889a9/packages/core/src/services/chatRecordingService.ts), [telemetry](https://qwenlm.github.io/qwen-code-docs/en/developers/development/telemetry/) | usageMetadata/model/session paths; cumulative Goal observations cannot be summed directly; disable prompt logging. |
| A19 | [Zoo 3.86.0](https://github.com/Zoo-Code-Org/Zoo-Code/releases/tag/v3.86.0), [Release message enumeration](https://github.com/Zoo-Code-Org/Zoo-Code/blob/6aa9d0174a9ecae155c6c5db9134bead4b67197d/packages/types/src/message.ts), [Earlier aggregation reference](https://github.com/Zoo-Code-Org/Zoo-Code/blob/f7806475331fcae5f4e8b5558d04415eeb5da88c/packages/core/src/message-utils/consolidateTokenUsage.ts) | 2026-10-06 official VSIX/real extension API/native-UI callbacks/model API: cache-inclusive tokensIn, default-zero unknown, full ask/say enumeration. CLI/condense/deletion separate. |
| A20 | [TRAE session API](https://docs.trae.cn/enterprise_query-usage-details-by-session-id), [Enterprise individual usage](https://docs.trae.cn/enterprise_check-individual-usage) | Remote enterprise interfaces/reports excluded; do not infer local formats, which remain F1. |
| A21 | [ZCode usage](https://zcode.z.ai/cn/docs/usage-stats), [Prototype zcode.py](../../../previous-draft/collectors/zcode.py) | Official docs distinguish local sessions/remote Coding Plan; model-io schema was initially a prototype clue. Later versioned checks remain in adapter/validation records. |
| A22 | [WorkBuddy usage](https://www.workbuddy.cn/docs/workbuddy/Usage), [AgentHUD provider requirements](https://github.com/jazzenchen/agent-hud-open/blob/main/docs/providers.md), [tokmesh-core parser](https://docs.rs/tokmesh-core/latest/src/tokmesh_core/sessions/tencent_buddy.rs.html) | Official pages describe points/plans only. Third-party source verifies .workbuddy/projects JSONL; later local eight-file/420-event comparison passed. Trace/session overlap/version scope remain unverified. |
| A23 | [Zed telemetry](https://zed.dev/docs/telemetry) | Hosted accounting differs from local telemetry; server-side token accounting does not establish complete local fields. |
| A24 | [Hermes site](https://hermes-agent.nousresearch.com/), [Storage documentation](https://hermes-agent.nousresearch.com/docs/developer-guide/session-storage/), [usage](https://github.com/NousResearch/hermes-agent/blob/ef70b3661cbfcf57e583008ad91dd04d8ba46070/hermes_state_usage.py), [schema](https://github.com/NousResearch/hermes-agent/blob/ef70b3661cbfcf57e583008ad91dd04d8ba46070/hermes_state_schema.py), [0.21.5 release](https://github.com/NousResearch/hermes-agent/releases/tag/v2026.9.24), [Fixed normalization source](https://github.com/NousResearch/hermes-agent/blob/f97608f178d1ffeca59860195ab7da295f7c8e5f/agent/usage_pricing.py) | Official 0.21.5 image/real CLI/public resume: uncached 849/cache read 812/output 4. A24/release normalization agree; default zero unknown, reasoning output subset. Whole-DB schema cannot identify row client versions; compatible fallback remains. Gateway/auxiliary/backfill/mixed models/per-call logs not accepted; [Hermes record](../../validation/desktop-usage/hermes-container-sample.md). |
| A25 | [tokscale](https://github.com/junhoyeo/tokscale) Fixed main 1d9a9395418efc6952944b794097935d7d6fa1e8. | Third-party open-source Rust local-session parsers. Read-only clients/scanner/sessions supply path/field clues; no execution. They do not replace official per-product checks/local test samples; estimation/price-comparison logic excluded. |
| A26 | [Goose](https://github.com/aaif-goose/goose) Migrated block/goose=>aaif-goose/goose; official a701bb1 checked 2026-09-29; native CLI 1.53.0 checked 2026-10-06. | usage_ledger per request since migration 15. Real call320/2/322, cache read 0/write/cost NULL; never add session totals or substitute prefilled cumulative zeros for unknowns. Other versions/GUI separate; [M8 samples](../../validation/desktop-usage/m8-container-samples.md). |
| A27 | [Crush](https://github.com/charmbracelet/crush) Fixed main 1f3827bcd2d20f38076b2d46123683271e6ed9ba,2026-09-29. | projects.json {projects:[{path,data_dir,last_accessed}]} via CRUSH_GLOBAL_DATA/XDG/LOCALAPPDATA; per-project &lt;data_dir&gt;/crush.db. sessions.prompt/completion snapshot latest-step context, reset after compaction, not usage. Cumulative cost rolls children into parents: root parent_session_id IS NULL only. No message token columns. |
| A28 | [Amp site](https://ampcode.com/) Closed-source CLI; tokscale 1d9a939 sessions/amp.rs,2026-09-29. | ~/.local/share/amp/threads/T-*.json: messages[].usage model/inputTokens/outputTokens/cacheRead/cacheCreation/credits and usageLedger.events timestamp/model/credits/tokens. Compare both to avoid double-counting. |
| A29 | [Official Roo 3.54.0 release](https://github.com/RooCodeInc/Roo-Code/releases/tag/v3.54.0), [Fixed commit](https://github.com/RooCodeInc/Roo-Code/tree/27001b2b5aa47b65e8a6ba1914e0f4216be0ebb0), [Archive announcement](https://roocodeinc.github.io/Roo-Code/) 2026-10-06 VSIX hash/real extension-host-API; earlier structure b867ec9. | globalStorage/tasks/ui_messages.json: four Task buckets initialize zero; OpenAI omits nested cache details; missing price cost defaults zero, all unknown. Cancellation three API calls/two native records separate from one-call request-limit comparison. Public API needs no acceptance-injected key; [old-summary/package checks](../../validation/desktop-usage/m8-container-samples.md). Does not verify CLI/other versions/forks. |
| A30 | [Aider](https://github.com/Aider-AI/aider) Fixed main 5dc9490bb35f9729ef2c95d00a19ccd30c26339c,2026-09-29; native 0.86.2,2026-10-06. | Opt-in local --analytics-log. Real ask message_send 95/3/98, locally estimated cost/cache-reasoning unknown. No default historical backfill; other cases separate; [M8](../../validation/desktop-usage/m8-container-samples.md). |
| A31 | [Continue](https://github.com/continuedev/continue) Official 5522c6f44ca0ac3528b37b44818fbfa39b5af470,2026-09-29; native CLI 1.5.47,2026-10-06. | Isolated CONTINUE_GLOBAL_DIR sessions usage accumulates by session; real input/output 1,471/2. Prefilled cache zero does not establish API-reported zero. Unknown calls/model/interval start do not invent day attribution; hub account data excluded; [M8](../../validation/desktop-usage/m8-container-samples.md). |
| A32 | [Droid](https://factory.ai/) Factory.ai CLI. | ~/.factory/sessions/{uuid}.settings.json cumulative tokenUsage input/output/cacheRead/cacheCreation/thinking plus same-name JSONL transcripts; no cost. Cumulative allocation is interval estimation only. |
| A33 | [Amazon Q Developer CLI](https://github.com/aws/amazon-q-developer-cli) Official 15cc8f3,2026-09-29. | Third-party timestamped JSON history path ~/.aws/amazonq/history/. Open source permits inspection; history usage fields unverified, SSO relogin may lose history. |
| A34 | [News report](https://www.penligent.ai/) Grok Build,xAI closed-source CLI; news supports product/version. | ~/.grok/sessions/&lt;workspace&gt;/&lt;session&gt;/{updates.jsonl,signals.json,summary.json,events.jsonl} and ~/.grok/logs/unified.jsonl. Explicit five-bucket usage usable; cumulative totalTokens increments/compaction-difference compensation are inference and excluded. |
| A35 | [Google Antigravity](https://antigravity.google/) | ~/.gemini/antigravity[-cli]/conversations/&lt;uuid&gt;.db gen_metadata protobuf turn usage: fixed-plus-new input/cacheRead/output/thinking/responseId. Reverse-engineered layout; timestamp changed1.1.18. IDE usage via language server needs separate checks. |
| A36 | [Junie custom models](https://junie.jetbrains.com/docs/custom-llm-models.html), [Official release 3419.29](https://github.com/JetBrains/junie/releases/tag/3419.29) Actual 26.9.22,distribution hash/bytecode/seven calls checked 2026-10-06; earlier tokscale 1d9a939. | JUNIE_HOME/sessions or .junie modelUsage.inputTokens is uncached input; default missing-field zeros unknown. Positive cost client Estimated; zero cost/duration unknown. Missing API/provider/product version cannot derive complete input-total usage; written calls count despite failed tasks. [Old-summary/cursor/transaction/package results](../../validation/desktop-usage/m8-container-samples.md); IDE remainsF1. |
| A37 | [Kiro](https://kiro.dev/) AWS;tokscale reference1d9a939 sessions/kiro.rs,2026-09-29. | Three formats: CLI ~/.kiro/sessions/cli/*.json(+jsonl), kiro-cli ~/.local/share/kiro-cli/data.sqlite3 conversations_v2, IDE globalStorage kiro.kiroagent .chat/execution/promptLogs. Auto agent often0; inferred usage excluded. Metering credits remain separate. |
| A38 | [Zed thread storage](https://github.com/zed-industries/zed) Official bd74733/local read-only schema,2026-09-29. | Historical bd74733 paths ~/.local/share/zed, ~/Library/Application Support/Zed, %LOCALAPPDATA%/Zed; threads/threads.db data JSON-zstd with request_token_usage input/output/cache_read/cache_creation and cumulative. Then checked hosted-only/imported exclusion; later Zed 1.22.0/DbThread 0.3.0 specified external provider is separately verified, [native results](../../validation/desktop-usage/plan-20261007.md). Do not treat old hosted-only scope as current universal behavior. |
| A39 | [Codebuff](https://codebuff.com/) Former Manicode; official caec5fc,2026-09-29. | ~/.config/manicode*/projects/*/chats/&lt;chatId&gt;/chat-messages.json, CODEBUFF_DATA_DIR override; usage fields require samples. |
| A40 | [Command Code](https://commandcode.ai/), [Repository](https://github.com/CommandCodeAI/command-code) Repository lacks product source; npm command-code@1.69.0 dist/cli.mjs read line-by-line,2026-09-29. | ~/.commandcode/projects/&lt;slug&gt;/*.jsonl v3 tree session/message/model_change; assistant inputTokens/outputTokens/cacheRead/cacheWrite/costUsd. Exclude rewind orphan branches; deduplicate fork copies by ID/time. |
| A41 | [jcode](https://jcode.sh/) Official 1jehuang/jcode 4f6bf8e,2026-09-29; native 0.91.0,2026-10-06. | Snapshots/journal token_usage input/output/optional cache, no independent reasoning/cost. Real custom endpoint460/2 matches API-CLI-snapshot; unknown components remain unknown. Environment snapshot versions do not identify every message; [M8](../../validation/desktop-usage/m8-container-samples.md). |
| A42 | [gajae-code](https://github.com/Yeachan-Heo/gajae-code) Official 7e54f9c,2026-09-29; native 0.18.7,2026-10-06. | Real sessionv5 API-CLI-native 412/2/414. Fixed source distinguishes configuration chains from usage, OpenAI-completions zero fallback/message initialization timestamps. Cache/uncached unknown; time request start. Other API/version scope separate; [M8](../../validation/desktop-usage/m8-container-samples.md). |
| A43 | [Fixed Xum commit](https://github.com/coder/xum/tree/81b0b744db6e27a4416f3596d70bf88529171caf), [Official CLI](https://xum.coder.com/reference/cli), [provider](https://xum.coder.com/config/providers) Official npm 0.30.0/integrity/fixed commit/two calls checked 2026-10-06; former coder/mux. | sessionUsageService v1 cumulative byModel; displayUsage uncached input/output excluding reasoning/default zeros. Native XUM_ROOT/MUX_ROOT/RUN_SESSION_ROOT retention paths. Default custom streaming provider omits usage requests; retain real five-zero and usage-only gateway comparisons separately. [Parser/complete-old-summary/package results](../../validation/desktop-usage/m8-container-samples.md). |
| A44 | [Warp](https://www.warp.dev/) | Only local account/workspace usage caches requestsUsed/spendCents/syncedAt found, without token details; quota data excluded from token statistics. |
| A45 | [Cursor CLI documentation](https://cursor.com/docs/cli/overview), [CursorDump transcript analysis](https://github.com/lpalbou/CursorDump) | CLI ~/.cursor/projects/&lt;slug&gt;/agent-transcripts/&lt;uuid&gt;/*.jsonl has conversations without per-call token/model fields. Per-call usage only remote dashboard get-filtered-usage-events, excluded; IDE state.vscdb unverified. |
| A46 | [iFlow CLI](https://github.com/iflow-ai/iflow-cli) Alibaba iFlow,closed-source; official 4642808/discontinued2026-04-17,checked 2026-09-29. | Official ~/.iflow settings.json/tmp/&lt;project_hash&gt;; /chat save JSON. OTel api_response five buckets resemble Gemini; actual chats paths/usageMetadata need local samples. Service discontinued2026-04-17; no implementation. |
| A47 | [Qoder](https://qoder.com/) Alibaba/former Tongyi Lingma,brand change 2026-05; docs.qoder.com/npm @qoder-ai/qodercli1.1.64 unpacked2026-09-29. | CLI device flow ~/.qoder/; Electron IDE %APPDATA%/com.qoder.app.stable*. CLI session/usage fields unverified; local checks required. |
| A48 | [AtomCode](https://atomcode.atomgit.com/docs/en/index.html) Official e4215f733eeba4cede553e28f9b559e6b3dc34ef/45e05cb14775f070f3867539e1f21c9849b3484b;npm 5.2.1,2026-10-06. | Real .meta v1/CLI-API one turn input 6,176/output 2. Model turn_stats accumulates separately from round_count; never add last total_tokens. Default zero unknown; exclude completely matched UIv1/rewindv2 only. Binary buildID unknown; sampled version cannot identify other sessions; [M8](../../validation/desktop-usage/m8-container-samples.md). |
| A49 | [Visual Studio discovery API](https://learn.microsoft.com/en-us/visualstudio/extensibility/locating-visual-studio?view=visualstudio), [vswhere](https://github.com/microsoft/vswhere), [SKU inheritance](https://learn.microsoft.com/en-us/visualstudio/extensibility/vsix-extension-schema-2-0-reference?view=visualstudio), [Copilot installation](https://learn.microsoft.com/en-us/visualstudio/ide/visual-studio-github-copilot-install-and-states?view=vs-2022) Official bodies/local Setup-PE/official VS 2022 VSIX checked 2026-10-07. | No Community/year directory guessing. VS 18 writer uses Path.GetTempPath; check TMP/TEMP. Inspected 17.14.1713.63837 has no such JSONL exporter; other versions retain limits. [Catalog integrity/per-package/two-version results](../../validation/desktop-usage/m9-vs-copilot-discovery.md). |

Fixed commits rechecked via public GitHub commits API; files read from raw.githubusercontent.com,
no upstream code execution. Rolling pages were not matched to local releases; integrations need
version inventories/samples. CodeBuddy browser fetch 502 was followed by read-only PowerShell
official HTML-body checks, also used for WorkBuddy/ZCode; failed pages supplied no facts.
Hermes fixed commit came from commits API with targeted storage/schema/usage reads. Later
agent/usage_pricing.py fetch 429 stopped that read; outer PowerShell exit 0 did not count as file
retrieval. Field names did not establish cache containment; M3 version samples checked it later.

<a id="m8-格式依据核验于-2026-09-29"></a>

## M8 format references checked 2026-09-29

Per-product format research follows extended coverage. A25–A48 identify original references;
full field descriptions live in adapter headers/[M8 results](../../validation/desktop-usage/m8-second-batch.md).

- Official fixed source: archived Roo b867ec9, Goose a701bb1,Crush 1f3827b,jcode 4f6bf8e,gajae 7e54f9c,
  Continue 5522c6f,AtomCode e4215f7,Zed bd74733,Aider 5dc9490,Amazon Q15cc8f3,Codebuff caec5fc,iFlow 4642808.
- Official distributions: Command Code npm 1.69.0 dist/cli.mjs without repository product source;
  Qoder1.1.64 obfuscated bundle verifies paths only.
- Closed-source third-party parser references: tokscale 1d9a939 Amp/Grok/Junie/Kiro/Droid/Xum/
  Antigravity read line-by-line.
- Key findings: Goose usage_ledger absent from tokscale; Crush context snapshots not usage;
  no verified local per-call token records for Amazon Q/Codebuff, excluded; iFlow discontinued
  2026-04-17. Roo/legacy Cline UI tokensIn definitions differ; each uses its own fixed source,
  with later native checks recorded separately.

<a id="研究边界"></a>

## Research limits

Remote billing/organization APIs excluded; real local formats need per-version checks without
inventing absent samples. JetBrains AI Assistant/TRAE and other unverified IDEs remain F1;
Zed/Junie CLI belong to M8. Packages/performance/platform/native results stay separate from targets.

<a id="扩展覆盖的证据层级"></a>

<a id="扩展覆盖的来源类型与核验范围"></a>

## Extended source types and verification scope

A25–A48 identify extensions; [matrix](adapters.md#扩展覆盖)/[M8 execution](execution.md#m8)
maintain capabilities/stages.

- Read-only third-party [tokscale](https://github.com/junhoyeo/tokscale), fixed main
  1d9a9395418efc6952944b794097935d7d6fa1e8, clients.rs/scanner.rs/sessions gives paths/fields/
  lifecycle clues. Estimation/price comparison/aggregation rules are not adopted.
- Product identities checked individually against official sites/repos/multiple reports:
  block/goose=>aaif-goose/goose,coder/mux=>coder/xum,Qoder brand change 2026-05,Grok Build xAI CLI
  reported in news; Command Code/jcode/gajae/Codebuff/AtomCode have official sites/releases.
- Amp/Grok Build/Junie CLI/Kiro/Droid/Xum/iFlow/Qoder/Antigravity protobuf references include
  third-party/reverse-engineered formats. Matrix records product-version capability/native
  acceptance; synthetic regressions do not verify actual local output.
- Exclude Cursor remote get-filtered-usage-events/TRAE tokscale remote usage API/Warp account
  requests-spend caches from token statistics under local-source policy.
- Added Zed A38 supplements telemetry A23 with threads.db local usage; Junie CLI supplements
  JetBrains A07 with local events.jsonl. Both registered in M8; JetBrains AI Assistant IDE/TRAE
  remain F1. Later Zed external-provider acceptance is limited to the separately verified format.
- F1 unverified IDE families, no probes/implementation: Cursor IDE,Windsurf IDE-CLI,JD JoyCode,
  Zhipu CodeGeeX,Baidu Comate,Huawei InsCode/CodeArts Snap. Keep product identity/unverified
  local-format status instead of unsupported claims.

These references authorize no signed-in backends/billing APIs/organization access. Current
requirements govern implementation/validation.

<a id="verification-20260929"></a>
<a id="verification"></a>
<a id="readiness-verification"></a>

<a id="当前验证入口"></a>

## Current validation entry points

Design/plans maintain current requirements; actual commands/versions/environments/results/
remaining gaps are in [current acceptance](../../validation/desktop-usage/current-acceptance.md).
Specific formats: [validation directory](../../validation/desktop-usage/); Git retains older
process history. Document checks cannot replace product/real-source/native-platform acceptance.
