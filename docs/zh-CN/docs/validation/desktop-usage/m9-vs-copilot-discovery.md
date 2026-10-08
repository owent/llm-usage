# Visual Studio Copilot 版本与目录发现修复

<a id="visual-studio-copilot-version-and-directory-discovery-correction"></a>

2026-10-07，Windows 11 x64、PowerShell 7、llm-usage 0.2.1。
用户反馈 Enterprise 漏采；反馈机的 VS/Copilot 完整版本及文件状态尚未提供。
下述结果区分源码缺陷、官方组件核验和本机真实样本，不能据此断定反馈机的唯一原因。

<a id="confirmed-cause-and-correction"></a>

## 已确认原因与修复

旧适配器没有 Community、年份或安装目录过滤。它只从 `TEMP` 取一个根，
`TEMP` 非空时完全忽略 `TMP`，也不会检查启动器改写环境之前的用户默认临时目录。
已核验的 VS 18 `FileOutputResolver.DefaultBaseLogPath` 使用
`Path.Combine(Path.GetTempPath(), "VSGitHubCopilotLogs")`，
`InstrumentationServiceBuilder.BuildWithDefaultExporters` 写入
`traces/<短 telemetry session id>_VSGitHubCopilot_traces.jsonl`。
[.NET 说明](https://learn.microsoft.com/en-us/dotnet/api/system.io.path.gettemppath?view=net-10.0)
调用 Windows 临时目录 API；普通用户的
[Win32 取值顺序](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-gettemppathw)
是 `TMP`、`TEMP`、`USERPROFILE`。因此两变量不同时，旧发现路径可能与 VS writer 不同。

修复同时检查 `TMP`、`TEMP`、已观测的 `LOCALAPPDATA/Temp` 候选。
两临时变量均缺失时采用当前 `USERPROFILE`/传入用户目录回退；不检查其他用户或
Windows/SystemTemp。候选只从传入发现上下文取得，保持 `manual_roots_only` 隔离。
手工根增加临时目录父级；与日志目录、traces 目录、单文件使用相同物理根身份。
按规范化物理目录去重，改用框架有界单目录枚举；不改变解析、事件身份、费用或历史数据。

<a id="version-and-edition-references"></a>

## 跨版本和 SKU 依据

| 范围 | Copilot 安装依据 | 已核验文件/目录及边界 |
| --- | --- | --- |
| VS 2026 / 18.x，Community、Professional、Enterprise | 安装实例由 Setup Configuration 查询，遥测发现不按 SKU 筛选 | 本机 Community 的已安装 Core 组件 `18.10.1203+60f4a0a576` 有 `FileOutputResolver` 和 `JsonlOtlpTraceExporter`；上述临时目录 writer 无 SKU/年份分支。本机真实记录版本另为 `18.10.1197+4b9e241b86`、`18.10.12217.157`，不能用安装版本覆盖历史依据。Professional/Enterprise 没有本机原生样本 |
| VS 2022 / 17.10+ 内置 Copilot | [Microsoft 安装要求](https://learn.microsoft.com/en-us/visualstudio/ide/visual-studio-github-copilot-install-and-states?view=vs-2022) | 官方 Copilot VSIX `17.14.1713.63837` 的 Core/Service 没有 VS 18 的 Instrumentation/JSONL exporter。存在 `TokenTelemetryHelpers` 的 VS telemetry 上报，以及 `TokenCacheCounting.WriteTokenCountsToCsv` 的临时目录 `VSGitHubCopilotLogs/UsageDetails` writer 定义；在受查 Service IL 中没有该 CSV 方法调用。不能把定义当成默认写入文件；CSV 没有事件时间且缺字段默认零，不接入。不推广到全部 17.x，也不能据此核验其他本地文件不存在 |
| VS 2022 / 17.8–17.9 独立 Chat/Completions 扩展 | [Microsoft 发布说明](https://devblogs.microsoft.com/visualstudio/introducing-the-new-copilot-experience-in-visual-studio/)、[Chat Marketplace](https://marketplace.visualstudio.com/items?itemName=VisualStudioExptTeam.VSGitHubCopilot) | 实际官方 Chat VSIX `0.2.765.20217` 清单范围 `[17.8,17.10)`、Community 目标及 amd64/arm64；受查 Core/Shared/Vsix 类型枚举没有上述 JSONL exporter，持久会话使用 copilot-chat。独立分发不能由 VS 18 writer 核验，本轮没有原生使用样本 |
| 更早 VS 2022 Completions | [官方旧扩展页面](https://marketplace.visualstudio.com/items?itemName=GitHub.copilotvs) 保留 17.4.4–17.5.4 使用 1.84.0.1 的历史说明 | 属早期补全扩展，不是 17.10 起的内置 Chat；未核验逐次用量文件 |
| VS 2019 / 更早 | 当前官方 Copilot 文档要求 VS 2022；不将 vswhere 能枚举旧 IDE 当成 Copilot 支持依据 | 无已核验本地 Copilot token 格式，不能补零或拼接假想目录 |

VS 2022 VSIX 清单 `InstallationTarget=Microsoft.VisualStudio.Community`，范围
`[17.14,18.0)`、amd64/arm64。这不表示 Enterprise 被排除：
[官方 VSIX 安装说明](https://learn.microsoft.com/en-us/visualstudio/extensibility/vsix-extension-schema-2-0-reference?view=visualstudio)
说明低 SKU 目标适用于更高 SKU，Community 目标同样适用于 Professional/Enterprise。
这个规则证明安装适用范围，不能代替各版本真实用量验证。

两代受查 `CopilotSessionProvider` 都通过 `IVsWorkingFolders.GetFolder(1,false,true)`
取得解决方案工作目录，再由 `PersistedSessionFolderProvider` 追加
`copilot-chat/<账号散列>/sessions`。VS 2022 的 MessagePack repository 持久化
会话和 interaction；它不是安装目录中的用量文件。不能猜所有会话都在一个
`LOCALAPPDATA/Microsoft/VisualStudio/17.0_*` 根，也不全盘扫描项目。
本机 VS 18 历史实样的会话/日志边界见 [原生记录](m9-vs-copilot-local.md)。

<a id="finding-installation-directories-and-read-only-inspection"></a>

## 安装目录获取与只读核查

[Microsoft 定位说明](https://learn.microsoft.com/en-us/visualstudio/extensibility/locating-visual-studio?view=visualstudio)
明确 VS 2017 起不存在可核验所有实例的单一环境变量/注册表值，应使用
Setup Configuration API；`vswhere` 是官方原生客户端。用户可运行：

```powershell
pwsh -NoProfile -File desktop/scripts/inspect-vs-copilot.ps1
```

[脚本](../../../desktop/scripts/inspect-vs-copilot.ps1)先查 PATH，否则定位官方 Installer
工具位置；也接受 `-VsWherePath`。实例查询使用
`-all -prerelease -products * -utf8 -format json`，不使用 `-latest` 或年份过滤。
组件查询使用官方 `-find **\Microsoft.VisualStudio.Copilot.Core.dll`，从返回的实际
安装根归属组件，支持自定义位置及并存实例，不拼接 `2022/2026/18/<edition>`。
缺工具/查询失败显式报告；不自动安装工具。

脚本只读取 PE 元数据、组件版本及临时目录文件数量/大小/最新写入时间，不加载或执行
组件，不读取会话正文、账号、凭据或远端账单。exporter 类型存在只是静态检查结果，
不能核验当前 VS 已写出非空用量。临时候选同时报告当前 .NET 解析结果、进程和
用户层 TMP/TEMP、系统已知 LocalApplicationData/Temp；与采集程序环境不同的
自定义根可手工添加。目录不存在、没有 JSONL、目录不可读分别保留。

本机 `vswhere` 返回一个实例：`18.10.12224.181`，产品
`Microsoft.VisualStudio.Product.Community`，安装路径
`C:\Program Files\Microsoft Visual Studio\18\Community`。
这是当前机器的实测路径，不声明所有 VS 2026 默认安装目录相同。
没有安装或启动其他 SKU/VS 2022，没有发送 Copilot 请求。

<a id="official-packages-and-first-anomalies"></a>

## 官方包与首次异常

VS 17 release channel 指向 `17.14.37710.0` catalog，下载内容自报
`17.14.41 (September 2026)`。本次 `Invoke-WebRequest` 和 `curl.exe` 得到相同
catalog，但与 channel 声明 SHA-256/大小不符，原因未确认；不能据此核验整个 catalog 完整性。
实际 catalog SHA-256 为 `F0A50EA157222C29ABD5EA6FF01BFC3C33B04E011C5E45EE2CA38EF0778E5643`，
channel 声明 `6e470016e4324c84c255ffd0beb3767d17ec89cc8561e9409ee3e1f6d29400f5`。

随后单独下载 [Microsoft Copilot VSIX](https://download.visualstudio.microsoft.com/download/pr/bc92e2cb-33de-4a0c-995d-efa817f16b16/b1b9eb236af78de99b4ece7f40525eca0f704c3f498c15ffb46e9963301f5d9d/VisualStudio.GitHub.Copilot.vsix)，
实测 SHA-256 与其 payload 声明一致：
`B1B9EB236AF78DE99B4ECE7F40525ECA0F704C3F498C15FFB46E9963301F5D9D`。
包大小仍与 catalog 声明有差异，保留此异常，不报告整个分发链一致。
解出的 Service Core DLL ProductVersion 为 `17.14.1713-rc+f95dadab4e.RR`，
Windows Authenticode `Valid`，SHA-256
`FFFB9D471DAB8D08DCE49B4B969A665D794748CE3105A88DA8176E51190E174E`。
ILSpy 11.1.0.9782 仅安装到任务目录并读取代码，不执行 VSIX。
上面静态结论限定此实际包，不从 catalog 推广其他版本。

另从 Marketplace 官方 publisher 的 `latest/vspackage` 下载旧 Chat 扩展，清单版本
`0.2.765.20217`，包 SHA-256
`CC7BA4862AD8E7C1D7701803ABAB89EE861AB0606233E174A73FDB92781E1529`。
该哈希是本次下载身份记录，没有独立预期哈希，不称为完整性对照通过。
所查 Core/Shared/Vsix DLL ProductVersion 为 `0.2.765-beta+4ef9459926.RR`，
Windows Authenticode 均 `Valid`；Core 签名者为 Microsoft Corporation。
仅核对 manifest、元数据及 IL，不安装扩展。

<a id="validation-and-remaining-scope"></a>

## 验证与剩余范围

临时结果均在 `build/vs-copilot-discovery/`，不复制原始会话。

| 命令/核对 | 当前结果 |
| --- | --- |
| `cargo test --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core --test vs_copilot_discovery` | 退出 0，4 项通过：双临时根、旧游标/全注册表/重扫、默认用户临时根及去重、手工父级与隔离。旧源码首先复现默认用户根/父级漏发现；测试首版另有 checkpoint 列名及 run ID 复用错误，修正测试后验证，未把这些错误当作产品缺陷 |
| `node --test desktop/scripts/inspect-vs-copilot.test.mjs` | 退出 0，合成 vswhere 覆盖 Community/Professional/Enterprise、17/18、preview、自定义安装路径；不能据此核验真实 SKU 数据格式 |
| `desktop/scripts/inspect-vs-copilot.ps1` | 退出 0，本机全实例查询及 PE 元数据读取成功，发现 1 个 traces JSONL，无警告 |
| `cargo run ... --example real_verify_vs_copilot -- <本机 traces> <独立核对目录>` | 退出 0，4 次调用，input=77,418/output=356/cache_read=57,297，诊断 0；独立 Python 仅按允许字段逐 span 求和一致，重复扫描新增 0。按记录保留两个实际 service.version；其他 token 桶未知 |
| `npm run verify` | 退出 0：Rust 1,032 通过、8 条件项忽略；前端 22、脚本 5；类型、fmt/clippy、资源和前端构建通过。结束后新增的文档另跑 Markdown 检查 |
| `npm run build:desktop` | 退出 0，Windows x64 release 可执行文件及 NSIS 安装包构建成功；不等于安装/GUI/发行验收 |
| `npm run test:headless` | 退出 0，真实可执行文件/SQLite，隔离合成来源 11 项通过 |
| 新 release `--scan-once` 双临时根专项 | 退出 0，隔离合成 TMP/TEMP 各一个非空 VS trace，真实默认发现得到 2 来源/2 调用、input=200/output=20；两轮结果一致、无解析诊断、无其他来源。首版核对脚本误把正常 `scan_completed` 活动记录当作解析诊断，明确分类后通过，未修改产品行为 |
| `npm run lint:md`、`git diff --check` | 文档 210 文件、0 问题；差异检查退出 0 |

没有反馈机日志或原生 Enterprise/Professional/VS 2022 调用样本，不能声称反馈机已恢复。
若反馈机有合法 traces，修复后的发现规则可直接读取或手工指定该目录；若该组件没有
可核验本地逐次用量文件，安装路径、对话数量、premium 额度及模型上下文上限都不能替代 token。
其他文件/更老扩展保持待核验，不访问远端 API、不伪造默认零。
