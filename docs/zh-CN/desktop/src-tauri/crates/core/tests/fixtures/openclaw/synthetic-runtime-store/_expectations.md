# synthetic-runtime-store 期望（全合成）

<a id="synthetic-runtime-store-expectations-synthetic"></a>

场景：`agents/main/agent/openclaw-agent.sqlite`（占位表 synthetic_table_rows /
synthetic_table_events——文档未给出真实表名）。

期望（人工核算）：

- 发现：默认根 `<home>/.openclaw` 下定位到该文件，实例根为
  `agents/main`（每 Agent 一个实例）。
- 探测：Agent 身份由文档路径形状确认，但表级 schema 缺少文档说明 ⇒
  `unknown_format` fail closed（诊断 reason 说明"docs do not name any
  table/column; pending a real sample"），库结构可读（2 张用户表计入 reason）。
- `usage_events` / `source_aggregates` 0 条：不读表、不猜字段、不产零值。
- 跨轮重扫不重复入库：状态与计数稳定，无数据产出。
