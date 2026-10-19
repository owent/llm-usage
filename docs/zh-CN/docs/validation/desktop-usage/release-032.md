# 0.3.2 兼容读取与遥测文案核验

<a id="version-032-compatibility-and-telemetry-wording-checks"></a>

范围：根据逐版本原生样本修复 Codex/Kilo 兼容提示，改进本地 OTel 说明，
并将应用清单版本更新为 0.3.2。下述本地检查在打 tag 和运行远端 CI 前完成；
后续 tag 构建结果单独记录。原生来源只读提取遵循
[脱敏要求](../../design/desktop-usage/implementation-readiness.md)。

## 兼容读取提示

本机应用数据库中有 66 个 Codex 文件和一个 Kilo 数据库标为 `active_compat`。
提示表示解析器能够识别格式，但记录所属的客户端版本尚未核验，与记录错误分开。
Codex 注册表缺少 0.159.0-alpha.12.1、0.160.0、0.160.1 和 0.162.0-alpha.17.2。
Kilo 有 27 个未登记的会话版本，包含 7.8.3 和 7.8.7。此前 7.8.3 只有空会话，
不能核验用量格式；此次原生会话已有真实调用。

每个新增版本均有独立提取的原生样本和计算的期望值，位于 core crate 的
`tests/fixtures/{codex,kilo}/releases-032/`：

| Codex 版本 | 调用 | 含缓存输入 | 缓存读 | 含推理输出 | 推理 | 总量 |
| --- | --- | --- | --- | --- | --- | --- |
| 0.159.0-alpha.12.1 | 3 | 81,454 | 58,368 | 756 | 71 | 82,210 |
| 0.160.0 | 1 | 22,600 | 4,864 | 203 | 140 | 22,803 |
| 0.160.1 | 1 | 35,556 | 17,152 | 138 | 62 | 35,694 |
| 0.162.0-alpha.17.2 | 14 | 392,480 | 350,720 | 1,443 | 345 | 393,923 |

Kilo 的 27 个完整原生会话包含 787 次 assistant 调用。新增的精确版本为：
7.3.42、7.3.45、7.3.46、7.3.49、7.3.50、7.3.54、7.3.63；
7.4.1、7.4.2、7.4.5、7.4.7、7.4.11、7.4.13、7.4.15、7.4.16、7.4.17、
7.4.20、7.4.22、7.4.23；7.5.6、7.5.14；7.6.0、7.6.2；7.7.5、7.7.12；7.8.3、7.8.7。
逐版本期望值保留五个互斥桶、派生总量、缺失的原生总量及原生会话快照。
26 个快照一致；7.3.42 明细已有正用量，累计快照却为零。差异继续保留为审计诊断，
不会据此丢弃有效调用，也不会用快照核验版本。

样本进入 Git 前已移除或匿名化正文、路径和身份标识。新增 Kilo 样本将 Provider 标识
替换为常量，保留模型名、时间、token 数值和所属会话版本。样本仅核验原生 CLI/数据库
格式，不扩大 IDE 或遥测支持范围。

`compatibility_release_032.rs` 使用旧兼容解析依据生成真实的旧事件摘要并消费游标，
随后通过完整内置注册表重新发现来源。检查父目录发现、逐 token 字段、已核验版本依据、
统计和首次观测时间不变、诊断保留、无冲突及重复读取幂等。未知或缺失版本、混合数据库、
空会话及坏记录继续沿用已有检查。登记一个版本不会核验相邻版本。

真实旧数据库核验还发现了文件连续性缺陷：一个稳定的 63,974,896 字节 Codex 文件
反复停在第 33,554,261 字节。库中保存的创建时间指纹与文件当前创建时间不同。
重读保存了文件大小、修改时间和内容指纹，却遗漏新的创建时间，导致每次扫描都重置
文件代数。现已将创建时间与其他指纹、事件和游标在同一事务中保存。独立回归在修复前
只读到一次调用，期望为三次；修复后核对仅重置一次、重开数据库后继续前进、
读完三次调用且源文件字节不变。

### 创建时间指纹回归覆盖

[Issue #1](https://github.com/owent/llm-usage/issues/1) 指出的测试缺口成立：
文件系统未提供创建时间时，`codex_file_fairness.rs` 在任何断言之前就直接返回。
现已让测试始终执行。有原生创建时间时保留文件重建场景；缺少创建时间时，写入旧创建时间
和不匹配的文件头指纹以触发重扫。重开 SQLite 后检查完整的持久化指纹，包含清除过期创建时间，
以及文件代数恰好增加一次。随后检查三次调用/360 token、重复读取后的指纹和代数稳定，
以及源文件字节不变。产品代码和采集行为没有变化。

2026-10-19 的后续验证使用 Windows x64、Node 24.22.0、Rust/Cargo 1.99.0 和锁定依赖。
四项文件轮转测试在原生创建时间和临时将探测值改为 `created_ms = None` 的两种模式下均通过。
临时移除 `row.created_ms = probe.created_ms` 后，修复后的回归在两种模式下都失败
（退出 101）：持久化指纹仍保留过期创建时间。两份产品源文件均在最终检查前恢复。
缺少创建时间的运行属于模拟，不据此确认 Linux 文件系统或远端 CI 验收通过。
日志和独立临时数据库位于被忽略的 `build/issue-1/`。

最终检查均退出 0：

| 命令 | 结果 |
| --- | --- |
| `cargo test --manifest-path desktop/src-tauri/Cargo.toml --locked -p llm-usage-core --lib --test codex_file_fairness --test jsonl_v07 --test compatibility_release_032` | 282 项通过，无失败或忽略 |
| `cargo clippy --manifest-path desktop/src-tauri/Cargo.toml --locked -p llm-usage-core --test codex_file_fairness -- -D warnings` | 无警告 |
| `cargo fmt --manifest-path desktop/src-tauri/Cargo.toml --all --check` | 通过 |
| `npm run lint:md` | 无问题 |
| `npm run check:docs` | 检查了 207 对仓库文档、21 对指南及源注释；Astro 无错误或警告 |
| `npm run test:docs` | 39 项通过，无跳过 |
| `git diff --check` | 通过 |

首次 PowerShell 启动报 `CreateProcessAsUserW failed: 5`，后续检查改用 `cmd.exe`。
沙箱内 Node/Git 子进程启动也曾报 `EPERM`；通过平台允许的重试方式，原有注释审核和文档脚本
均已完成，无需修改脚本。这些进程启动失败与上述故意撤掉赋值后的预期测试失败分别记录。

## OTel 文案

简体中文说明更新为：

> 读取本机遥测文件。统计同一会话、同一天的 Copilot 用量时，优先使用已核验且匹配的遥测导出数据，避免与会话记录中的用量重复计算。

“原生贡献”描述的是统计时选用的用量；“原生提取”指提取动作，仍不能说明选用的数据。
新文案明确区分导出数据和会话记录中的用量，保留已核验、匹配及去重条件。已同步十种
界面语言和中英文遥测/客户端指南。来源选择仍要求原主机、用户、会话和本地日期相同，
原始记录仍保留，已停止改写的分区继续使用既有统计来源，其他未核验导出保持隔离。
采集和去重行为没有变化。

## 版本与验证

根目录和桌面端 npm 清单及锁文件、两个 Rust 包清单、对应 Cargo 锁文件项及 Tauri
配置均更新为 0.3.2。依赖版本没有变化；pnpm 锁文件不保存本地包版本，无需修改内容。

环境：Windows x64、Node 24.21.0、Rust/Cargo 1.99.0，使用锁定依赖。

| 命令/检查 | 结果 |
| --- | --- |
| `npm run verify` | 退出 0：Markdown/资源、13 个脚本测试、22 个界面测试、Svelte/fmt/Clippy 无错误、1,057 个 Rust 测试通过；八个平台测试忽略；前端构建通过 |
| `npm run test:browser` | 退出 0：Edge 交互、十种语言和主题 |
| 原生来源/旧应用数据库 | 退出 0：独立一致性备份；493 个 Codex 文件和一个 Kilo 数据库全部正常，兼容文件数为零；原有 10,888 条事件的 token 字段、发生时间和首次观测值保持不变；无冲突 |
| 原生重复读取 | 退出 0：无新增、更新或冲突，完整读取后的用量保持不变 |
| `npm run build:desktop` | 退出 0：Windows x64 0.3.2 可执行文件和 `LLMUsage_0.3.2_x64-setup.exe` |
| `npm run test:headless` | 退出 0：11 项真实可执行文件/SQLite 检查；隔离合成来源，三条事件/75 token |
| `npm run check:docs` | 退出 0：205 对仓库文档、21 对指南和源注释审核；Astro 无错误或警告 |
| `npm run test:docs` | 退出 0：39 个测试，包含新验证页的导航入口 |
| `npm run build:docs` | 退出 0：1,383 页、2,897 个文件；本地链接、搜索、语言目录和发布标记通过 |
| pnpm 冻结锁文件检查 | 使用已安装 pnpm 11.13.0，退出 0：`pnpm with current install --lockfile-only --frozen-lockfile --ignore-scripts --offline`；原锁文件内容不变 |

项目仍固定 pnpm 12.10.1。版本引导程序未能从配置的 registry 取得该可执行文件；
上述离线检查使用已安装版本，不据此确认固定版本引导程序下的全新安装。
版本引导新增的锁元数据已撤回，没有升级依赖。在这些本地检查完成时，尚未重跑
Windows 安装生命周期、原生 GUI/IPC、远端 CI 或版本发布；浏览器与 headless 检查分别记录。

首次回归复用了采集任务编号；下一次回归假设未变 JSONL 扫描总会生成入库结果。
现已为每次扫描分配独立任务编号，允许跳过未变文件，同时断言数据和修订保持不变。
能力表断言和未知版本合成输入已随精确注册表更新。这些属于验证设置问题。
真实来源核验在 `build/compat-032/` 中的独立一致性应用数据库备份上执行，
不修改原始 Agent 文件或应用数据库。
创建指纹修复前，读取反复耗尽同一个 32 MiB 窗口，完整读取断言失败。复现回归并修复后，
全新备份经过五轮采集完成，下一轮无重复计数。首次文档检查还拒绝了英文记录中
未放入代码块的中文界面原文，现已改为字面代码示例。导航数量由 224 更新为 225，
并明确断言新增验证页存在。

## Tag CI 与 Release

`v0.3.2` tag 指向 `12207ea1a6c710c67ef681a6adc44b3041dc9128`。
[CI 运行 38156835286](https://github.com/owent/llm-usage/actions/runs/38156835286)
已成功完成：Markdown、前端、三个原生 Rust 作业、六个平台便携包构建和 Draft Release，
共 12 个作业全部通过。发行作业需要等待这些构建完成；
它于亚洲/上海时区 2026-10-12 01:13 完成。

草稿 Release ID `409806776` 包含六种便携归档、Windows x64 NSIS 安装包和六份平台报告，
共 13 个附件。发行作业在上传后核验了大小和 SHA-256 摘要，并检查了 tag 对应的源码 revision。
`draft: true` 设置使它在公开发布前不显示在公开 Release 列表中。
创建草稿与公开发布分别记录；该 tag 的流水线已经成功完成。

后续检查期间，同一 Release 于亚洲/上海时区 2026-10-12 01:16 公开发布。
当前 API 状态为 `draft: false`，仍对应同一 tag 源码和 13 个附件，地址为
[v0.3.2](https://github.com/owent/llm-usage/releases/tag/v0.3.2)。
六份已下载平台报告与 API 摘要、tag revision 和版本一致，七个包的 API 大小/摘要与报告一致。
后续核验最初要求草稿状态，因状态已变化而失败；重新读取 API 确认公开发布后，
再次执行公开状态检查。这些核验没有修改 Release 元数据。
[Release 触发的文档作业](https://github.com/owent/llm-usage/actions/runs/38158980685)
及其[主分支刷新](https://github.com/owent/llm-usage/actions/runs/38158986481)
均在同一源码 revision 成功完成，不据此确认真实已安装客户端的升级已通过。

## Actions 版本引用

后续维护按用户 2026-10-12 的指令，将两个工作流改为上游数字版本 tag。
已通过官方 GitHub API 核对最新稳定发行和 tag 内容：

| Action | 工作流引用 | 已核验稳定发行 |
| --- | --- | --- |
| [actions/checkout](https://github.com/actions/checkout/releases) | `v7` | `v7.0.1` |
| [actions/setup-node](https://github.com/actions/setup-node/releases) | `v7` | `v7.1.0` |
| [actions/upload-artifact](https://github.com/actions/upload-artifact/releases) | `v7` | `v7.0.2` |
| [actions/download-artifact](https://github.com/actions/download-artifact/releases) | `v8` | `v8.0.2` |
| [actions/github-script](https://github.com/actions/github-script/releases) | `v9` | `v9.0.0` |
| [pnpm/action-setup](https://github.com/pnpm/action-setup/releases) | `v6.1.0` | `v6.1.0` |
| [dtolnay/rust-toolchain](https://github.com/dtolnay/rust-toolchain/releases) | `v1` | `v1` |
| [Swatinem/rust-cache](https://github.com/Swatinem/rust-cache/releases) | `v2` | `v2.9.2` |
| [xresloader/upload-to-github-release](https://github.com/xresloader/upload-to-github-release/releases) | `v1.6.2` | `v1.6.2` |

pnpm 的 `v6` tag 落后于 `v6.1.0`，上传 Action 没有 `v1` tag，因此这两项使用最新的完整数字版本。
Rust Action 的 `v1` 提交与原引用 revision 的文件内容一致。
已核对输入参数名称、工具链/target 设置和现有仅由 tag 触发的草稿发行规则。
根规则、执行指引、覆盖记录以及中英文平台/发行文档使用相同约定。
这些修改发生在该 tag 构建之后；运行 38156835286 核验的是它自己的源码 revision。

后续本地检查使用 Windows/Node 24.21.0：26 处 Action 引用均匹配九项上游最新稳定发行，
全部传入参数都存在于对应发行的清单中。
公开 Release/报告核验、`npm run lint:md`、`npm run check:docs` 和 `npm run test:docs`
均退出 0；39 项文档测试通过。临时脚本、API 快照和下载报告位于根目录下被忽略的
`build/actions-tags-032/`。
`npm run build:docs` 同样退出 0：1,381 页、2,893 个发布文件，本地链接、语言目录、
搜索和发布标记检查通过。
