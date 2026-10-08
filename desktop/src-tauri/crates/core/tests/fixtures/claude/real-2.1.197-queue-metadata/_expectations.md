# real-2.1.197-queue-metadata._expectations.md (anonymized native sample)

<a id="real-21197-queue-metadata_expectationsmdreal-脱敏提取"></a>

**Source: a real Claude Code 2.1.197 transcript, captured on 2026-09-30 in WSL Debian
by the first unauthenticated run of `claude -p hi`.** IDs and timestamps retain their
original shapes but have anonymized replacements; message bodies are placeholders.
Record types and fields match the native file: eight lines, comprising two queue-operation,
one user, three attachment, one assistant (`<synthetic>`, all-zero usage), and one last-prompt.

<a id="本样本固化的真实格式事实21197"></a>

## Native format observations for 2.1.197

1. A file can begin with `queue-operation` (enqueue/dequeue, with sessionId/content).
   The previous parser rejected this as an unknown format.
2. `attachment` (context attachment metadata) and `last-prompt` (session pointer)
   are records without model usage.
3. Without authentication, the placeholder assistant has `message.model == "<synthetic>"`
   and all-zero `usage`, including the server_tool_use/cache_creation extension keys.
   It represents no model call.
4. User records contain `promptSource: "sdk"`, entrypoint and permissionMode.

<a id="期望人工核算"></a>

## Manually calculated expectations

- detect: Supported; the initial queue-operation type is permitted.
- Scan: complete, lines_read=8, records_seen=8, **events=0**.
- The placeholder assistant produces synthetic_assistant_skipped and no event;
  zero usage does not become a model call.
- Skip queue-operation/attachment/last-prompt. Reject the whole file if any contains
  usage, using usage_on_unexpected_record_type.
- Rescanning adds nothing.
