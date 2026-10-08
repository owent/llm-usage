# M3: Cline, DSH, Hermes, OpenClaw and OpenCode-family adapters from documents/source

<a id="m3基于文档或源码实现的-clinedshhermesopenclawopencode-家族适配器"></a>

<a id="m3clinedshhermesopenclawopencode-家族文档级证据适配器"></a>

Four products were locally absent in the 2026-09-25 inventory, with candidate paths such
as ~/.cline missing and all four real_verify examples discovering zero roots. The user
directed implementation against fixed source/official documentation. Synthetic tests pass;
**native-data acceptance was deferred** until installation/discovery and real_verify checks.
Adapters and samples explicitly identify document/source references.

<a id="元信息"></a>

## Run information

| Item | Value |
| --- | --- |
| Date | 2026-09-25 |
| Environment | Windows 11 x64; rustc 1.98.0 |
| Code revision | Uncommitted tree after M3/M4 plus this work |
| Design references | adapters.md A03/A08/A24/A09 fixed source; V03/V07/V12/V17/V30 |
| Execution | Three parallel subagents stopped at API limits, 2026-09-25 21:33; each adapter's five files compiled. Main session added tests/examples, fixed inline failures and integrated |

<a id="实现与依据"></a>

<a id="实现与证据"></a>

## Implementation and references

<a id="clinea03固定源码-dcf8c3c"></a>

### Cline: A03, fixed source dcf8c3c

- Usage comes from text JSON on type=say with say in api_req_started/deleted_api_reqs/
  subagent_usage. tokensIn/tokensOut/cacheWrites/cacheReads/cost individually optional;
  four exclusive buckets derive input_total=in+cw+cr. Compaction estimates excluded.
  Task ui_messages.json is a whole-file array under the task directory.
- Inline tests exposed missing documented request key and missing negative/nonfinite
  map_cost rejection; main session fixed both consistently with parse rules.
- Three format tests: synthetic end-to-end three calls/input 475/total 530 and stable
  repeats; documentation-based capability label; partially usable boundary scenarios.

<a id="dsha08固定-token-meter-readme-46a7f68"></a>

### DSH: A08, fixed token-meter README 46a7f68

- Final replaces streaming samples of the same attempt. retry-started ends replacement
  scope and starts another billable attempt. Attempt/step boundaries finalize the latest
  sample. occurred_at uses observation time because README lacks per-event timestamps.
  pressure/contextBreaks estimates excluded.
- Four tests: two attempts/input 140/total 158 including streaming→final/retry; replacement
  scenario three attempts/input 195/total 208; non-usage vocabulary creates no requests;
  capability clearly says synthetic assumptions.
- Persistence path/row serialization undocumented; proposed JSONL shape remains a labeled
  synthetic assumption.

<a id="hermesa24固定源码-ef70b36"></a>

### Hermes: A24, fixed source ef70b36

- session_model_usage is cumulative per model/route/task, not request-level. Map to source
  intervals/UsageObservation; api_call_count does not become model_call. Cross-day first_seen/
  last_seen never assigns the entire total to the last day, following V03. Auxiliary task
  and main-session cumulative values are mutually exclusive.
- Eight tests from subagent, with two type/assertion fixes during integration: basic
  cumulative, cross-day, exclusive auxiliary totals, v20 backfill/compaction inheritance,
  capability evidence_level=doc-level.
- projection.json rebuilds SQLite; schema DDL accompanies that extracted test data.

<a id="openclawa09官方文档"></a>

### OpenClaw: A09, official documentation

- Documents then established only ~/.openclaw/agents/id/agent/openclaw-agent.sqlite and
  legacy sessions archive locations, **without table/entry schema**. Runtime DB/archive
  return unknown_format with pending-native-sample reasons. No table/field guesses or
  invented zeros. Archives treated as migration input following doctor --fix.
- Three tests: runtime rejection/stable repeats, archives excluded even with placeholder
  1200+300 values, accurate capabilities. layout.json rebuilds temporary SQLite/JSON/JSONL layout.

<a id="命令与结果主会话集成后"></a>

## Commands and results after integration

| # | Command (directory) | Exit | Result |
| --- | --- | --- | --- |
| 1 | cargo test -p llm-usage-core | 0 | Cline 3, DSH 4, OpenClaw 3, Hermes 8 and additional inline tests pass; verify includes 53 test binaries |
| 2 | cargo run --example real_verify_{cline,dsh,hermes,openclaw} | 0 | All four discovered_roots=0; local data directories absent |
| 3 | npm run verify | 0 | Lint/Clippy/fmt clean; frontend build unchanged |
| 4 | APPDATA=temporary cargo run -p llm-usage-m0 -- --headless | 0 | 14 registered adapters; 7 instances/27,158 events, same order of magnitude as prior batch with active-file changes |

<a id="过程缺陷与修复"></a>

## Problems and fixes during work

| Location | Problem | Fix |
| --- | --- | --- |
| scanner.rs | Registry overwritten twice with stale external editor/tool buffers; app discovered zero versus probe seven roots | Restore integration and assert with script; discover_probe.rs prints per-adapter roots using app context |
| Cline inline tests | request absent from known keys; map_cost accepted negatives | Add key; reject nonfinite/negative cost as parse does |
| hermes_contract | Fourth tuple field incorrectly nonnullable; evidence_level assertion too strict | Nullable type and prefix assertion |

<a id="opencode--mimo-code--zoo-code2026-09-26-第二批a17a14a19"></a>

### OpenCode / MiMo Code / Zoo Code: Second batch, 2026-09-26, A17/A14/A19

- **OpenCode**, 0027387: part.data.type=step-finish has input, output, reasoning,
  cache.read/write and cost. Source projector/publish-llm-event explicitly establishes
  inputTokens=in+cr+cw and total=sum of five exclusive fields. session.tokens_* compares
  totals, matching here; message.data turn totals excluded to avoid duplicates. Exact
  call time/model-switch order needs event layer; this stage uses observed_at.
- **MiMo**, 456678b: related layout with distinguishing message.agent_id, no session
  cumulative columns, MIMOCODE_HOME/data. Shared ancestry does not establish compatibility;
  both adapters reject the other's format.
- **Zoo**, f780647: Roo-related whole ui_messages.json, LIFO start/finish pairing with
  finish replacing start and unmatched finish discarded. condense_context.cost contributes
  upstream totalCost as auxiliary work with unknown tokens. **tokensIn includes cache**,
  unlike the OpenCode family.
- adapters/opencode_family.rs shares step-finish scan for OpenCode/MiMo with injected product
  identity. Sixteen format tests plus twelve module tests pass.
- **Cross-product collision fixed:** Kilo held matching opencode-rc.db; initial discovery
  misidentified it as OpenCode and matched 14 comparison events. Restrict discovery to
  opencode/mimocode directories or direct MIMOCODE_HOME/data children. After repair,
  not_found agrees with inventory; negative tests and capability collision warning retained.

<a id="未完成项"></a>

## Outstanding work at this stage

| Item | Status | Next step |
| --- | --- | --- |
| Native data for seven products | Deferred; locally not_found | After installation, real_verify/redacted samples; verify each session.version before known_version |

<a id="验证产物"></a>

<a id="证据文件"></a>

## Validation files

- adapters/{cline,dsh,hermes,openclaw}/.
- `tests/{cline,dsh,openclaw,opencode,mimo_code,zoo}_contract.rs`, tests/hermes_contract.rs.
- tests/fixtures/{cline,dsh,hermes,openclaw}/: synthetic data and expectations.
- real_verify_{cline,dsh,hermes,openclaw,opencode,mimo_code,zoo}.rs and discover_probe.rs.
