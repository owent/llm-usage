# synthetic-no-usage-messages._expectations.md (synthetic)

<a id="synthetic-no-usage-messages_expectationsmdsynthetic"></a>

**All files in this directory are synthetic test data, not extracts from real sessions.**
Test tool/user messages with no usage. These records produce no model calls
or request counts (V03).

<a id="场景与期望"></a>

## Scenario and expectations

- Nine lines: session_meta, turn_context, task_started, response_item/message(user),
  response_item/reasoning, response_item/custom_tool_call, response_item/custom_tool_call_output,
  event_msg/user_message and task_complete.
- No token_usage_record or token_count.
- Model calls=0, events=0, scan status=complete, reconciliation=no_snapshot.
  turns started/completed=1/1 appears only in parsing context and produces no event.
