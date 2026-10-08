# Validation record template

<a id="验证记录模板"></a>

Copy this template to `<stage>-<topic>.md`, for example `m0-windows-baseline.md`.
Fill every field from actual observations. Keep unexecuted items marked **Not run** with
the reason; do not remove them or prefill successful results.

<a id="元信息"></a>

## Metadata

| Item | Content |
| --- | --- |
| Date | YYYY-MM-DD |
| Environment | OS version/architecture, CPU, memory and runtime versions (Node/Rust/WebView, etc.). |
| Code revision | Commit or working-tree status summary. |
| Governing design specifications | Relevant design sections and V identifiers. |

<a id="命令与结果"></a>

## Commands and results

For each command, record cwd, full command, exit code, duration and key output summary.

| # | Command (cwd) | Exit code | Result summary |
| --- | --- | --- | --- |
| 1 | | | |

<a id="锁定版本"></a>

## Locked versions

| Dependency | Locked version | Actual resolution (lockfile value) | License | Source |
| --- | --- | --- | --- | --- |

<a id="测量与测试"></a>

## Measurements and tests

| Metric/case | Target | Observed | Method/sample | Conclusion |
| --- | --- | --- | --- | --- |

<a id="失败与未执行项"></a>

## Failures and unexecuted items

| Item | Status | Reason | Follow-up condition |
| --- | --- | --- | --- |

<a id="证据文件"></a>

<a id="验证产物"></a>

## Validation artifacts

List artifact paths (test data, build artifacts, logs), storage location and redaction method.
