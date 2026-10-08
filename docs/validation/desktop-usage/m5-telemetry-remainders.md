# M5 telemetry and historical remaining work: Second checks, 2026-09-29

<a id="m5-遥测文件接收器与历史待办验证记录2026-09-29-第二轮"></a>

<a id="m5-遥测载体与历史余项实施验证记录2026-09-29-第二轮"></a>

This page retains that day's methods/results; the remaining-work table reflects that stage.
Later native files/field rules/rate limits/background/performance results:
[latest acceptance](current-acceptance.md). Current work: [Plan](../../../Plan.md).
Receiver settings below describe the historical implementation; current authentication rules
are in [receiver authentication](../../design/desktop-usage/receiver-auth.md).

Scope: address previous inventory's outstanding items: M5 local Copilot CLI/OTel spans/OTLP
receiver, M1 pre-migration backups, M6 per-source schedules, V20 initial performance, M7 idle
memory recheck. No commit/push/deployment.

<a id="本机数据状态变化2026-09-29-复查"></a>

## Local data changes: Recheck, 2026-09-29

- Copilot CLI data restored: ~/.copilot/session-store.db schema_version=8, assistant_usage_events
  36 native rows dated 2026-08-03, claude-opus-4.8. Entire tree absent on 2026-09-25 during
  upgrade migration, now written back.
- Claude ~/.claude only empty sessions/backups, without projects transcripts; native samples unavailable.
- CodeBuddy ~/.codebuddy diagnostics/logs/memwatch only, no usage files. Official monitoring
  documents CLI OTLP export only, without file exporter.
- APPDATA WorkBuddy/WorkBuddy AI directories reappeared but empty. Still no local format
  to verify; M4 paused state unchanged. Directory reappearance recorded separately.
- VS Code Copilot Chat globalStorage session-store.db contains only sessions/turns/session_files/
  session_refs, no usage table, matching the then-current A06 conclusion about telemetry requirements.

Later source investigation narrows the CodeBuddy conclusion to **this local directory state
and OTel file exporter**. Official directory docs separately establish ~/.codebuddy/projects
session JSONL; third-party source identifies usage fields and WorkBuddy .workbuddy/projects.
Both implemented from [local-session references](m4-buddy-local.md), initially without native samples.

<a id="m5-实施"></a>

## M5 implementation

<a id="1-copilot-cli-适配器真实数据核对-pass"></a>

### 1. Copilot CLI adapter: Native data check PASS

adapters/copilot/ common/detect/versions reads per-turn assistant_usage_events.
map_copilot moved from root usage_map.rs into its product directory, adding reasoning,
per V30. schema_version=8 KnownVersion; others latest-compatible.

- Native redacted tests/fixtures/copilot/assistant-usage-events-v8.sanitized.json: 36 rows
  of permitted columns, anonymous session_id/agent_id; expectations in _expectations.md.
- copilot_contract.rs rebuilds SQLite from the sample, calls run_adapter_scan, and matches
  manual input 4,649,981/output 34,157/cache read 4,416,791/cache write 233,118/reasoning
  17,678/uncached 72/total 4,684,138. Repeat scanning adds nothing.
- examples/real_verify_copilot.rs, desktop/src-tauri cwd: cargo run -p llm-usage-core
  --example real_verify_copilot -- "$USERPROFILE/.copilot" build/m5-copilot/verify.
  36 events, fields match independent SQL, stable repeat, VERDICT: PASS.
- All 36 rows satisfy input≥cache_read+cache_write, verifying the M0 rule on actual data.
  request_multiplier=27.0 premium multiplier and total_nano_aiu excluded from tokens.

<a id="2-otel-spans-jsonl-适配器文档级"></a>

### 2. OTel spans JSONL adapter: Document-based

adapters/otel/ reads three formats: VS Code Copilot Chat file exporter github.copilot.chat.otel.*,
official bdc5ebe NDJSON rather than OTLP, startTime seconds/nanoseconds pair; Copilot CLI
file exporter, undocumented per-line schema with related-format tolerance awaiting samples;
and this app receiver's normalized exports, CodeBuddy agentlens unprefixed usage.*.
Skip aggregate invoke_agent/codebuddy_code.interaction spans and model_request per official
double-count warning. Permitted attributes; accept both trace/span ID spellings since docs
do not specify exact keys.

<a id="3-本地-otlphttp-接收器src-taurisrcotel_receiverrs"></a>

### 3. Local OTLP/HTTP receiver: src-tauri/src/otel_receiver.rs

- 127.0.0.1 loopback only; otel_receiver_enabled off by default, otel_receiver_port=4318.
  At this stage, enabling represented explicit local-instance authorization, V25.
- OTLP/JSON and OTLP/protobuf; CodeBuddy protobuf-only. Explicit wire parsing ResourceSpans→
  ScopeSpans→Span→KeyValue→AnyValue. Bounded gzip decompression 64 MiB, headers 64 KiB/body
  64 MiB, V22 compressed-input protection.
- Save permitted gen_ai.*/usage.*/model* prefixes; explicitly exclude message-text keys such
  as gen_ai.input.messages; strings at most 256 B. Output APPDATA/llm-usage-desktop/otel/spans.jsonl,
  automatically discovered by OTel adapter.
- JSON/protobuf unit tests and actual E2E: receiver→HTTP POST /v1/traces→saved output→OTel
  reads codebuddy model_stream event, HTTP 200.
- Debugging fixed two defects: accepted connections from nonblocking listener need explicit
  blocking restoration; HTTP header separator now byte array [0x0d,0x0a,0x0d,0x0a].

<a id="m1-余项迁移重建清理前一致备份--空间检查"></a>

## M1 remaining work: Consistent backup and space checks before migration/rebuild/clear

src-tauri/src/db_backup.rs plus fs4 dependency:

- consistent_backup uses VACUUM INTO for a WAL-consistent snapshot. Check free space at least
  1.1×database including -wal, keep latest three backups, ordered by timestamp strings.
- app_state.rs schema-mismatch rebuild first calls read-only consistent_backup_legacy;
  failure aborts with original intact. clear_all_data uses shared helper with space check,
  replacing inline backup_before_clear.
- Pure space-check tests, independently reopenable backup round trip and retention of three.
  Windows open SQLite connections prevent deletion; test scopes now close them. fs4 ^0.13.

<a id="m6-余项逐源定时接线extraction_schedules"></a>

<a id="m6-余项逐源定时实现extraction_schedules"></a>

## M6 remaining work: Per-source schedules, extraction_schedules

- core/src/schedules.rs SourceScheduleRule: interval 15 seconds–24 hours/daily HH:MM/
  weekly ISO weekday+HH:MM; initial release without arbitrary cron. Pure next_due with
  jiff timezone and then-current DST-ambiguity +24h fallback. upsert/delete/read/due_instances/
  custom_scheduled_instances/mark_source_run with schedule_state. Five boundary/daily/weekly
  crossing/disabled/storage tests.
- Framework run_adapter_scan_filtered uses InstanceFilter include/exclude; original
  run_adapter_scan signature retained as delegation.
- scanner.rs global refresh excludes custom schedules. Poll due instances every 500 ms,
  trigger individually. FixedTime shares refresh with global/manual requests and merges
  same-source jobs without concurrent execution. Advance next_due after running;
  missed times on wake cause one scan.
- set_source_schedule registered in invoke_handler; list_sources returns schedule/nextDueMs.
  SourceList.svelte per-source editing: inherit global, interval presets 15 seconds–24 hours,
  daily/weekly time-day selectors and next extraction. Weekday names use locale Intl.
  Six i18n keys across ten languages, positional locales.ts insertion plus zh-CN/en.
- scheduling.md status updated to implemented.

<a id="v20-性能初值m1m7-前置数据"></a>

## V20 initial performance: M1/M7 baseline measurements

examples/bench_v20.rs, release, configurable size, desktop/src-tauri cwd:

| Scale | Insert throughput | Database + WAL | Daily-query p50/p95/p99 | Detail pagination, 200 rows, p50/p95/p99 |
| --- | --- | --- | --- | --- |
| One million events, 366 days/4 models/6 agents/4 sources | 30,453/s, 32.8 s | 676.8 MiB + 34.9 MiB | 0.01/0.01/0.01 ms | 0.67/1.24/1.52 ms |
| Ten million events | 23,560/s, 424.4 s | 6,788.3 MiB + 35.6 MiB | 0.01/0.01/0.02 ms | 2.28/4.65/5.41 ms |

No unbounded whole-file loading; indexed pagination. Ten-million temporary database cleaned.

<a id="m7-资源复测v21-部分数据"></a>

## M7 resource recheck: Partial V21 measurements

Release LLMUsage.exe GUI startup, headless scan, then idle: primary working set 32.5 MB plus
WebView2 children 138.5 MB, about 171 MB, within 180 MiB target. Earlier M0 minimal-shell
206.5 MB exceeded target; this measurement meets it. First-screen P95 and actual install/
uninstall remained M7 outstanding at this stage.

<a id="测试与命令汇总"></a>

## Tests and commands

- Repository-root npm run verify passes all checks: Markdown/Svelte zero errors or warnings,
  Cargo fmt/Clippy -D warnings/Vite build.
- desktop/src-tauri cargo test: 588+ pass. New Copilot format test 1/schedules 5/db_backup
  2/otel_receiver 4 including E2E, OTel adapter units and migrated mapping_v01 pass.
  V30 covers Copilot/OTel directories and moving map_copilot from the root.
- git diff --check passes; temporary files under ignored build/.

<a id="剩余未完成项如实登记"></a>

## Remaining work at this stage

| Item | Status | Missing requirement |
| --- | --- | --- |
| M5 native VS Code/Copilot CLI file exports | Await user enablement | Export must be enabled; per-line schema needs samples |
| M5 actual CodeBuddy CLI to receiver E2E | Await receiver enablement/CodeBuddy use; local sessions in [record](m4-buddy-local.md) | Receiver implemented/E2E with synthetic payload; overlap with sessions unresolved |
| M2 native Claude/Gemini/Qwen samples | No local data | Claude recheck finds empty directories only |
| M4 WorkBuddy | [Local-session adapter](m4-buddy-local.md) registered; later native eight-file/420-event check passed | Trace/session relationship and other versions unchecked |
| M3/M4/M8 native data acceptance | Deferred | Products not installed/no data locally |
| F2 cost engine | Main implementation complete, [record](f2-cost-engine.md) | Optional online refresh/usage-cost alerts/native E2E remain |
| M6 actual desktop V13–V18/V23–V25 and V24 system tasks | Await native operations | Manual/automated GUI records needed |
| M7 actual install/upgrade/uninstall V19, first-screen P95 V21/offline | Await actual environment checks | — |
| V26 first three-platform CI | Record after push | Push not authorized at this stage |
| File-change triggers | Not implemented; optional optimization, polling already allowed | Scheduling rules permit polling first |
