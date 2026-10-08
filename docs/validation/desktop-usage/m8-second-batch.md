# M8 second-batch adapter implementation checks

<a id="m8-第二批适配器实施验证记录2026-09-29"></a>

Historical implementation date: 2026-09-29. On 2026-10-06, official container/local-model
runs added nonempty native samples, independent comparisons and regressions for Aider 0.86.2,
Goose 1.53.0, Continue CLI 1.5.47, jcode 0.91.0, gajae-code 0.18.7, AtomCode 5.2.1,
Crush 0.97.1, Junie 26.9.22, Xum 0.30.0 and Roo 3.54.0. Crush supplied cost only; Junie
failed its task after writing seven calls; Xum's default stream lacked usage, with gateway
upstream-option comparisons recorded separately; Roo cache/cancelled-tail gaps remain.
See [M8 container samples](m8-container-samples.md). The findings below retain the earlier
documentation-based implementation stage. Later native checks/field corrections, including
Zed, are indexed in [current acceptance](current-acceptance.md).

Scope: documentation-based M8 coverage from [A25–A48 research](../../design/desktop-usage/research.md#agents)
and [the adapter matrix](../../design/desktop-usage/adapters.md#扩展覆盖). Added 18 independent
directories, 17 parsers plus a Qoder probe, registration and requirement tests.
No commit/push/deployment occurred in this implementation stage.

<a id="本机环境与盘点"></a>

## Local environment and inventory

- Windows 11 x64, Node 22+, repository-root cwd; complete check: npm run verify.
- Read-only home/APPDATA/LOCALAPPDATA inventory on 2026-09-29 found no M8 installations.
  Only %LOCALAPPDATA%/Zed/threads/threads.db existed. Its threads schema matched official
  migration SQL with zero rows; this verifies schema without native usage.
- Follow the earlier M3 absent-product approach: implement from fixed source/official
  distributions/third-party parser references with synthetic test data. Local discovery
  returned no roots, except Zed's empty DB. Native samples require later product checks.

<a id="实施前的产品资料核验"></a>

<a id="深度调研先于实施逐产品证据核验"></a>

<a id="深度调研先于实施逐产品资料核验"></a>

## Product reference checks before implementation

Parallel read-only research checked the following versions. Field details are in
[research.md](../../design/desktop-usage/research.md#agents) and each adapter header.

| Product | Checked reference | Reference type |
| --- | --- | --- |
| Roo Code | RooCodeInc/Roo-Code b867ec9（archived 2026-05） | official-source |
| Goose | aaif-goose/goose a701bb1 | official-source |
| Crush | charmbracelet/crush 1f3827b | official-source |
| jcode | 1jehuang/jcode 4f6bf8e（master） | official-source |
| gajae-code | Yeachan-Heo/gajae-code 7e54f9c | official-source |
| Command Code | npm command-code@1.69.0 dist/cli.mjs line-by-line | official-distribution |
| Continue | continuedev/continue 5522c6f | official-source |
| AtomCode | atomgit e4215f7（same SHA in GitHub mirror） | official-source |
| Zed | zed-industries/zed bd74733 + local schema | official-source + local schema |
| Aider | Aider-AI/aider 5dc9490 | official-source |
| Amp/Grok/Junie/Kiro/Droid/Xum/Antigravity | tokscale fixed commit 1d9a939 | third-party-parser / reverse engineering |
| Amazon Q | aws/amazon-q-developer-cli 15cc8f3 | official-source（**no local token records**） |
| Codebuff | CodebuffAI/codebuff caec5fc | official-source（**local credits only, no token fields**） |
| iFlow CLI | iflow-ai/iflow-cli 4642808（**2026-04-17 service discontinued**） | official-docs（no default usage records） |
| Qoder | docs.qoder.com + npm 1.1.64 unpacked | path-verified / fields-unverified |

Findings used for admission at this historical stage:

- Goose has per-request usage_ledger in official migration 15: Unix seconds, five token
  buckets and cost_source. tokscale covered session aggregates only. Older DBs fall back
  to sessions.accumulated_* aggregates.
- Crush prompt_tokens/completion_tokens describe the latest step's context size: overwritten
  by SET and reset after compaction. Only root-session cost is cumulative; child cost rolls
  into parents, so parent_session_id IS NULL selects roots. Implement cost-only collection.
- Roo tokensIn includes cache, checked at three official-source locations. Legacy Cline
  dcf8c3c defines four exclusive buckets. Each adapter uses its own references; record the
  difference in adapters.md and require separate native samples.
- jcode preserves provider input semantics: OpenAI input includes cache, allowing derived
  uncached input; Anthropic's three input buckets are exclusive; unknown semantics permit
  no derivation. Merge snapshots/journals and upsert by message ID across crash windows.
- gajae officially normalizes exclusive usage buckets, compatible with map_pi_family rules.
  Require all five buckets as the official statistics parser did. message.timestamp uses
  milliseconds alongside ISO entry timestamps.
- Command Code inputTokens includes cache under its official cost formula. Follow parentId
  backward from the final entry; copied forks preserve id/timestamp for cross-file deduplication.
- Continue writes usage only through CLI, as session cumulative actual API values.
  devdata.sqlite contains tokenizer estimates and is excluded; GUI sessions lack fields.
- Historical Zed reading used cumulative usage for statistics and request buckets for
  comparison; official thread.rs:2893 resets request buckets. At this stage TokenUsage's
  omitted serialized zero was treated as reported zero, with zed.dev-only provider coverage
  and imported threads skipped. Later native field rules revise the applicable scope.
- Amazon Q/Codebuff have no local per-call tokens: Q's type conversion discards API tokenUsage;
  Codebuff saves credits only. Implement no token adapter, under the same local-source
  restriction applied to Warp/Cursor.
- iFlow discontinued service; its only usage route needs opt-in OTel under M5 OTLP
  requirements, so no independent adapter is implemented.
- Qoder fields remain unverified; bundle-schema hints cannot establish them. Provide only
  detection that rejects unverified formats.

<a id="实施18-个适配器全部独立目录--版本注册表--v30"></a>

## Eighteen independent adapters and V30 registries

Under desktop/src-tauri/crates/core/src/adapters/: zed, aider, junie, xum, droid, amp, grok,
roo, goose, crush, jcode, gajae-code, commandcode, continue (module continuedev), atomcode,
kiro, antigravity and qoder (probe). All are registered in built_in_adapters().

The matrix's boundaries apply individually:

- Exclude estimates: Kiro Auto zero counts/byte conversion/window differences, Goose reasoning
  differences, Droid transcript allocation, Grok cumulative differences/compensation,
  Continue devdata and Crush token snapshots.
- Amp's timestamped ledger supplies statistics; message usage is comparison-only and cannot
  invent timestamps. Kiro CLI turns and kiro-cli SQLite remain separate, with overlap unverified.
- Preserve session aggregates without inventing per-call records: historical Zed cumulative
  statistics/request comparison, Xum, Droid, Continue, AtomCode summed round_count and old Goose DBs.
- Added zstd ^0.13 for bounded Zed threads.db blob decompression, maximum 64 MiB.

<a id="测试与命令"></a>

## Tests and commands

- cargo test -p llm-usage-core, cwd desktop/src-tauri: all 566 tests passed, including 158
  library tests and integration tests. New tests/m8_contract.rs has 19 cases: 17 parser
  requirement checks, Qoder rejection and V30 directory/registry structure.
- tests/adapter_layout_v30.rs expands its AGENTS list across all M8 directories.
- npm run verify, root: Markdown lint 150+ files/zero issues; assets/scripts/frontend logic/
  Svelte zero errors/warnings; fmt/Clippy -D warnings/all Rust tests/Vite 8.3.1 build passed.
- git diff --check passed; git status --short had no temporary files because build/ is ignored.

<a id="核验范围与剩余缺口"></a>

<a id="证据等级与剩余缺口"></a>

## Verification scope and remaining gaps

| Reference type | Adapter |
| --- | --- |
| official-source, including distributions, plus local schema | zed |
| official-source / official-distribution | roo, goose, crush, jcode, gajae, commandcode, continue, atomcode, aider |
| third-party-parser, tokscale 1d9a939 | amp, grok, junie, kiro, droid, xum |
| third-party-reverse-engineered, highest uncertainty | antigravity protobuf layout; reject 1.1.18+ rows without timestamps |
| path-verified / fields-unverified | qoder probe rejects unverified formats |

Remaining items at that stage:

1. Native acceptance for all 18 products was deferred: local directories lacked usage, with
   Zed's DB empty. Redacted native samples must verify each product's reconciliation/coverage/
   timezone boundaries before support status changes.
2. Amazon Q/Codebuff/iFlow have no token adapters because of absent local records/discontinued
   service. Matrix statuses remain; Cursor/Warp/TRAE/Windsurf retain exclusion/F1 status.
3. Kiro file/DB overlap, Roo/Cline tokensIn semantics and Antigravity 1.1.18+ timestamp layouts
   require native samples.
4. The initial stage changed core/registration only, without browser/release reruns. Subsequent
   worktree review added browser checks below; release builds remained part of M7 acceptance.

<a id="f6ede266590b37c14f424070b048ffd773f3ae9c-工作区复核2026-09-29"></a>

## Worktree review after f6ede266590b37c14f424070b048ffd773f3ae9c: 2026-09-29

Reviewed 74 staged changes after that commit and repaired confirmed issues. Changes remained
uncommitted/unpushed. Windows 11 x64, Node.js v24.21.0, Cargo 1.98.0; npm/Git from root,
targeted Cargo commands from desktop/src-tauri.

Confirmed corrections:

- Zed cumulative totals switched to checked arithmetic, but callers still treated them as
  integers, breaking core compilation. Overflow now records a diagnostic and skips the thread.
- Old Goose session cumulative values can grow; after a complete page, cursors recheck from
  the beginning. Crush cumulative cost uses updated_at as revision, avoiding conflicts
  between later costs and old Final events sharing an ID. Zed/Kiro/Crush paginate beyond
  50,000 rows and recheck mutable rows after reaching the final page.
- jcode journals resume persisted offsets after line limits; after the last page they reread
  session metadata. Previously every run restarted at the beginning, leaving the tail
  unreachable. Message source_revision uses snapshot/journal updated_at instead of completion
  time. Same-millisecond content changes still produce ingestion conflicts.
- Xum versions use top-level fields of complete JSON only; truncated headers use compatibility
  fallback. Partially written Kiro SQLite magic remains Pending; shared header reading handles short reads.
- Worktree differences verified disabled-source scan prevention, per-source due filtering,
  actual scan results, OTel error propagation, SourceList weekday timezones and source-list indexes.

tests/m8_review_fixes.rs covers pagination/cumulative updates/journal resumption/corrections
with unchanged message timestamps, disabled sources, version selection, invalid buckets and
single-line errors; Zed overflow also has a unit test. npm run verify at root exited 0 for
Markdown/assets/scripts/frontend/Svelte/Rust format-Clippy-tests/Web build. npm run test:browser
at root exited 0. Default sandbox Node child creation failed with spawnSync EPERM; the same
command passed through approved escalation. This infrastructure failure is separate from product failures.

Acceptance remained documentation/synthetic-data only; this review added no native M8 usage.
The local default Copilot DB was absent, so earlier checks of 36 native rows are not this
round's repeated acceptance. Native samples/Kiro overlap/Roo-Cline input semantics/Antigravity
timestamp gaps remained. M2 default %USERPROFILE%/.claude, .gemini and .qwen paths were absent;
this describes that shell's defaults without verifying environment/manual overrides. M2 native
sample gaps remained.
