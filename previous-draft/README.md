# LLM usage dashboard (standalone prototype)

<a id="llm-用量看板独立版"></a>

Migrated from Kimi Work's LLM usage dashboard as a standalone local prototype.
Its runtime behavior has not been verified for the current desktop project; the descriptions
below preserve the prototype's documented behavior.

- `collect.py` / `store.py` / `collectors/`: collection/statistics, SQLite cache at `data/usage.db`.
- `dashboard/`: static ECharts dashboard reading `dashboard/data.json`.
- `data/usage.db`: historical migration snapshot, with a 366-day rolling window.

<a id="快速开始"></a>

## Quick start

```bash
cd /d D:/tmp/llm-usage

# 1. Collect incrementally and generate dashboard data; first runs backfill source history
python collect.py

# 2. Open the dashboard
start dashboard/index.html        # Windows; alternatively double-click index.html
```

In standalone mode, Refresh today's data reloads data.json. Run python collect.py again to
update it; file-protocol pages need a refresh.

<a id="常用参数"></a>

## Common arguments

```bash
python collect.py --refresh           # Ignore incremental state and rescan all
python collect.py --tool kimi-code    # Run a specified collector; repeatable option
python collect.py --out output.json   # Custom snapshot output
```

<a id="支持的数据源"></a>

## Supported sources

| Tool | Source |
| --- | --- |
| oh-my-pi | `~/.omp/agent/sessions/*/*.jsonl`, `~/.omp/logs/*.log` |
| kilo code | `~/.local/share/kilo/kilo.db` |
| codex | `~/.codex/sessions/**/rollout-*.jsonl` |
| zcode | `~/.zcode/cli/rollout/model-io-*.jsonl` |
| copilot | `~/.copilot/session-store.db`, `~/.copilot/session-state/*/session.db` |
| kimi work | kimi-code engine `…/kimi-code/home/sessions/**/wire.jsonl` (`usage.record`) |
| **kimi code** | **Newer v2.x `~/.kimi-code/sessions/**/wire.jsonl` (`usage.record`)** |

<a id="新版-kimi-codekimi-code-采集器"></a>

### Newer Kimi Code (kimi-code collector)

The newer Kimi Code desktop 2.x home is ~/.kimi-code. Events still use
sessions/**/agents/&lt;agentId&gt;/wire.jsonl. The collector scans the whole sessions tree recursively,
so directory-depth changes do not affect discovery.

Environment variables:

- KIMI_CODE_HOME overrides home (default ~/.kimi-code).
- KIMI_CODE_SESSIONS directly sets session roots, separated by semicolons; overrides KIMI_CODE_HOME.
- KIMI_WORK_SESSIONS sets kimi work session roots, separated by semicolons.

<a id="新增一个工具"></a>

## Add a tool

Create a collectors/ file and register a collect(ctx) generator with @collector("tool-name");
the framework discovers/schedules it. See collectors/__init__.py's docstring.
input must be uncached input tokens; request_id must be stable/unique for idempotent rescans.

<a id="调试"></a>

## Debugging

```bash
python -c "import collect; print(collect.run({'input': {'refresh': True}})['artifact']['sources'])"
```
