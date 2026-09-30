# M5 遥测载体与历史余项实施验证记录（2026-09-29 第二轮）

范围：上轮盘点出的未完成项逐项处理——M5 本机遥测主体（Copilot CLI +
OTel spans 载体 + OTLP 接收器）、M1 迁移前备份余项、M6 逐源定时接线、
V20 性能初值、M7 空闲内存复测。不提交、不推送、不部署。

## 本机数据状态变化（2026-09-29 复查）

- **Copilot CLI 数据已恢复**：`~/.copilot/session-store.db`（schema_version=8）
  `assistant_usage_events` 36 行真实用量（2026-08-03，claude-opus-4.8）。
  2026-09-25 "全树消失"为升级迁移，现已回写。
- Claude Code：`~/.claude` 仅空 sessions/backups（无 projects 转录）——真实
  fixture 仍不可得。
- CodeBuddy：`~/.codebuddy` 仅 diagnostics/logs（memwatch），无用量载体；
  官方文档证实 CLI 仅支持 OTLP 导出（无 file exporter）。
- WorkBuddy：`%APPDATA%` 下 WorkBuddy/WorkBuddy AI 目录**重新出现但为空**——
  仍是"无本地格式可核验"，M4 挂起状态不变（目录出现本身已记录）。
- VS Code copilot-chat globalStorage session-store.db：只有 sessions/turns/
  session_files/session_refs 表，**无 usage 表**（本机实测）——与 A06
  "本地无逐次用量、需启用遥测"结论一致。

后续源码调查补充：上述 CodeBuddy 结论只针对**本轮本机目录状态和 OTel
file exporter**。官方目录文档另确认 `~/.codebuddy/projects` 会话 JSONL，第三方
源码确认其中的用量字段；WorkBuddy `.workbuddy/projects` 也有第三方源码证据。
两者已按[本地会话记录](m4-buddy-local.md)文档级接入，本机仍无真实样本。

## M5 实施

### 1. Copilot CLI 适配器（真实数据核对 PASS）

`adapters/copilot/`（common/detect/versions）：assistant_usage_events 逐 turn
事件；`map_copilot` 自根级 usage_map.rs 下沉并扩展 reasoning 列（V30：产品
映射随目录）。锚点 schema_version=8（KnownVersion；其他走 latest 兼容）。

- 真实脱敏 fixture：`tests/fixtures/copilot/assistant-usage-events-v8.sanitized.json`
  （36 行白名单列提取，session_id/agent_id 匿名化，期望值在 _expectations.md）。
- 合同测试 `copilot_contract.rs`：fixture 重建 SQLite → run_adapter_scan →
  合计与人工核算一致（input 4,649,981 / output 34,157 / cache_read 4,416,791 /
  cache_write 233,118 / reasoning 17,678 / uncached 72 / total 4,684,138）；
  重复扫描不增量。
- 真实核对 `examples/real_verify_copilot.rs`（cwd desktop/src-tauri）：
  `cargo run -p llm-usage-core --example real_verify_copilot -- "$USERPROFILE/.copilot" build/m5-copilot/verify`
  → 36 事件、各桶与独立 SQL 直查一致、重扫幂等，**VERDICT: PASS**。
- 语义复核：36/36 行满足 input ≥ cache_read + cache_write（M0 结论在真实数据成立）；
  request_multiplier=27.0（premium 倍率）与 total_nano_aiu 不入 token 统计。

### 2. OTel spans JSONL 适配器（文档级）

`adapters/otel/`：覆盖三载体——VS Code Copilot Chat file exporter
（`github.copilot.chat.otel.*`，官方文档 bdc5ebe：NDJSON 非 OTLP、
startTime [秒,纳秒] 对）、Copilot CLI file exporter（行级 schema 未文档化，
同族容错解析待样本）、本应用 OTLP 接收器归一化输出（CodeBuddy agentlens
无前缀 usage.*）。汇总 span（invoke_agent/codebuddy_code.interaction）与
model_request 按官方防双计警告跳过；白名单属性，trace/span ID 键名双拼写
容错（文档未逐字给出）。

### 3. 本地 OTLP/HTTP 接收器（src-tauri/src/otel_receiver.rs）

- 仅 127.0.0.1（loopback）；settings `otel_receiver_enabled`（默认关闭）+
  `otel_receiver_port`（4318）；启用 = 用户显式授权本机实例（V25 语义）。
- OTLP/JSON 与 OTLP/protobuf 双协议（CodeBuddy 仅 protobuf：手写 wire 解析
  ResourceSpans→ScopeSpans→Span→KeyValue→AnyValue）；gzip 有界解压（64 MiB）；
  头 64 KiB / body 64 MiB 上限（V22 压缩炸弹防护）。
- 白名单落盘：gen_ai.*/usage.*/model* 等前缀 + 显式拒绝正文键
  （gen_ai.input.messages 等）+ 字符串 256B 上限；输出
  `%APPDATA%/llm-usage-desktop/otel/spans.jsonl`（otel 适配器默认发现根）。
- 测试：JSON/protobuf 解析单测 + **真实 E2E**（起接收器→HTTP POST /v1/traces→
  落盘→otel 适配器扫描出 codebuddy model_stream 事件，HTTP 200）。
- 调试副产物修复两项实现缺陷：非阻塞 listener 接受的连接需显式恢复阻塞；
  HTTP 头分隔符字节常量（现 [0x0d,0x0a,0x0d,0x0a]）。

## M1 余项：迁移/重建/清理前一致备份 + 空间检查

`src-tauri/src/db_backup.rs`（+ fs4 依赖）：

- `consistent_backup`：VACUUM INTO 一致快照（WAL 下一致点）+ 备份前剩余空间
  ≥ 库文件（含 -wal）×1.1 检查 + 保留最近 3 份（时间戳字典序清理）。
- 接线：schema 不匹配重建分支（app_state.rs）先 `consistent_backup_legacy`
  只读备份旧库，失败即中止（原库保持不动）；清空数据（clear_all_data）
  改用共享助手（新增空间检查）。原 backup_before_clear 内联实现移除。
- 测试：空间判定纯函数 + 备份往返（快照可独立打开读出数据）+ 修剪保留 3 份
  （发现并记录 Windows 文件锁语义：未关闭的 SQLite 连接会阻止删除，测试已
  用作用域关闭连接）。依赖 fs4 ^0.13。

## M6 余项：逐源定时接线（extraction_schedules）

- 核心 `core/src/schedules.rs`：SourceScheduleRule（interval 15s–24h /
  daily HH:MM / weekly ISO weekday+HH:MM；首版无任意 cron）；next_due 纯函数
  （jiff tz；DST 歧义回退 +24h）；upsert/delete/read + due_instances +
  custom_scheduled_instances + mark_source_run（schedule_state 记录）。
  单测 5 项（边界/每日/每周跨周/禁用/存取往返）。
- 框架 `run_adapter_scan_filtered`（InstanceFilter include/exclude；
  run_adapter_scan 保持原签名委托）。
- 调度循环（scanner.rs）：全局刷新排除有自定义计划的实例（覆盖语义）；
  500ms 轮询到期实例单独触发（FixedTime 语义，与全局/手动经 refresh 单飞
  合并——同源不并发）；运行后推进 next_due（错过时点醒来只补一次）。
- 命令 `set_source_schedule`（注册进 invoke_handler）+ list_sources 附带
  schedule/nextDueMs；UI（SourceList.svelte）：每源计划编辑行（继承全局/
  间隔预设 15s–24h/每日/每周 + 时间与周选择器 + 下次提取显示），周名按
  当前语言 Intl 生成；i18n 6 键 ×10 语言（locales.ts 位置化插入 + zh-CN/en）。
- scheduling.md 状态行更新为已实施。

## V20 性能初值（M1/M7 前置数据）

`examples/bench_v20.rs`（release，参数化规模；cwd desktop/src-tauri）：

| 规模 | 插入吞吐 | 库+WAL | 日汇总查询 p50/p95/p99 | 明细分页(200行) p50/p95/p99 |
| --- | --- | --- | --- | --- |
| 100 万事件（366 日/4 模型/6 Agent/4 源） | 30,453/s（32.8s） | 676.8 MiB + 34.9 MiB | 0.01/0.01/0.01 ms | 0.67/1.24/1.52 ms |
| 1,000 万事件 | 23,560/s（424.4s） | 6,788.3 MiB + 35.6 MiB | 0.01/0.01/0.02 ms | 2.28/4.65/5.41 ms |

无全文件无界载入（分页查询走索引）。10M 临时库已清理。

## M7 资源复测（V21 部分数据）

release 构建（LLMUsage.exe）GUI 启动 + headless 扫描后空闲：
主进程工作集 **32.5 MB** + WebView2 子进程 **138.5 MB** ≈ **171 MB ≤ 180 MiB
目标**。M0 时代"206.5 MB 超标"为旧空壳实现，当前实现达标。
（首屏 P95 与真实安装/卸载仍在 M7 待验清单。）

## 测试与命令汇总

- `npm run verify`（cwd 仓库根）**全链通过**（含 markdownlint / svelte-check
  0 错误 0 警告 / cargo fmt + clippy -D warnings / vite 构建）。
- `cargo test`（cwd desktop/src-tauri）全部通过：**588+ 项**（新增 copilot
  合同 1、schedules 5、db_backup 2、otel_receiver 4（含 E2E）、otel 适配器
  单测、mapping_v01 迁移后全绿；V30 扩展覆盖 copilot/otel 目录与根级
  map_copilot 下沉断言）。
- `git diff --check` 通过；临时产物均在 build/（gitignore）。

## 剩余未完成项（如实登记）

| 项 | 状态 | 阻塞 |
| --- | --- | --- |
| M5 VS Code/Copilot CLI file exporter 真实样本 | 待用户启用 exporter | 载体需启用；行级 schema 待样本核验 |
| M5 CodeBuddy 端到端（真实 CLI 打接收器） | 待用户启用接收器并使用 CodeBuddy；本地会话另见 [记录](m4-buddy-local.md) | 接收器已实现并 E2E 验证（合成载荷）；双载体重叠待消解 |
| M2 claude/gemini/qwen 真实 fixture | 无本机数据 | Claude 复查仅空目录 |
| M4 WorkBuddy | [本地会话适配器](m4-buddy-local.md)已注册，后续本机 8 文件/420 事件核对通过 | trace 与 session 关系及跨版本待核 |
| M3/M4/M8 真实数据验收 | 后置 | 本机未安装/无数据 |
| F2 费用引擎实施 | 主体已实施（[F2 记录](f2-cost-engine.md)） | 余可选在线刷新/预算提醒/真实端到端 |
| M6 真实桌面逐操作验收 V13–V18/V23–V25、V24 系统任务真实验收 | 待实机操作 | 需 GUI 人工/GUI 自动化记录 |
| M7 真实安装/升级/卸载（V19）、首屏 P95（V21）、离线 | 待真实环境验收 | — |
| V26 三平台 CI 首次运行 | 待推送后登记 | 未获推送授权 |
| 文件监听触发（监听为优化项，轮询先行已合规） | 未实施 | 调度合同允许轮询先行 |
