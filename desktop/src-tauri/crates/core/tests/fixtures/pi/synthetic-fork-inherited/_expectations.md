# synthetic-fork-inherited._expectations.md（SYNTHETIC）

**本目录全部为合成样本（synthetic），不是真实会话提取。** 覆盖 fork 继承条目的
跨文件幂等去重。结构仿 pi session JSONL v3（固定源码 session-manager.ts：fork 把
源文件全部非头条目**逐字复制**进新文件，id/parentId/timestamp 不变，新头记 parentSession）。

## 场景与期望

- 源文件 `..._syn-sess-src.jsonl`（4 行）：session 头（id=syn-sess-src）、model_change、
  assistant syn-fa-1（usage 100/50/0/0/150）、assistant syn-fa-2（200/60/40/0/300）。
- fork 文件 `..._syn-sess-fork.jsonl`（5 行）：session 头（id=syn-sess-fork，
  parentSession=syn-sess-src），其后逐字复制源文件 L2–L4，再追加新 assistant
  syn-fa-3（10/5/0/0/15，timestamp 2026-01-05T11:00:01.000Z）。

## 期望

- 两轮扫描（先只放源文件，再补 fork 文件）后总调用数 = 3（syn-fa-1、syn-fa-2、syn-fa-3），
  复制件不双计。
- 汇总（2026-01-05）：input_total_known=350（派生 input+cacheRead+cacheWrite：
  100+240+10）、cache_read_known=40、output_total_known=115（50+60+5）、
  total_tokens_known=465（150+300+15）。
- fork 新条目 syn-fa-3：session_id=syn-sess-fork、parent_session_id=syn-sess-src。
- 事件键四元组：`pi:message:<id>:<parentId>:<timestamp>`（parentId 为空用 "-" 占位）。

## 已知偏差（实测核验）

复制条目在 fork 文件中会带上 fork 会话的 session_id/parent_session_id（解析上下文来自
本文件 session 头），与源文件已存事件同键不同内容，仲裁为 **conflict（保留先扫者）**
而非 capability 声明的「同键同内容 Keep」。净效果仍是幂等（不双计、先扫者保留），
但 outcome.conflicts=2 且两条已存事件 conflict=1。详见 gaps 测试注释与交付报告。
