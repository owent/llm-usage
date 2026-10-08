# M7 (partial): release build and package measurements

<a id="m7部分release-构建与包体测量"></a>

<a id="m7部分release-构建与包体证据"></a>

Full M7 installation/resource/scheduling/three-platform acceptance (V19–V27) had not
started at this stage. This record covers the Windows release build and measured
package sizes, plus WSL Linux compilation/tests (part of V27). Per user instructions,
macOS had no local acceptance and was reserved for subsequent CI testing.

<a id="元信息"></a>

## Metadata

| Item | Value |
| --- | --- |
| Date | 2026-09-25 |
| Environment | Windows 11 x64; rustc 1.98.0; Node v24.21.0; WSL2 Ubuntu rustc 1.98.1 |
| Code revision | Uncommitted working tree, after the M3 documentation-based batch |

<a id="命令与结果"></a>

## Commands and results

| # | Command (cwd) | Exit code | Result |
| --- | --- | --- | --- |
| 1 | `npm run build:desktop` (repository root) | 0 | Release build and NSIS packaging succeeded |
| 2 | WSL `cargo check -p llm-usage-core / -p llm-usage-m0` | 0 | Core and application compiled on Linux |
| 3 | WSL `cargo test -p llm-usage-core` | 0 | Linux tests passed |

<a id="包体实测对照-architecturemd-资源目标"></a>

## Package measurements (architecture.md resource targets)

| Item | Measured | Target |
| --- | --- | --- |
| NSIS installer | Initial 2,343,869 B ≈2.24 MiB; renamed/rebuilt LLMUsage_0.1.0_x64-setup.exe had comparable size | ≤20 MiB ✓ |
| Main executable | 5,808,128 B ≈5.54 MiB | Installed directory ≤60 MiB; component measurements pending |
| Frontend gzip | 220.32 kB | ≤1 MiB ✓ |

The M0 shell package was superseded by the implementation containing 17 adapters,
queries, scheduling, export and i18n. Installer growth meets the target when reusing
WebView2 Evergreen. Per user request, executable and installer were renamed LLMUsage
from 2026-09-26; old llm-usage-m0 artifacts were deleted. Idle memory (≤180 MiB) and
first-screen/query percentiles (V20) were not measured and remained for formal M7 acceptance.

<a id="未完成项m7-剩余"></a>

## Unfinished M7 items at this stage

Actual installation/upgrade/uninstall and restart recovery (V19), all-process resources
(V21), offline operation, Windows system-task comparison (V24), three-platform CI
artifacts (V26; first run to be recorded after pushing), WSL packaging/WSLg (V27;
compilation already verified), and support-matrix/README synchronization.
