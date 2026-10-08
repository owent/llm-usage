# synthetic-legacy-archive 期望（全合成）

<a id="synthetic-legacy-archive-expectations-synthetic"></a>

场景：`agents/main/sessions/` 旧 JSONL 归档（条目含文档提及的规范化
usage 形状占位：input_tokens/output_tokens 与 prompt_tokens/completion_tokens
别名、cacheRead）+ `sessions/sessions.json` 旧会话行迁移输入。

期望（人工核算）：

- 发现：两个文件都定位到（实例根 `agents/main`）。
- 探测：JSONL 归档按**迁移/离线维护输入降级处理**（官方文档：Gateway
  启动不导入，须经 `openclaw doctor --fix` 迁移；entry 级 schema 未文档化）
  ⇒ `unknown_format` fail closed，reason 标注待核验；
  sessions.json 同为迁移输入 ⇒ `unknown_format`，reason 指向 doctor 迁移。
- `usage_events` / `source_aggregates` 0 条：合成的 1200+300/60+15 等
  数值**不入账**（记录 schema 待核验，不猜字段、不产零值）。
- 跨轮重扫不重复入库。
