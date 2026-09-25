# synthetic-fork-inherited._expectations.md（SYNTHETIC）

**本目录全部为合成样本（synthetic），不是真实会话提取。** fork 逐字复制条目
（type/id/parentId/timestamp 四元组不变）到 fork 会话文件；继承不是新调用。
本机 58 文件未观测到 fork（无 parentSession），此场景按 pi 同口径合成。

## 场景与期望

- 源文件（…d060.jsonl）：5 行，session id=syn-omp-src，assistant syn-fa-1
  （100/50/0/0/150）+ syn-fa-2（200/60/40/0/300）⇒ 2 事件。
- fork 文件（…d061.jsonl）：6 行，session id=syn-omp-fork、
  parentSession=syn-omp-src；逐字复制源 L3–L5（model_change + 2 assistant），
  追加新 assistant syn-fa-3（10/5/0/0/15）⇒ 3 事件。
- 两轮扫描（先源后 fork）：第一轮 added=2；第二轮源文件 unchanged 短路，
  fork 文件 events=3。

## 已知偏差（与 pi 同一定案）

复制条目四元组与源文件逐字相同，但事件 session_id/parent_session_id 取自
fork 会话头，与已存事件同键不同内容 ⇒ 仲裁为 conflict（保留先扫者），
而非「同键同内容 Keep」。幂等净效果成立：不双计、先扫的源会话归属保留。
第二轮 outcome：added=1（syn-fa-3）、conflicts=2、unchanged=0；
update_conflict 诊断 ×2。

- syn-fa-3 携带 fork 会话身份：session_id=syn-omp-fork、
  parent_session_id=syn-omp-src。
- 汇总（2026-01-05 UTC）：call_count=3、input_total_known=350（派生口径
  (100+0+0)+(200+40+0)+(10+0+0)）、cache_read_known=40、cache_write_known=0、
  output_total_known=115、total_tokens_known=465。
