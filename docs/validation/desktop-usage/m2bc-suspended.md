# M2-B/C interruption: pi, oh-my-pi, Claude, Gemini and Qwen adapters

<a id="m2-bc-中断记录pioh-my-piclaudegeminiqwen-适配器半成品保留未验收"></a>

> **Resumed and accepted on 2026-09-25**; see [the recovery record](m2bc-resumed.md).
> This historical record preserves the interruption and instructions for resuming on another machine.
> The completion-gap table below includes the recovery results.

This is an interruption record, not a passing acceptance report. The user stopped the parallel
M2-B (pi/oh-my-pi) and M2-C (Claude Code/Gemini CLI/Qwen Code) implementation tasks on 2026-09-24.
Their unfinished worktree changes were retained after compilation/tests passed. Before recovery,
the adapters could not be reported as complete.

<a id="元信息"></a>

## Metadata

| Item | Value |
| --- | --- |
| Date | 2026-09-24 |
| Environment | Windows 11 x64; rustc 1.98.0; Node v24.21.0 / npm 12.0.2 |
| Revision | Uncommitted worktree: accepted M2-A changes already staged by another session, plus untracked/unstaged changes left by this interruption |
| Requirements | [Adapter matrix](../../design/desktop-usage/adapters.md); [validation requirements](../../design/desktop-usage/validation.md) V07/V12/V17 |

<a id="中断经过与恢复方式"></a>

## Interruption and recovery

| Task | Scope | Duration | Reason | Recovery method |
| --- | --- | --- | --- | --- |
| M2-B, background agent-16 | pi/oh-my-pi adapters | About 57 minutes | User requested a pause and written status | On the original machine: `Agent(resume="agent-16", "继续")`; on another machine, use the recovery instructions below |
| M2-C, background agent-17 | Claude/Gemini/Qwen adapters | About 57 minutes | Same request | On the original machine: `Agent(resume="agent-17", "继续")`; on another machine, use the recovery instructions below |

<a id="换机恢复方案2026-09-24-增补"></a>

## Recovery on another machine: 2026-09-24 addition

<a id="换机后会失效的东西"></a>

### State that does not transfer automatically

- Agent-16/17 contexts exist only in the original machine's `~/.kimi-code/sessions/`, outside
  the repository. A new agent can use the brief below instead of resuming those sessions.
- All uncommitted M0-and-later work, including this record, exists only in the worktree/index.
  The latest `git log` entry remained `9394264 P1`; failing to transfer these files loses the work.
- `build/desktop-usage-validation/` contains ignored raw verification artifacts. The conclusions
  are recorded in m2a-codex.md/m0-agent-fixtures.md and transfer with Git; copy the ignored directory
  separately when raw artifacts are needed.
- The Claude/Gemini/Qwen not_found results and empty pi sessions/no_data result apply to the
  original machine. Another machine may have installations and native data. Repeat discovery on
  that machine and collect additional native samples where available.

<a id="第一步保存并转移工作树三选一"></a>

<a id="第一步把工作树固化并转移三选一"></a>

### Step 1: preserve and transfer the worktree

| Option | Original-machine operation | Restore on the new machine | Applicability |
| --- | --- | --- | --- |
| A. Commit and push, recommended | `git add -A && git commit -m "M0-M2 阶段性成果与 M2-B/C 中断半成品" && git push origin main`; push uploads LFS objects | `git clone git@github.com:owent/llm-usage.git && cd llm-usage && git lfs pull` | GitHub access; preserves commit history and a recovery revision |
| B. Git bundle | Commit as in A, then `git bundle create llm-usage.bundle main` | `git clone llm-usage.bundle` | Avoids a remote push; bundles omit LFS objects, so copy `.git/lfs/` separately or SVG/ICO assets remain pointers |
| C. Copy the entire directory | Copy `llm-usage/`, including `.git/`, using file sync/USB/rsync; node_modules/target may be omitted | Work directly in the copied directory | Preserves worktree/index bytes with fewer steps, but creates no additional commit recovery point |

After any transfer, reinstall dependencies and verify the baseline on the new machine.
The original recorded command block is retained below:

```bash
npm ci                 # Repository root; root scripts load desktop dependencies (run npm --prefix desktop ci if needed)
npm run verify         # Baseline: core 143 + app 2 = 145 passed; lint/clippy/fmt/svelte-check passed
```

Its notes require root dependencies, optional `npm --prefix desktop ci` if desktop dependencies
are absent, and the historical 145-test baseline (core 143/app 2), with lint/clippy/fmt/Svelte checks passing.

<a id="第二步新机环境前提"></a>

### Step 2: new-machine prerequisites

| Dependency | Requirement | Original-machine result |
| --- | --- | --- |
| OS | Windows 11 x64 for first-release acceptance; WSL2 may run a Linux build smoke check | Windows 11 x64 |
| Node.js | 22+, under AGENTS.md requirements | v24.21.0 / npm 12.0.2 |
| Rust | Stable; edition 2021, no pinned toolchain file | 1.98.0 |
| git-lfs | Required; .gitattributes uses LFS for SVG/PNG/ICO and other assets | Enabled |
| WebView2 | Required for the GUI smoke check with npm run dev:desktop | Passed |

<a id="第三步恢复简报换机后给新-agent替代-agent-resume"></a>

<a id="第三步冷启动简报换机后给新-agent替代-agent-resume"></a>

### Step 3: brief for a new agent

> Continue M2-B/C in llm-usage. Read Plan.md for status; this interruption record for unfinished
> files and gaps; m2a-codex.md for the accepted adapter standard; and adapters.md/data-contract.md
> for source and field rules. Four unfinished adapters (pi/Claude/Gemini/Qwen) and shared usage_map
> already compile and pass their inline unit tests. Add integration tests matching Codex's
> contract/gaps/incremental groups; tests/common/mod.rs already has helpers, but the sample
> directories remain to be created. Determine whether oh-my-pi needs a separate adapter.
> Rediscover native data on the actual machine: the old machine's Claude/Gemini/Qwen not_found
> and empty pi sessions do not apply automatically. Extract/redact available native samples
> under m0-agent-fixtures rules; otherwise record not_found/no_data. Add real_verify examples
> and acceptance records. The historical brief required tests without mocks, unknown values
> left unknown, rejection of unknown versions, no previous-draft changes, and no unauthorized
> commit/push. npm run verify must pass; the 145-test baseline may increase, not decrease.

<a id="半成品清单全部保留在工作树未提交"></a>

## Retained unfinished files: uncommitted worktree

| File | State | Content and verified scope |
| --- | --- | --- |
| desktop/src-tauri/crates/core/src/adapters/pi.rs | Untracked; 985 lines/three unit tests | pi session JSONL v3; read-only online checks against pi-mono b4559750; local pi 0.87.1 sessions empty, native status no_data |
| desktop/src-tauri/crates/core/src/adapters/claude.rs | Untracked; 680 lines/three unit tests | A01 documentation only; header marks native samples pending; local not_found |
| desktop/src-tauri/crates/core/src/adapters/gemini.rs | Untracked; 658 lines/two unit tests | Similar documentation-based implementation; native samples unverified |
| desktop/src-tauri/crates/core/src/adapters/qwen.rs | Untracked; 705 lines/two unit tests | Similar documentation-based implementation; native samples unverified |
| desktop/src-tauri/crates/core/src/adapters/usage_map.rs | Unstaged; +195 lines | PiFamilyUsage/map_pi_family based on fixed pi-mono b4559750 and oh-my-pi 62bc57b rules; ClaudeTranscriptUsage and other shared mappings |
| desktop/src-tauri/crates/core/src/adapters/mod.rs | Unstaged; +five lines | Registers the four modules |
| desktop/src-tauri/crates/core/tests/common/mod.rs | Unstaged; +114 lines | M2-C temporary-directory/sample-path helpers; tests/fixtures/{claude,qwen,gemini}/ and integration tests do not yet exist, so the helpers have no callers |

No independent oh-my-pi adapter file was found; only shared map_pi_family rules existed.
Whether M2-B had completed oh-my-pi sampling was unverified; the historical recovery instruction
was to ask agent-16 when its session remained available.

<a id="命令与结果中断后核验"></a>

## Checks after interruption

| # | Command and cwd | Exit | Results |
| --- | --- | --- | --- |
| 1 | cargo test --locked -p llm-usage-core; desktop/src-tauri | 0 | 143 passed/zero failed, including ten new inline adapter tests: pi three, Claude three, Gemini two, Qwen two |
| 2 | npm run verify; repository root | 0 | Markdown lint: 57 files/zero issues; assets:check/test:scripts/Svelte/fmt/clippy -D warnings passed; locked Rust tests core 143/app 2 = 145, compared with M2-A's 135; Vite 518.01 kB/gzip 176.33 kB. .venv/** was added to markdownlint ignores because it is an ignored local Python environment, not project documentation |

<a id="与-m2-完成条件的差距恢复后待办"></a>

## M2 completion gaps and recovery results

> Recovery completed on 2026-09-25; see [the recovery record](m2bc-resumed.md).

| Item | Status | Result | Follow-up |
| --- | --- | --- | --- |
| Integration tests matching Codex contract/gaps/incremental groups | Complete, 2026-09-25 | Five sources, 15 integration-test files/103 passing cases; oh-my-pi uses independent omp.rs | — |
| Synthetic/native test data | Complete, 2026-09-25 | pi: one native/seven synthetic; omp: three native/nine synthetic; Claude/Gemini/Qwen: eight synthetic each. Local no_data/not_found results remain in the recovery record | Extract/redact native samples for those three when data becomes available |
| Read-only native checks matching real_verify_codex | Complete, 2026-09-25 | Six examples; pi/omp passed repeat-read/invariant checks; other three recorded no_data/not_found | — |
| Repeat scans add no usage; primary/auxiliary/sub-agent coverage visible | Accepted, 2026-09-25 | All five V12 incremental suites passed; native omp counts separate primary/sub_agent | — |
| Version-specific legacy Codex rejection samples | Partial, 2026-09-25; [M2-D](m2d-layout-versions.md) | 0.153.0/0.154.0-alpha.6.1/6.2 verified; native 0.139–0.151 lack per-call records, requiring a separately verified implementation | Follow M2-D conditions |
| Adapter directories/unknown-version compatibility, execution.md#m2-layout | Complete, 2026-09-25; [M2-D](m2d-layout-versions.md) | Six adapter directories, registry dispatch and persisted-compatibility regressions passed | — |

<a id="验证产物"></a>

<a id="证据文件"></a>

## Verification artifacts

- The four adapters/*.rs headers list their fixed-source commits/documentation references individually.
- Repeated worktree command results establish the checks in this record; no additional redacted
  artifacts were written to build/desktop-usage-validation/ during the interruption check.
