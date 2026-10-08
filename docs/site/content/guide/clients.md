---
title: Client support and verification
description: Find the verified local formats, client versions and validation scope.
sidebar:
  order: 3
---

The [adapter matrix](/reference/design/adapters/) is the complete support reference. It
records coverage IDs, product versions, local files/databases, field semantics, parser status
and verification sources/results. Use it for an exact client/version decision rather than treating a shared
engine or model name as proof of compatibility.

## Common families

| Client family | Local collection route and important boundary |
| --- | --- |
| Codex | Native rollout JSONL and verified usage records; bounded backfill preserves cursors and rotates file visits. |
| Claude Code | Native messages and record-specific versions; repeated content blocks deduplicate by message identity. |
| OpenCode and derivatives | Product-specific records and schemas. A shared kernel does not establish another product's mapping. |
| Gemini, Qwen, Pi and Oh My Pi | Dedicated adapters with per-version rules; missing/default zero values are not blindly treated as reported usage. |
| Cline and Roo | Independently verified VS Code files/databases. Cline SDK metrics can be observations covering multiple requests. |
| Zed and Junie | See the tested local-provider and native-file records; hosted and untested provider routes are separate. |
| Local OTel exports | Only verified senders, file/database formats and local attribution qualify. The receiver is not a general-purpose collector. |
| Other registered clients | Consult the complete matrix, including document-level implementations, probes and excluded routes. |

## GitHub Copilot clients

- **CLI:** older `assistant_usage_events` and newer chronicle formats have different
  capabilities; unavailable per-request token fields are not reconstructed.
- **VS Code:** native `chatSessions/*.jsonl` is read by `copilot_chat`; verified local
  OTel file output can replace eligible native contributions without double counting.
- **Visual Studio:** `vs_copilot` reads verified `%TEMP%`/`%TMP%` trace files under
  `VSGitHubCopilotLogs/traces`. These temporary files do not promise complete history.
- **Account quota:** local `copilot-user-cache.json` provides separate quota snapshots.
  Premium requests and credits are not converted into tokens.

Visual Studio discovery does not filter Community, Professional or Enterprise, or guess
installation paths. The supplied read-only inspection tool uses Microsoft's `vswhere`
with all products, versions and prerelease instances. The verified VS 18 exporter is
different from the investigated VS 2022 and older extension artifacts. A version lacking
that exporter is not fixed by adding guessed year/SKU directory names.

```powershell
pwsh -NoProfile -File desktop/scripts/inspect-vs-copilot.ps1
```

Read the [cross-version Visual Studio analysis](/reference/evidence/m9-vs-copilot-discovery/)
and [Copilot review](/reference/evidence/m9-copilot-review/) before interpreting an empty result.

## Validation scope

Documented fields, static upstream code, synthetic test data, real native files/databases,
browser IPC mocks, native desktop tests, installations and CI builds answer different
questions. “Not verified” does not mean “unsupported.” An empty session or installed
binary does not establish real token capture. Current results and historical limits are
kept in [acceptance records](/reference/evidence/current-acceptance/).
