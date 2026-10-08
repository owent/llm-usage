# Desktop validation records

<a id="桌面客户端验证记录"></a>

[Current acceptance](current-acceptance.md) lists tested versions, platform results and
source records. [Plan.md](../../../Plan.md) maintains unfinished tasks and conditions;
[V01–V31](../../design/desktop-usage/validation.md) retains acceptance criteria.
This directory preserves execution records from each phase. Pending items in historical
records do not describe current status.

| Record category | Entry point |
| --- | --- |
| This batch's three-platform CI and downloadable artifacts | [CI jobs, tested commits, package digests and first failures](ci-plan-validation.md) |
| Windows/Linux installation and actual GUI | [NSIS/deb/AppImage/FUSE lifecycle](installation-lifecycle.md), [Orca in ten languages](orca-multilang.md) |
| System credentials, native interaction and scale | [Cross-platform credentials](platform-auth-continuation.md), [native/cancellation/incremental tests](plan-finalization.md), [queries and resources](plan-execution.md) |
| Native sources and field corrections | [Current source index](current-acceptance.md#来源与功能记录), [Qwen/OpenCode](container-sources.md), [ten M8 sources](m8-container-samples.md), [MiMo/Zoo/DSH](m3-container-samples.md) |
| Design review and resources | [Initial review](review-2026-09-27.md), [M0/M1 review](m0-m1-review.md), [static assets](static-assets.md) |

Record rules: [delivery requirements](../../design/desktop-usage/execution.md).
Template: [TEMPLATE.md](TEMPLATE.md).

- Mark a check passed only after running it and recording its result. Preserve cwd,
  command, versions, environment, exit code, counts, first failure, recovery and unexecuted items.
- Report static checks, synthetic data, native sources, simulated browser IPC, native
  desktop, WSL/containers and CI separately. One result does not verify another category.
- Keep private originals, comparison databases, extraction scripts and logs only in
  root build/. Anonymize public records and test data; exclude bodies, credentials and private paths.
- Put completed details in dedicated records. Active plans retain stable IDs, current
  status, remaining conditions and links, without repeating accumulated counts or each run's history.
