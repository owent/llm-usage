---
title: Collection and background tasks
description: Manual refresh, automatic schedules, pauses and Windows task verification.
sidebar:
  order: 4
---

## Manual and automatic collection

Manual refresh collects all enabled sources. Automatic collection uses a global interval
of one hour by default, with zero meaning disabled. Each source can inherit that interval
or use its own interval, daily time or weekly rule. A schedule reads existing local records;
it never launches an agent, prompt or model request.

Fixed-time rules save their own timezone. Changing the statistics timezone does not silently
move an existing schedule. A nonexistent daylight-saving time moves to the first valid
instant after the transition; a repeated time runs at its first occurrence only. Missed
deadlines are merged rather than replayed as invented usage.

Two source instances can be read concurrently while a single writer commits events,
cursors and aggregates. Pause and cancel preserve confirmed progress. Interrupted or
unvisited sources remain due; a read window does not establish that the complete scan finished.

## When the application is closed

| State | Collection behavior |
| --- | --- |
| Window open | Enabled automatic schedules can run; manual refresh is available. |
| Window closed to tray | The process can continue according to the saved settings. Closing to tray is optional. |
| Process exited | In-process collection stops. An explicitly enabled Windows background task can launch headless collection. |
| Automatic interval zero | Automatic collection is disabled, including residual system triggers; manual refresh still works. |
| Sleep or logout | Do not assume execution. OS lifecycle and task behavior must be checked for the actual environment. |

Battery-saver pausing is enabled by default. Inspect the pause reason before interpreting
stale collection times as a parser failure. File watching is optional, applies to eligible
JSON/JSONL interval sources and retains polling as a fallback.

## Windows startup and background tasks

Both features are off by default. Enable them in Settings and inspect **requested** versus
**effective** state. The minute task checks saved due rules; the mere existence of a task
does not establish that its definition, executable or last invocation is correct.

The application manages only task/startup definitions whose full ownership matches the
current executable, data directory and user. It does not delete unrelated tasks with a
similar name. Read the [scheduling rules](/reference/design/scheduling/) for recovery,
locking and lifecycle details.

For an explicitly requested manual command:

```powershell
LLMUsage.exe --scan-once --data-dir C:\UsageData
```

`--headless` respects saved background intent and due rules; `--scan-once` is manual and
reads all enabled sources. Neither mode creates a WebView or telemetry receiver.
