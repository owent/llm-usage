# 剩余计划执行与验收

日期：2026-10-04；版本 0.2.1，schema 11。cwd：仓库根。
Windows 11 Pro x64 10.0.26300，Ryzen 9 9950X3D（16 核/32 线程），约 125 GiB RAM，
Node 24.21.0、Rust 1.98、Tauri CLI 2.12.0、WebView2 154.0.4258.53。
开发机结果不能认证拟定的 4 核/16 GiB 基准。

## 实施与合同

- 日查询新增可重建的派生表；只读连接在分区有效或经权威表证明为空时使用。
  供应商筛选先合并同来源/会话再计算 DISTINCT；模型/Agent 筛选使用表达式索引。
  原事件、修订、游标和未知字段保留；旧写者触发失效并移除派生身份。
  旧布局补建、跨连接写入、事务回滚、DST、保留/清空和可选 SUM 溢出有回归。
  见 [查询设计](../../design/desktop-usage/query-acceleration.md)。
- 六类图表切换 SVG；功能、主题和选区沿用实际查询结果。
- Windows 可选关闭到托盘、明确退出、节能暂停及目录通知已实现。目录最多 32 个，
  静默 2 秒合并，暂停释放句柄，日/周定点来源排除监听；失败保留间隔轮询。
- GUI 逐源间隔用单调时钟，UTC 到期时间仍持久化。全局暂停覆盖启动、逐源及
  系统残留触发；恢复只补扫一次，手动仍可读取全部启用来源。
- 每源默认 30 秒、自动轮次 300 秒协作式预算，瞬时 IO/SQLite 锁最多重试 3 次，
  5/15/30 秒等待仅在剩余预算允许时执行；中断的结束状态可持久化，之后从游标恢复。
  设置写锁等待前先暂停，保存异步执行；重试等待及下一次读取均检查暂停/期限。
  两个来源实例槽并行（同 Agent 不同根也可并行），同实例作业在解析前合并，
  解析在数据库锁外执行，短写入仍串行。JSON/JSONL 受控读取、JSON 解析、
  SQLite VM/备份分页、权威归档、保留和费用事务均接入协作控制；OS 阻塞不能强制中止。
  单文件默认 32 MiB 读取窗，完整行可提交后续读，中断/未访问/合并不推进到期规则。
  指纹/代数与事件/游标同事务，失败后的同大小替换可重新识别；旧结果与修订保留。
  见 [调度合同](../../design/desktop-usage/scheduling.md)。
- 遥测仅保留明确属性键；真实 HTTP 证明坏 gzip 返回 400、超长正文及解压炸弹
  返回 413。全局每分钟最多 120 次、4 活动连接，超限 429，拒绝请求不落盘。
  Windows Claude/Codex logs 与 CodeBuddy CLI 2.98.0 隔离 traces 已接入逐源令牌与
  当前用户系统凭据库。CodeBuddy 限定首个 PATH launcher 同目录 npm manifest，
  按固定发布记录使用 generic headers 的 %20 编码；其他版本不推断支持。预览只显示
  占位符、后台检查只读；请求鉴权先于继续读正文，拒绝重复头/错误路径/Origin/转发头。
  失败回收、撤销吊销、来源隔离和重启后从系统存储读取的代码路径已实施。
  真实 exporter、其他平台存储、采样及跨载体关联仍有缺口，V22/V25 不完整。
  见 [认证合同](../../design/desktop-usage/receiver-auth.md)。

## 已执行检查

查询专项 `cargo test --offline --manifest-path desktop/src-tauri/Cargo.toml
-p llm-usage-core --test query_cache`：退出 0，7 项。
接收器专项 `cargo test --offline --manifest-path desktop/src-tauri/Cargo.toml
-p llm-usage-desktop otel_receiver::tests`：随完整检查通过，17 项，包含真实 HTTP 限流、压缩与认证边界。
暂停专项：Codex 合同 7 项及暂停意图 1 项，退出 0。瞬时失败后暂停不等待下一轮
重试、不新增用量/游标；恢复后同一来源读取成功。数据库写锁持有期间，多个待保存
暂停请求独立生效，失败/取消只释放本请求。

并行、受控读取/SQLite/维护回滚和同大小替换失败恢复专项共 9 项；
分页备份中断、ZCode 权威归档恢复、Codex 续读与 Kilo 锁等待期限也纳入完整回归。
`npm run test:receiver` 退出 0，8 项：真实 release/IPC/HTTP/Windows 凭据库，
端口冲突保留原配置和意图；三个独立凭据应用/认证，CodeBuddy protobuf 只进入隔离
traces、不能进入 logs/主 span，撤销一个不影响另一个，全部
撤销后拒绝旧令牌并恢复原始配置，自有凭据残留 0。安装与 exporter 为合成载体，
没有启动 Agent、模型调用或修改真实用户配置；不代替真实产品导出。
`cargo test ... -p llm-usage-desktop windows_credential_store_roundtrip -- --ignored`
退出 0，1 项；原生系统存储创建、重新读取、路径限制和精确删除通过。
预览脱敏/失败回收专项及 HTTP 缺头/重复头/错误路由/转发/吊销专项同时通过。

完整 `npm run verify` 退出 0：Rust 889、前端 20、脚本 3、类型无错误/告警、fmt、
Clippy -D warnings 及前端构建通过；默认忽略 6 项显式/环境测试。
`npm run build:desktop`、`npm run test:headless` 退出 0，后者 11 项、3 事件/75 token。
`npm run test:browser` 退出 0：Edge 模拟 IPC 验收分组小时图、tooltip、币种卡片、
窄屏明细、部分数据饼图、配置批量应用/重试/撤销、十语言、主题、时区、筛选和分页。
最终 NSIS 3,901,754 字节（3.72 MiB），主程序 9,856,000 字节；前端 JS/CSS gzip
分别 363.77/9.50 kB。已构建，未安装。

`npm run test:desktop -- --runs 20` 退出 0：实际 WebView2/IPC 共 17 项，20 次
小库首屏 P95 770.09 ms。原生 DPI 144（150%）下用 Windows UI Automation 核对
控件名称；原生 WM_CLOSE 验证托盘隐藏，关闭选项后恢复窗口。该脚本使用
`--force-renderer-accessibility` 测试标志，不写入成品默认配置，也不认证 Narrator/NVDA。
真实文件通知在全局一天间隔下读取新增记录，暂停后不自动采集，手动仍读取；
普通用户 OS 分钟任务在 GUI 启动前已独立核对新增用量。

`node desktop/tests/native-incremental.mjs --runs 20` 退出 0：
百万库每轮新增 1,000 条至可见卡片更新，20 轮 P95 717.98 ms，最终 1,020,001 条事件；
token 合计、缓存失效及重扫幂等通过。隔离副本关闭保留，以免日期跨日时正常清理
基准数据改变固定计数；原始百万基准未改。

## 查询与导入

从根执行 release `bench_v20 <合成目录> <1000000/10000000> --query-only --filtered`。
旧库补建另加 `--prepare-indexes`；该模式先验证非空 synthetic/bench 来源，禁止写真实库。
基准为 366 日、50 模型、20 Agent，各查询 20 次清除应用连接缓存，OS 文件缓存未清空。
最终复测在完整检查/构建完成后串行执行。

| 查询 P95 / 数据量 | 百万 | 千万 |
| --- | ---: | ---: |
| 无维度筛选 | 47.05 ms | 133.86 ms |
| 模型 | 35.83 ms | 87.95 ms |
| Agent | 50.02 ms | 196.62 ms |
| 供应商 | 52.83 ms | 163.33 ms |
| 模型 + Agent | 24.60 ms | 145.96 ms |
| 命中应用缓存 | 0.024 ms | 0.026 ms |
| 明细 200 行分页 | 1.15 ms | 1.16 ms |

以上查询在此开发机达到 200 ms 目标，Agent 千万档余量较小。它们不认证所有
筛选组合、小时/周/月或拟定硬件。旧库第一次补建约 8.56 / 49.00 秒，包含新索引和派生表；
随后补建/规划核对为 286 / 2,269 ms。数据库 1,079.4 / 9,461.8 MiB，WAL 0 MiB，
较原 906.3 / 8,034.1 MiB 增加 173.1 / 1,427.7 MiB；空间代价保留，不隐去索引。
旧库首次升级准备耗时不计入下面已经准备好数据库的重复原生启动测量。

`node desktop/tests/bench-import.mjs` 退出 0：通过当前 release 内核 `commit_batch`，
每批 50,000，完整导入 1,000,000 事件用时 184.5 秒，5,421 条/秒；500 ms 采样全部
子进程，private bytes 峰值 53.45 MiB、working set 62.96 MiB。它不启动 WebView，
证明标准化事件写入有界，不认证全部原始载体和 GUI 完整导入峰值。

全量并发回归发现 Winsock 提前关闭拒绝连接可能使响应丢失，已改为发送关闭序列，
再以累计 100 ms/64 KiB 上限丢弃剩余输入，不解析/落盘；JSON 响应统一 CRLF 和
Content-Length。关闭依据见 [Microsoft Winsock](https://learn.microsoft.com/en-us/windows/win32/winsock/graceful-shutdown-linger-options-and-socket-closure-2)。

## 原生资源与内存目标

`node desktop/tests/native-scale.mjs --runs 20 --idle-seconds 600 --ui-cancel` 退出 0。
已准备百万事件库，20 次首屏 P95 1,260.72 ms；两个实际 IPC 取消及设置页按钮取消
均保留事件、修订和游标。随后以默认 GPU 渲染连续采样 601.19 秒，118 个样本、
最多 7 进程，包含主程序和全部本应用 WebView 子进程。

| 指标 | 实测 | 目标状态 |
| --- | ---: | --- |
| 全进程 private bytes 均值 / 峰值 | 299.90 / 363.30 MiB | 旧 180 MiB 未达标；符合新均值 350 / 峰值 400 MiB 预算（开发机） |
| 全进程 working set 峰值 | 525.79 MiB | 另报，不替代 private bytes |
| 空闲 CPU，单个逻辑核 | 0.244% | 低于 1% |
| 首屏 P95 | 1,260.72 ms | 低于 2 秒；不含首次旧库补建 |
| 应用调度循环计数 | 1,202 | 约每秒 2 次；不代表 OS 全部唤醒 |

| 进程角色 | private bytes 均值 | 各角色峰值 |
| --- | ---: | ---: |
| 主程序 | 17.01 MiB | 17.49 MiB |
| WebView 浏览器 | 43.38 MiB | 45.38 MiB |
| WebView GPU | 146.24 MiB | 187.21 MiB |
| WebView 渲染器 | 71.13 MiB | 92.04 MiB |
| WebView utility | 19.21 MiB | 19.79 MiB |
| WebView 其他 | 2.93 MiB | 3.37 MiB |

各角色峰值可能出现在不同样本，不能相加当作全进程峰值。每个样本都断言角色
之和等于全进程值；原始命令行/路径不写入结果。GPU 约占均值的 48.8%，是本机
最大占用来源；此测量不能独立区分运行时/显卡驱动固定开销与页面造成的开销。
之前仅一次启动/31 秒的 `--disable-gpu` 诊断为均值 183.48 / 峰值 185.54 MiB，
仍超目标；该诊断不代替最终默认配置、20 次启动及完整时长验收。
[Microsoft WebView2](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/webview-features-flags)
将这些标志用于测试/调试，不据此修改成品默认配置。

2026-10-04 用户允许依据现有功能与测量调整预算。当前为单窗口、完整总览、SVG
图表、SQLite 和有界查询缓存；没有同时运行多个 WebView 或随包 Node/Python 服务。
41 个适配器和后处理能力不意味着空闲时将所有来源载入内存。主程序仅占约 17 MiB，
GPU/浏览器/渲染器占主要部分，180 MiB 不适合作为当前 Windows GUI 的硬门槛。
[微软性能指导](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/performance)
解释多进程、GPU 驱动/缓冲与内容复杂度带来的开销，建议保留硬件加速，但未规定
统一最低内存；不能将本机 GPU 占用全部称为不可优化固定成本。
新的工程预算为连续 10 分钟全进程 private bytes 均值 ≤350 MiB、采样峰值 ≤400 MiB，
相对实测留约 17%/10% 余量，当前开发机符合。GUI 首次导入峰值改为 ≤512 MiB，
独立验证；working set、headless 与托盘后台分别记录。原测量与旧目标失败事实保留。
`native-scale.mjs --enforce-budget` 同时检查均值、峰值和完整时长；软件渲染/暖空页
诊断不认证成品预算。拟定 4 核/16 GiB 硬件、固定运行时/驱动/DPI、持续增长与其他
平台仍需复测；放宽预算不免除泄漏或无界载入排查。按同一原始采样重新核对，
前 120 秒均值 309.06 MiB、末 120 秒 299.93 MiB；本次 10 分钟未观察到持续增长，
不能认证长时无泄漏。新预算评估结果保存在 `build/plan-continuation/memory-budget.json`，
没有改写原始采样。

百万 GUI 首次导入另用 `npm run test:import`：空库、100 个隔离 Codex 根、每根
10,000 条非空合成事件；保留产品默认保留规则，从真实按钮触发到可见卡片/图表，
独立按 500 ms 采样全进程。初次 600 秒时限在 950,000 条已提交、97 个来源已登记
时到达，整轮仍在运行，退出 1，未作为峰值验收。失败结果保存在
`build/plan-continuation/native-first-import/1791123652310/failure.json`。
已确认持续提交后将该首次回填测试的完整采样窗改为 900 秒；这是扩大观测时长，
不改变每源产品预算、2 秒增量目标或 512 MiB 峰值门槛。脚本初始化另修正了保留参数、
Windows 内联采样方式、后端保存后重载界面语言，以及空库先等待刷新按钮；
百万已提交后的中文缩写误判属于测试脚本失败，不是产品用量失败，记录仍保留。

最终 `npm run test:import` 退出 0：从空库首次采集到可见卡片/图表 668.55 秒，
1,000,000 次调用、15,000,000 token 的独立预期与实际 summary 一致；第二次扫描
仍为 1,000,000 条。连续采样 669.86 秒、1,031 样本，配置间隔 500 ms（包含采样开销），
最多 8 个本应用进程；private bytes 均值 303.42 / 峰值 371.09 MiB，达到 512 MiB
导入预算；working set 峰值 586.97 MiB 另报。主程序 private bytes 峰值 72.57 MiB，
GPU 165.85 MiB、渲染器 80.96 MiB；各角色峰值不相加。没有脚本异常或已观测页面
外部 HTTP，不能扩大为 OS 全进程出站审计。结果在
`build/plan-continuation/native-first-import/1791125560874/result.json`。
这项测量使用合成 Codex JSONL、100 个输入根及产品默认保留，不是绕过解析的
标准化批量写入；两者吞吐不能直接比较，也不认证其他真实 Agent/版本/超大载体。

另执行 `node desktop/tests/native-scale.mjs --runs 1 --idle-seconds 600 --blank-page`，
退出 0：在相同百万库的完整页面暖启动后导航 `about:blank` 并请求 GC；仍保持默认 GPU，
采样 600.36 秒、118 样本、最多 7 进程。private bytes 均值 270.63 / 峰值 283.63 MiB，
GPU 均值 148.12 MiB，渲染器 41.81 MiB，主程序 16.26 MiB；空闲单核 CPU 0.083%。
导航后不能读取应用调度计数，明确记为 null。该诊断仍超过 180 MiB，说明暖 WebView
的 GPU 进程占用仍较高；不能证明全新空页启动的最低开销、拟定硬件
表现或把空页当成成品验收。下一步仍需要全新最小页面和拟定机器的原生复测。
结果在 `build/plan-finalization/native-scale/1791121549201/result.json`。

原生/浏览器检查未观察到页面外部 HTTP 和脚本异常；这只覆盖 WebView 已观测请求，
不能当作全进程出站审计。最终结果保存在
`build/plan-finalization/native-scale/1791109899513/result.json`；其他原生和增量结果分别
位于 `build/plan-completion/native/1791123291549/` 与
`build/query-next/native-incremental/1791121379123/`。
最终构建/完整检查日志位于 `build/plan-continuation/build-final.log`、`verify-final.log`；
接收认证结果位于 `build/plan-continuation/native-receiver/1791123187714/result.json`。

## 本机来源条件

只读运行内置 41 适配器的真实 discover，限定已知根；输出文件计数、有限格式/版本依据，
不输出路径、会话标识或正文。Zed SQLite 另查只读 COUNT，0 个 thread；空库不作用量验收。
默认根未发现的 30 项为 claude、gemini、qwen、cline、dsh、hermes、openclaw、opencode、
mimo-code、zoo、aider、junie、xum、droid、amp、grok、roo、goose、crush、jcode、workbuddy、
gajae-code、commandcode、continue、atomcode、kiro、antigravity、qoder、copilot、otel。
WorkBuddy 的既有手工真实核对不被本次默认根结果覆盖；探针仅说明发现/读取条件，
不把库内最高版本、格式可读或目录存在当作逐次用量认证。

没有启动 Agent、模型请求、WSL/容器，未修改真实 IDE 配置来制造样本。
缺失非空真实样本、其他原生平台、拟定硬件基准、安装/升级/卸载与签名/发布保持未验收。
安装按既有指示跳过，推送/部署/发布未执行；F1、完整明细 Merge 和预算提醒保持后置。
trace SQLite 仍需固定 schema/属性全文及非空样本核对。本次固定 GitHub/raw 页面
无法取回，独立 curl 请求 30 秒超时；没有从表名推断属性、增量或来源选择规则，
此前 C04 文档依据不扩大为本次完整实现/真实验收。

收尾 `npm run lint:md` 退出 0，180 个 Markdown；19 份变更文档的 180 个本地路径/锚点
无错误。`python -X utf8 .../quick_validate.py .agents/skills/ai-maintenance` 退出 0，
Skill is valid；description 未改变，不声称模型路由评估。`git diff --check` 退出 0。
只读核对本任务 build 命名空间：应用进程、WebView 进程和计划任务均为 0；
未清理用户对象，git status 未出现临时产物，期间其他任务的用词修改已保留。
