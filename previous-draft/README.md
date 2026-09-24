# LLM 用量看板（独立版）

从 Kimi Work 的「LLM 用量看板」迁移而来，可在本机独立运行：

- `collect.py` / `store.py` / `collectors/` — 用量采集与统计（SQLite 缓存在 `data/usage.db`）
- `dashboard/` — 静态看板（ECharts），读取 `dashboard/data.json`
- `data/usage.db` — 迁移时的历史数据快照（保留 366 天滚动窗口）

## 快速开始

```bash
cd /d D:/tmp/llm-usage

# 1. 增量采集并生成看板数据（首次会自动回填各数据源历史）
python collect.py

# 2. 打开看板
start dashboard/index.html        # Windows；或直接双击 index.html
```

看板顶部的「刷新今日数据」按钮在独立模式下会重新读取 `data.json`；
重新执行 `python collect.py` 即可更新数据（文件协议下页面需刷新）。

## 常用参数

```
python collect.py --refresh           # 忽略增量状态，全量重扫
python collect.py --tool kimi-code    # 只运行指定采集器（可多次传入）
python collect.py --out 其它路径.json  # 自定义快照输出路径
```

## 支持的数据源

| 工具 | 数据源 |
|---|---|
| oh-my-pi | `~/.omp/agent/sessions/*/*.jsonl`、`~/.omp/logs/*.log` |
| kilo code | `~/.local/share/kilo/kilo.db` |
| codex | `~/.codex/sessions/**/rollout-*.jsonl` |
| zcode | `~/.zcode/cli/rollout/model-io-*.jsonl` |
| copilot | `~/.copilot/session-store.db`、`~/.copilot/session-state/*/session.db` |
| kimi work | kimi-code 内核 `…/kimi-code/home/sessions/**/wire.jsonl`（`usage.record`） |
| **kimi code** | **新版（v2.x）`~/.kimi-code/sessions/**/wire.jsonl`（`usage.record`）** |

### 新版 Kimi Code（kimi-code 采集器）

新版 Kimi Code（桌面端 2.x）的 home 目录为 `~/.kimi-code`，会话事件仍写在
`sessions/**/agents/<agentId>/wire.jsonl`，采集器递归扫描整棵 sessions 树，
层级变化不影响采集。

环境变量：

- `KIMI_CODE_HOME`：覆盖 home 目录（默认 `~/.kimi-code`）
- `KIMI_CODE_SESSIONS`：直接指定 sessions 根路径（多个用 `;` 分隔），设置后忽略 `KIMI_CODE_HOME`
- `KIMI_WORK_SESSIONS`：kimi work 采集器的会话根路径（多个用 `;` 分隔）

## 新增一个工具

在 `collectors/` 下新建文件，用 `@collector("tool-name")` 注册 `collect(ctx)` 生成器即可，
框架自动发现并调度；详见 `collectors/__init__.py`  docstring。

要点：`input` 必须是未命中缓存的输入 token；`request_id` 稳定唯一（重复扫描幂等去重）。

## 调试

```
python -c "import collect; print(collect.run({'input': {'refresh': True}})['artifact']['sources'])"
```
