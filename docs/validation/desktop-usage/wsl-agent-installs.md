# WSL agent installation checks and cleanup, 2026-09-30

<a id="wsl-agent-产品安装验证与清理2026-09-30"></a>

Scope: install headless CLI products lacking native samples in M2/M3/M8 on WSL Debian,
user owent, Node 20.19.2/npm 10.9.9/Python 3.13.5. Network available, no passwordless sudo;
all installations are per-user. Check installation/actual saved data, then uninstall and
clean everything created by this run. No model calls or login: no credentials were available.
Windows read WSL files through the read-only UNC path
\\wsl.localhost\Debian\home\owent.

<a id="安装与首跑全部官方渠道npm-包名经-registry-身份核验"></a>

## Installation and first run: Official channels and npm package identity checks

| Product | Verified channel/version | Actual first-run files |
| --- | --- | --- |
| Claude Code | npm @anthropic-ai/claude-code, official repository checked; claude -p hi exits without login | Native transcript ~/.claude/projects/cwd-slug/*.jsonl, 8 lines, plus ~/.claude.json and backups/sessions |
| Gemini CLI | npm @google/gemini-cli 0.62.0 | ~/.gemini/projects.json.*.tmp; temporary write, no retained main file |
| Qwen Code | npm @qwen-code/qwen-code, executable reports 0.15.10 | ~/.qwen: installation_id/debug/skills/output-language.md |
| OpenCode | npm opencode-ai 1.18.33; opencode run | Native ~/.local/share/opencode/opencode.db with session/message/part tables and 7 event rows, plus snapshot/log |
| Command Code | npm command-code 1.72.4, executable reports 0.52.5 | --version/--help creates no configuration directory |
| gajae-code | npm gajae-code 0.18.1, official Yeachan-Heo package, plus bun 1.4.2 | gjc --version works; no data directory |
| OpenClaw | npm openclaw, official openclaw/openclaw | Defective package: package.json bin points to absent openclaw.mjs; executable not linked, product not run |
| goose | Official release v1.52.0 binary; download_cli.sh network failure, then latest release obtained through gh api | goose session list reports no sessions; no configuration directory |
| crush | npm @charmland/crush v0.97.1, channel from official Charm README | crush agent list requires TTY setup; no configuration directory |
| jcode | Official curl jcode.sh/install, v0.89.3 | ~/.jcode builds/logs/migrations/telemetry, no sessions until actual use |
| aider | Official aider-install via uv tool, 0.86.2; aider --yes --message hi | ~/.aider/analytics.json contains only opt-in state uuid/permanently_disable/asked_opt_in, plus installs.json/caches |

**npm name collisions:** npm jcode is an unrelated MVC framework; npm crush is a MediaCrush
remnant; goose-cli is a database migration tool; grok-cli is a third-party proxy; npm
hermes-agent is an unofficial bridge. None distributes the target official product.
Each was excluded after checking registry repository/homepage identity.

<a id="现成数据盘点安装前已存在与本次无关清理时保留"></a>

## Existing data: Predates this run and retained during cleanup

- ~/.local/share/kilo/kilo.db: native Kilo 7.6.2 database, 27 sessions/1,659 messages,
  retained from previous tests.
- ~/.omp/agent/agent.db: model_usage/usage_history/client_usage all contain zero rows.
- ~/.copilot: config/ide/logs only, no session-store.db.
- ~/.cline/data/db/*.db: Cline CLI interface, sessions/schedules tables with zero rows.
  It differs from the implemented VS Code extension's whole-file ui_messages.json arrays.
  The adapter matrix did not cover this CLI interface; no usage rows, not implemented.

<a id="真实验证结果probe_home-只读探针内存库不落盘"></a>

<a id="真实验证结果probe_home-只读检查内存数据库不保存到磁盘"></a>

## Native checks: Read-only probe_home with an in-memory database

New crates/core/examples/probe_home.rs uses the adapter registry moved into core through
adapters::built_in_adapters(), also reused by the app. Read-only discovery/scanning on WSL home UNC:

| Adapter | Result |
| --- | --- |
| claude | Native transcript discovered; first scan rejected unknown leading queue-operation. After the changes below: complete, 8 records, 0 events because synthetic placeholders were skipped |
| kilo | SQLite staging backup over UNC timed out under 9p locking. A consistent local snapshot through SQLite backup API completed: 1,659 records, 1,595 events/27 sessions/2 models; latest_fallback for unregistered native version 7.6.2 |
| opencode | Same UNC timeout; local snapshot completes, native 1.18.33 database detected, 0 events because usage rows are absent |
| jcode/goose/crush/aider/gemini/qwen | No usage directories, hence no roots discovered, as expected |
| Other 30+ adapters | No roots discovered |

Kilo/OpenCode staging backup timed out on the \\wsl.localhost 9p filesystem.
Manual WSL roots would encounter this limitation. Errors were reported and unknown reads
rejected; source data remained intact. Consistent local snapshots passed.

<a id="claude-code-21197-真实格式适配根据本次真实样本修复"></a>

<a id="claude-code-21197-真实格式适配本次新证据驱动的修复"></a>

## Claude Code 2.1.197: Changes based on native records

Native transcripts may start with queue-operation enqueue/dequeue. attachment/last-prompt
are non-usage metadata. An unauthenticated placeholder assistant has model=&lt;synthetic&gt;
and all-zero usage. claude/detect.rs and transcript_doc1.rs now skip these three metadata
types while rejecting them if they carry usage. Synthetic assistant placeholders produce
synthetic_assistant_skipped diagnostics and no events, rather than counting zero-usage calls.
Redacted tests/fixtures/claude/real-2.1.197-queue-metadata/ preserves all 8 record structures
with anonymous IDs.

- Two format tests cover metadata acceptance/zero events/stable repeats and rejection of
  metadata carrying usage as pending. All four claude_contract tests pass; native WSL
  file rescan completes.

<a id="命令与退出码关键节点"></a>

## Commands and exit codes

| Command/environment | Exit | Result |
| --- | --- | --- |
| npm install -g twice, plus bun/pip/uv, WSL per-user | 0 | 11-product installation trial; defective OpenClaw package excluded from successful product installation |
| cargo build --release -p llm-usage-core --example probe_home | 0 | Probe built |
| probe_home.exe against WSL home UNC, three runs | 0 | 39 adapters checked against actual home; results above |
| probe_home.exe against local snapshots, kilo opencode | 0 | 1,595 events/27 sessions/2 models |
| cargo test -p llm-usage-core --test claude_contract | 0 | Four passed |
| npm run verify, repository root before push | 0 | All unified checks passed |
| npm run build:desktop after icon/sample-data fixes | 0 | Installer 2.81 MiB |
| Per-item cleanup inspection | — | All created directories/binaries/caches/temp files absent; ~/.local/bin retains only existing npm/npx/omp/pnpm/yarn |

<a id="清理验证完成后执行"></a>

## Cleanup performed after validation

Uninstalled eight npm packages, reporting 672 packages removed; removed the owned
~/.local/npm-global and ~/.bun directories. Uninstalled aider-chat through uv tool,
aider-install/uv through pip, and removed goose/jcode binaries. Removed created
.claude/.claude.json/.qwen/.gemini, OpenCode configuration/cache/share directories,
.jcode/.aider/.local/share/uv and temporary files under /tmp. **Preexisting .cline/.omp/
.copilot/Kilo data was retained.** Individual checks found no run-created leftovers.

<a id="后续发现ci-暴露的暂存备份竞速已修复"></a>

## Later finding: CI exposed a staging-backup race, now fixed

After this push, Linux CI exposed an existing defect in nine SQLite staging adapters:
Busy/Locked retries without progress increased done_pages, creating a race between the
space limit and timeout. Linux hit the page limit after 20.5 seconds and incorrectly
reported space cap for busy databases. Windows reached timeout first, so local checks
passed; WSL UNC timeout uses the same backup path. Revision cb28454, 2026-09-30, counts
pages only for StepResult::More. Busy/Locked consumes elapsed time only. Windows/WSL
rechecks agree, and CI 36709419197 passed. See [CI record](m0-ci.md).

<a id="未完成后续"></a>

## Remaining work at that stage

1. No credentials/login across products: newly generated Claude synthetic/OpenCode
   empty data has zero usage events; nonempty native usage samples still require credentials.
2. goose/crush/command-code/gajae need actual sessions to create data directories;
   this run obtained none.
3. Cline CLI ~/.cline/data/db has zero rows; no native usage sample, not implemented,
   recorded as a separate adapter-matrix limitation.
4. SQLite staging backup over UNC times out. Future WSL manual-root support needs an
   appropriate 9p snapshot method or a documented limitation.
5. This record supplies verified installation versions, directory structure and npm
   identity collisions for the adapter matrix.
