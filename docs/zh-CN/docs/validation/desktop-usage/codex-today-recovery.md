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
