# synthetic-no-usage-messages._expectations.md（SYNTHETIC）

<a id="synthetic-no-usage-messages_expectationsmd-synthetic"></a>

**本目录全部为合成样本（synthetic），不是真实会话提取。** 覆盖无 usage 的
tool/user 消息：这些记录不产生模型调用、不产生 request 数（V03）。

<a id="scenario-and-expectations"></a>

## 场景与期望

- 9 行：session_meta、turn_context、task_started、response_item/message(user)、
  response_item/reasoning、response_item/custom_tool_call、
  response_item/custom_tool_call_output、event_msg/user_message、task_complete。
- 无任何 token_usage_record / token_count。
- 期望：模型调用数 = 0、事件数 = 0；扫描状态 complete；对账 no_snapshot；
  turns started/completed = 1/1（仅出现在解析上下文，不产生事件）。
