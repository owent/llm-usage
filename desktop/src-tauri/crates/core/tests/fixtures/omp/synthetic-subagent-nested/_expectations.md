# synthetic-subagent-nested._expectations.md（SYNTHETIC）

**本目录全部为合成样本（synthetic），不是真实会话提取。** 仿本机真实布局：
子 Agent 文件在 `<cwd>/<ts>_<父UUID>/<Name>.jsonl`，嵌套子 Agent 再深一层
`<Name>/<Name>.<sub>.jsonl`；父子关联来自最近的 `<ts>_<uuid>` 祖先目录名
（首个下划线后的父 UUID），隔代不归名。

## 场景与期望

- 3 文件（同一配置根，一次发现）：
  - 主会话 `…d070.jsonl`（文件名是 `<ts>_<uuid>` 形状）⇒ primary，
    parent_session_id=None；assistant syn-ma-1（10/5/0/0/15）。
  - 两层子 Agent `…d070/Research.jsonl` ⇒ sub_agent，
    parent_session_id=00000000-0000-7000-8000-00000000d070（目录名推定）；
    自有 session 头 id=syn-omp-sub1；assistant syn-sa-1（100/50/0/0/150）。
  - 三层嵌套 `…d070/Research/Research.Compactor.jsonl` ⇒ sub_agent，
    parent_session_id 同上（隔代不归名，归最近的 `<ts>_<uuid>` 祖先）；
    自有 session 头 id=syn-omp-sub2；assistant syn-sa-2（200/60/40/0/300）。
- 整轮：3 文件 complete、3 事件、added=3；类别计数 primary=1、sub_agent=2。
- 汇总（2026-01-05 UTC）：call_count=3、input_total_known=350（派生口径
  (10+0+0)+(100+0+0)+(200+40+0)）、cache_read_known=40、output_total_known=115、
  total_tokens_known=465；distinct session=3（各文件自有会话头）。
