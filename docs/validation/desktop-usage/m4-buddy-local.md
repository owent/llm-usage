# Local WorkBuddy / CodeBuddy Code sessions, 2026-09-29

<a id="workbuddy--codebuddy-code-本地会话接入2026-09-29"></a>

<a id="核验结果与边界"></a>

<a id="证据与边界"></a>

## Checked results and limits

- [Official CodeBuddy directories](https://www.codebuddy.ai/docs/cli/codebuddy-dir) establishes
  ~/.codebuddy/projects/ session JSONL. [Environment variables](https://www.codebuddy.ai/docs/cli/env-vars)
  provides CODEBUDDY_CONFIG_DIR override for configuration/data root. [Monitoring](https://www.codebuddy.ai/docs/cli/monitoring)
  also provides OTLP exports, off by default.
- [AgentHUD provider rules/source index](https://github.com/jazzenchen/agent-hud-open/blob/main/docs/providers.md)
  reads CodeBuddy .codebuddy/projects and WorkBuddy .workbuddy/projects assistant message/
  function_call usage locally, without remote quota reads. [tokmesh-core parser/synthetic tests](https://docs.rs/tokmesh-core/latest/src/tokmesh_core/sessions/tencent_buddy.rs.html)
  supplies JSONL field/identity samples. [aiusage v1.5.8 changes](https://github.com/juliantanx/aiusage/blob/main/CHANGELOG.md)
  says CodeBuddy message.usage.input_tokens includes cache read, with providerData.rawUsage
  cache hit/miss; applying Claude-style addition would double count.
- These latter sources are third-party code/tests, not official product format specifications.
  Initial inspection found no WorkBuddy session directory; later inspection found native
  .workbuddy/projects. .codebuddy/projects remains absent. Added native read-only WorkBuddy
  checks; CodeBuddy CLI still has document-based implementation/synthetic checks only.

<a id="实施"></a>

## Implementation

- Register independent codebuddy/workbuddy sources with bounded .jsonl discovery in explicit
  directories. CodeBuddy accepts CODEBUDDY_CONFIG_DIR; manual roots must be projects within
  the corresponding product directory, avoiding ownership competition between similar parsers.
- Read completed assistant message/function_call only, requiring valid millisecond timestamps,
  session/message identity. Missing tokens stay unknown. Invalid numbers/cache read greater
  than total input/inconsistent hit-miss-input skipped with diagnostics. Key product:session:
  messageId/traceId/id; line number supplies later-write revision within the same file.
- input_tokens/prompt_tokens are total input including cache. Explicit prompt_cache_miss_tokens
  maps to uncached; subtract only with consistent checked fields. reasoning remains an output
  component. Unverified provider/prices/trace totals are not inferred.
- Derive normalized total when input/output known and within bounds. Preserve source total
  separately and compare; inconsistent records excluded from usage.

<a id="验证与未完成条件"></a>

## Validation and remaining conditions

- cargo test --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core tencent_buddy_wire
  --lib: three synthetic unit tests pass, covering cache deduplication, independent WorkBuddy
  identity, invalid cache fields, default discovery and incremental cursors.
- cargo test --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core --test adapter_layout_v30:
  three pass, including separate product directories/registries.
- Native WorkBuddy last-launch.json reports latest launch 5.6.2, build
  37a65c0b33b8e394904eb49e6c218c4cc0601649; this does not identify every historical record's
  writer version. Read-only field check: eight JSONL files/420 rawUsage records. All 420 satisfy
  prompt=cache_miss+cache_hit, total=prompt+completion and message/rawUsage input-output
  consistency; all message identities unique. Native command: cargo run --manifest-path
  desktop/src-tauri/Cargo.toml -p llm-usage-core --example real_verify_workbuddy -- &lt;home&gt;
  build/workbuddy-review/real-check-20260929. Eight files/420 events/zero diagnostics; repeat
  adds zero, stored count stays 420; input=uncached+cache_read and total=input+output pass.
- Six native ~/.workbuddy/traces/trace_*.json: all trace.totalTokens zero, only two have numeric
  modelInfo; trace.traceId does not match session providerData.traceId. Without verified
  duplicate relationships/coverage, trace values are not added to these 420 individual records.
- Windows 11 x64, Node 22+/Rust lockfile: npm run verify exit 0, Markdown/assets/scripts/UI/
  Svelte/fmt/Clippy/Rust/web build. Default-sandbox Node subprocess EPERM retried with automatic
  approval, original command passed. git diff --check exit 0. Full verification preceded native
  sample review; later targeted Rust/Clippy/Markdown/fmt checks validated further edits.
- Obtain native CodeBuddy CLI samples later. WorkBuddy broader versions/subagent-trace overlap/
  history after deletion-compaction remain unchecked.
- CodeBuddy OTLP/local JSONL may represent the same calls; no cross-source duplicate identity
  or deduplication code yet. Choose one before acceptance. Historical overlap requires source-level
  resolution, without claiming simple addition establishes complete coverage.

<a id="2026-09-30-codebuddy-ide插件codebuddyextension本机核验与实施"></a>

## CodeBuddy IDE/plugin local checks and implementation, 2026-09-30

<a id="扩展存储核验结果与边界"></a>

<a id="扩展存储证据与边界"></a>

### Extension storage checks and limits

- User reported zero collected CodeBuddy usage. Read-only local ~/.codebuddy contains plugin
  marketplace/settings/logs; cli-memwatch-* establishes CLI runtime presence, without projects.
  No local CLI JSONL samples, so original adapter discovered no roots: confirmed cause.
- IDE candidates have no individual token records: APPDATA/CodeBuddy CN codebuddy-sessions.vscdb
  ItemTable zero rows; automations.db two empty tables; globalStorage/state.vscdb settings only;
  genie-history without token fields; workspaceStorage; LOCALAPPDATA/CodeBuddyExtension
  check-point/file-tree/plan-task.
- Actual file: LOCALAPPDATA/CodeBuddyExtension/Data/profile/host/workspace/history/session/
  conversation/index.json requests[]. Each item has id/type/messages/state/startedAt/usage.
  Request-level usage aggregates: inputTokens=cacheTokens+cachedMissTokens,
  totalTokens=inputTokens+outputTokens, plus cachedWriteTokens. lastTokens semantics unverified;
  credit is platform points, without currency interpretation. Parent session index.json contains
  only conversations/current registry. messages/id.json extra is embedded JSON string with
  modelId/modelName/requestId/isHelperMessage. Extracted preparation-permitted fields only,
  without reading/printing message text.
- Native sample: 14 conversation indexes, including empty requests. Only one conversation
  has four type=craft/state=complete requests. One reports input 64339/output 1822/cache read
  53504/miss 10835/total 66161, model kimi-k3-1. Three all-zero requests contain a single user
  message/no model call and are excluded. All four satisfy input=cache+miss and total=input+output.
  Same session hash occurs across multiple profile/workspace trees, with different conversation
  IDs; copies may repeat the same request.

<a id="扩展存储实施"></a>

### Extension implementation

- codebuddy becomes a combined entry for unchanged CLI JSONL tencent_buddy_wire and new
  codebuddy/extension_store.rs, dispatched by path shape with shared agent identity. WorkBuddy
  unchanged, protected by regression.
- Discovery: one LOCALAPPDATA/CodeBuddyExtension/Data root and bounded conversation-level
  index.json enumeration; session registries excluded. Manual roots accept that Data directory only.
- Parse: state!=complete, type!=craft, conflicting cache splits/totals skipped with diagnostics.
  All-zero usage means no model call and is excluded. Key codebuddy:request:UUID makes
  cross-tree copies stable within one instance. session_id=conversation ID; host_application=
  host component, VSCode/CodeBuddyIDE. Bounded reads of messages referenced by requests[].messages
  choose the last message with model information. Mixed models within one request unverified.
  credit/lastTokens unmapped. Cursor offset stores file length for unchanged short-circuit;
  rewrites trigger full replay without duplicate event keys.
- Added codebuddy-extension-requests-doc1 format registry entry from local checks.

<a id="扩展存储验证与未完成条件"></a>

### Extension validation and remaining conditions

- cargo test --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core --lib
  adapters::codebuddy: four pass, request parsing/all-zero/unverified-state skips,
  duplicate requests within a file, same key across trees, combined dispatch/WorkBuddy protection.
- Native command: cargo run --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core
  --example real_verify_codebuddy -- &lt;home&gt; &lt;localappdata&gt; build/codebuddy-local-verify/work.
  files=14, added=1, diagnostics=0, one kimi-k3-1 record. Repeat added=0/count stable;
  input=uncached+cache_read and total=input+output pass, verdict=PASS. Values match native
  read-only inspection.
- npm run fmt:check/npm run clippy/npm run test:rust exit 0.
- CLI projects remains without native samples, document-based only. Extension request aggregate
  cannot separate multiple model calls within a turn. Mixed-model request attribution and
  lastTokens/credit semantics need more samples.
- Full checks rerun at repository root, 2026-09-30: npm run verify exit 0, Markdown/assets/
  scripts/UI/Svelte/fmt/Clippy/full Rust/web build; npm run test:browser exit 0, calendar heatmap/
  ten languages/themes/timezones/stale responses/user isolation/members/pagination/idle polling.
  Earlier outstanding full-verification rerun is complete.
