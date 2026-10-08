# synthetic-step-finish 期望（全合成）

<a id="synthetic-step-finish-expectations-all-synthetic"></a>

场景：两个会话（主 + parent_id 子会话）× 3 条 step-finish 部件 + 1 条 text
部件（过滤）。MiMo session 表无 tokens_* 累计列（固定源码 session.sql.ts）
⇒ 无会话级对账；message.agent_id 仅为格式识别字段，不参与分类。

期望（人工核算）：

- `usage_events` 3 条（model_call，2026-06-13 日汇总 call_count=3）：
  - `mimo-code:part:part_syn_1`（primary，model_raw=mimo-latest、
    provider=mimo，time_basis=observed_at）：
    input_uncached=1000、input_total=9500（1000+8000+500 派生）、
    output_total=250（200+50）、output_reasoning=50、cache_read=8000、
    cache_write=500、total_tokens=9750、source_total=9750（一致）、
    cost=12000 micro-USD（estimated）。
  - `mimo-code:part:part_syn_2`：input_total=9500、output_total=80、
    total_tokens=9580（派生，无 total 字段 ⇒ source_total=NULL）。
  - `mimo-code:part:part_syn_3`（sub_agent，parent_id 非空）：
    input_total=1300、output_total=50、total_tokens=1350、source_total=1350。
- 日汇总（2026-06-13，UTC）：call_count=3、input_total=20300、
  cache_read=18000、cache_write=600、output=380、total_tokens=20680。
- reconciliations 为空（无累计列对账目标）。
- 诊断恰 1 条 latest_fallback（注册表为空，仅按文档或源码实现）。
- 再次扫描没有重复入库：call_count 仍 3。
