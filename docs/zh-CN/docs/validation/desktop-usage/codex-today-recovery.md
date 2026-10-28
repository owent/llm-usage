# Codex 当天漏采集修复

<a id="recovering-missing-current-day-codex-usage"></a>

2026-10-07，Windows 11 专业版 x64 10.0.26300，本机统计时区 Asia/Shanghai，应用 0.2.1。
Node 24.21.0、Cargo 1.98.1；依赖使用现有锁文件。
临时脚本、日志、一致备份和独立核算放根 `build/codex-today/`，原生来源只读。

<a id="cause-and-correction"></a>

## 原因与修复

修复前统计库的 Codex 最新明细为 2026-10-06；当天原生 rollout 已有独立
`token_usage_record`，0.160.1 / 0.162.0-alpha.2 的六个用量字段可由既有解析器读取。
这些版本继续保留 `latest_fallback`，本次不登记为逐版本完整核验。
Codex 来源启用，最近一次启动采集结束为 interrupted，原因是
`source_window_budget_exhausted; complete lines retained for next scan`；
已登记文件的日期止于 10-05，当天文件尚未访问。

原扫描按日期升序，一份历史文件达到默认 32 MiB 读取窗即结束该来源本轮采集。
同一大文件在后续轮次仍排在较新文件前，历史回填推迟了当天入库。
修复保留有界读取与中断规则：Codex 启用框架按已保存的文件访问时间轮转，未访问
文件优先，同等条件按日期路径倒序。其他适配器保留原读取顺序。每次确认的事件、
指纹、来源代数与完整行游标仍同事务提交；失败不提前消费文件。没有清库、修改原始 rollout 或
把累计回声再次相加。

<a id="regressions-and-native-comparisons"></a>

## 回归与真实核对

- 新增 `codex_file_fairness.rs` 三项：当天小文件与历史大文件并存；较新大文件
  用完读取窗后，在重开库的下一轮让未访问小文件先读，并继续到全部文件完成。
  桌面并行入口另验同一轮转规则，避免根包装器遗漏转发。最初两项在修复前
  均失败（退出 101），修复后三项通过。
- 首次定向运行文件轮转、Codex 增量、并行、提交回滚与中断共 19 项，退出 0；
  重复读取没有重复调用，半行、替换/截断/改名及回滚规则保留。
- SQLite Online Backup 包含 WAL 中有效数据。先在一致备份上运行修复后的
  Windows release `LLMUsage.exe --scan-once --data-dir <备份目录>`，只启用 Codex。
  第一次仍有文件读取窗未完成，第二次来源采集 succeeded；没有把进程退出 0
  当成来源完整成功。
- 独立脚本对现存原生逐次记录按稳定 response 身份去重、按完成时间选取北京时间
  10-07，截止 `2026-10-07 14:01:37.124 +08:00`：719 次调用，输入 69,722,873、
  缓存读 65,866,240、缓存写 0、输出 408,863、推理输出 115,244，
  总 token 70,131,736。缓存读属于输入、推理属于输出，六项不互相叠加。
  全部 719 条身份、时间和六字段与采集库逐条相符。
- 应用未运行时，通过修复后客户端的单写者锁及原生手动采集恢复实际统计库。
  两次采集进程退出 0，Codex 第二次来源 succeeded；同一截止范围逐条匹配，
  修复前已有 4,650 条 Codex 明细的摘要及 conflict 全部保留，新增历史回填另计。
  实际库 `PRAGMA quick_check=ok`，`foreign_key_check` 无结果。
- 最终客户端重新核对截至 `2026-10-07 14:12:53.983 +08:00` 的实际库：781 次调用，
  输入 75,709,478、缓存读 71,722,496、缓存写 0、输出 435,339、推理输出 127,681，
  总 token 76,144,817；身份、时间和六字段逐条相符。再次原生手动采集后同一范围
  仍为 781 次 / 76,144,817 tokens，4,650 条原有明细继续全部保留。
  两次来源均 succeeded、无更新/错误；新增的 47 / 2 条包含截止之后及新增回填，
  不将活动会话的新增调用误判为重复采集。

本次只核验现存本机 rollout 与统计库恢复；仍运行的会话在截止之后新增用量另算。
GUI 显示、发行安装/升级、其他 Codex 版本及未保存的调用不由这次核对确认。

<a id="commands-and-delivery-checks"></a>

## 命令与交付检查

定向检查：`cargo test --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core --locked
--test codex_file_fairness --test codex_incremental_v12 --test scan_commit_rollback
--test scan_controls --test parallel_scans`。

Windows release 与 NSIS 构建 `npm run build:desktop` 退出 0；制品位于既有
`desktop/src-tauri/target/release/`。构建不代表已安装或发布。

首次统一检查发现 oh-my-pi fork 测试依赖既有报告顺序（原文件 unchanged、新文件
complete）；将轮转明确限制为已核验文件可独立读取的 Codex，保留其他适配器的
发现与冲突处理顺序，未改动该回归断言。

最终 `npm run verify` 退出 0：Rust 1,028 项通过、8 项平台条件忽略，前端 22、
脚本 4 项通过，Svelte 0 错误/告警，fmt、Clippy、文档和前端构建通过。
`npm run test:headless` 退出 0：11 项真实可执行文件/隔离合成库检查，3 调用 / 75 tokens。
最终文档 lint 和 `git diff --check` 退出 0；工作区没有临时产物混入。

终端最初因 `CreateProcessAsUserW failed: 5` 无法启动，按执行环境的权限重试后恢复；
这是工具启动错误，未用作产品测试结果。未新增 GUI/IPC、跨平台 CI 或发行安装验收。

<a id="astra-appended-session-recovery"></a>

## 已有会话追加的 Astra 用量恢复

2026-10-28 检查发现，Codex 0.164.0-alpha.11.1 的本机记录包含北京时间
2026-10-27 的 83 次 `gpt-6-astra` 调用：输入 11,504,926、缓存读取 11,247,104、
缓存写入 0、输出 81,528、推理输出 24,139、总量 11,586,454。统计库中没有这些
调用。既有六字段解析器及此前的 `turn_context.model` 可以读取，模型名和未登记
的客户端版本没有阻止入库。

该会话已提交游标停在字节 16,357,786、下一行 3,772，而当前文件已有 32,295,275
字节。第 6,013 行在 2026-10-27 21:00:11.425 +08:00 切换到 Astra。最近的启动
采集均以 `source_window_budget_exhausted` 结束，另有 462 个已登记 Codex 文件
仍为 `new`。原轮转顺序把这些待补采文件排在已读完会话的新增内容前面。

修复后，优先访问此前已完整读完、随后大小或修改时间变化的文件，组内按确认访问
时间排序；其他文件保留既有轮转。未读完窗口不会仅因游标尚未到文件末尾而持续
优先。每文件/来源限制、解析器身份、事务接纳及诊断历史继续保留。新增模型切换
回归在旧实现上失败（退出 101），覆盖重开旧数据库、完整和未读完的追加窗口，
以及重新让历史文件优先读取。

通过 SQLite Online Backup 保留原 WAL 状态。修复后的原生核心在副本中新增
1,749 条 Codex 事件，重复扫描又新增其他事件 1,147 条，Astra 始终保持 83 次 /
11,586,454 tokens。原有 16,602 条 Codex 事件的摘要、冲突标记及创建时间全部保留。
重新构建的 Windows `LLMUsage.exe --scan-once --data-dir <副本目录>` 退出 0，
Astra 的调用身份、完成时间及六个 token 字段仍完全一致。来源报告仍为 interrupted，
因为历史文件窗口还需继续补采；退出 0 不代表全部采集完成。

随后，修复后的核心持有与应用相同的 OS 文件锁，只对实际统计库补采 Codex：
新增 1,772 条、更新 0 条。只读比较确认全部 83 条 Astra 原生记录一致，原有
16,602 条 Codex 明细完整保留，`quick_check=ok`、外键检查无异常。产品的按日
汇总查询以 Codex 和 `gpt-6-astra` 筛选，返回 Asia/Shanghai 日期 2026-10-27
相同的调用数、输入、缓存读取、输出及总量。原始来源及用户设置保持不变。
重新构建的可执行文件在备份上验证；实际库由原生核心探针恢复，没有 GUI/IPC
或安装验收。

Windows x64 上的 `cargo build --manifest-path desktop/src-tauri/Cargo.toml --locked
--release -p llm-usage-desktop` 退出 0。根 `npm run verify` 中的 Markdown、资源、
脚本/UI 测试、Svelte、格式和 Clippy 通过，随后 Copilot 测试在沙箱临时目录重命名
时遇到 `PermissionDenied`（Windows 错误 5）。相同未修改测试在正常用户权限下
通过；完整重跑 `npm run test:rust`，1,105 项通过、9 项按平台条件忽略。前端
生产构建和 `npm run check:docs` 也通过。该次数据恢复没有替换已安装程序或发布版本。

### 版本 0.3.6 复查

本次发布合入了已发布的 0.3.5 基线。复查发现另外两种情况：未知修改时间可能让
未变化文件获得优先级；读到文件末尾但最后一条 JSONL 尚未写完时，后来完成的记录
可能仍排在历史补采后。现在只比较已知修改时间，并在游标事务中保存
`file_read_complete` 标记，区分读到末尾与耗尽读取窗口。旧游标继续以偏移等于
保存大小作为依据。后续有界窗口未读完时会清除此标记，继续追加也不能让该窗口
持续优先于历史文件。不需要数据库结构迁移，也不改变用量或解析器身份。

新增 8 项回归，`codex_file_fairness` 共 12 项，覆盖完整和未完成的追加窗口、
重开数据库后补齐换行、修改时间缺失、同大小改写、旧游标、提交失败时访问时间及
游标回滚、应用并行入口和按模型筛选的日汇总。采集、回滚、中断和并行专项共
21 项通过。AI 维护指南和中英调度规范已同步这些规则。

版本 0.3.6 在 Windows x64 / Rust 1.99.1 上通过 `npm run verify`：1,135 项
Rust 测试通过、9 项按条件忽略，20 项脚本测试、32 项前端测试、Svelte、格式、
Clippy 和前端生产构建全部通过。`npm run check:docs`、41 项文档测试及
`npm run build:docs` 通过，检查了 1,407 个页面和 2,947 个发布文件。npm 和
冻结 pnpm 恢复均通过，package、Tauri 和 Cargo 版本一致。这些检查不增加 GUI
或安装验收结论；原生发布打包由 tag CI 执行。

本次临时产物位于 `build/astra-recovery/`。原始 Agent 文件只读，验证输出只包含
允许提取的用量、模型、时间及版本字段。

### 扫描优先级中的原生路径

2026-10-28，[issue #6](https://github.com/owent/llm-usage/issues/6) 指出
`files_in_scan_order` 把规范化后的数据库身份用作元数据读取路径。去掉 Windows
verbatim 前缀后，规范 UNC 身份变成 `UNC/server/share/...`，不再是原始文件系统路径。
元数据读取失败会使追加内容失去优先级。

排序映射现在同时保留发现时的原始 `PathBuf` 和上次访问时间，并用原始路径读取元数据。
已保存的身份、游标及轮转规则保持不变。回归使用真实 Windows verbatim 路径和名称末尾
带点的目录，先确认原始路径可读而身份字符串不可读。原实现在优先读取历史文件、未先读
追加用量处失败；修复后验证重开数据库、身份稳定、追加模型的调用数及 token、后续历史
轮转和无重复调用。Unix 分支使用目录名中的字面反斜杠。

Windows 采集、回滚、中断和并行专项通过 22 项，包括全部 13 项 Codex 文件轮转测试。
首次重读断言需要补入第二份未访问的历史文件：访问时间相同的文件按设计保留发现顺序。
本次没有挂载或修改 SMB 共享，未在本地执行真实 UNC 共享访问或 Unix 分支。
日志位于 `build/issue-6/`。

两项 issue 修复的最终 Windows x64 / Rust 1.99.1 检查通过：`npm run fmt:check`、
`npm run clippy` 和 `npm run test:rust` 均以 0 退出，1,139 项 Rust 测试通过，
9 项按条件忽略。Markdown lint 和 `npm run check:docs` 也以 0 退出。
这些检查不增加 GUI 或安装验收。
