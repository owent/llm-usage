# synthetic-step-finish 期望（全合成）

<a id="synthetic-step-finish-expectations-all-synthetic"></a>

场景：两个会话（主 + parent_id 子会话）× 两条 assistant 消息 × 3 条
step-finish 部件 + 1 条 text 部件（不含 usage 的记录，过滤不计）。
message.data.tokens 是 turn 级聚合（Σ 该消息部件），**不读不双计**。
session.tokens_* 五列按 projector applyUsage 语义 = Σ 当前部件（对账用）。

期望（人工核算）：

- `usage_events` 3 条（model_call，2026-06-13 日汇总 call_count=3）：
  - `opencode:part:part_syn_1`（primary，model_raw=claude-sonnet-4-6，
    provider=anthropic，time_basis=observed_at，
    source_revision=1781337600500）：
    input_uncached=1000、input_total=9500（1000+8000+500 派生）、
    output_total=250（200+50）、output_reasoning=50、cache_read=8000、
    cache_write=500、total_tokens=9750、source_total=9750（一致无诊断）、
    cost=12000 micro-USD（estimated）。
  - `opencode:part:part_syn_2`（primary，同模型）：
    input_total=9500（500+9000+0）、output_total=80（80+0）、
    total_tokens=9580（派生，无 total 字段 ⇒ source_total=NULL）。
  - `opencode:part:part_syn_3`（sub_agent，session.parent_id 非空；
    model_raw=gpt-5.2、provider=openai）：
    input_total=1300（200+1000+100）、output_total=50（40+10）、
    total_tokens=1350、source_total=1350（一致）。
- text 部件（part_syn_text）不产生事件、不计 records。
- 日汇总（2026-06-13，UTC）：call_count=3、input_total=20300、
  cache_read=18000、cache_write=600、output=380、total_tokens=20680。
- 对账 reconciliations：2 条均 matched——
  - ses_syn_main：部件五字段合计 9750+9580=19330 = 五列合计
    1500+280+50+17000+500=19330；
  - ses_syn_sub：1350 = 200+40+10+1000+100=1350。
- 诊断恰 1 条 latest_fallback（注册表为空，仅按文档或源码实现）。
- 再次扫描没有重复入库：call_count 仍 3（同键同修订 unchanged）。
