# 最新实施与验收

日期：2026-10-05；版本 0.2.1。cwd：D:/workspace/projs/github/owent/llm-usage。
Windows 11 Pro x64 10.0.26300，Ryzen 9 9950X3D（16 核/32 线程）、约 125 GiB RAM。
Node 24.21.0、Rust 1.98、Tauri CLI 2.12.0、WebView2 154.0.4258.53；依赖以锁文件为准。
本页仅保留当前结论；安装/实际 Linux GUI 与来源最新结果见
[安装验收](installation-lifecycle.md)、[容器来源](container-sources.md)；凭据跨平台命令见
[跨平台继续执行](platform-auth-continuation.md)，
规模测量见 [剩余计划执行](plan-execution.md)，
此前取消、生命周期与语言专项见 [原生与规模验收](plan-finalization.md)。

## 当前交付

- 总览及趋势支持连续鼠标/键盘选区，统计、模型/Agent 分布、模型表和 API 参考费用
  同范围联动；自然日文案明确，不联动的热力图/周分布置后。
- 解析器元数据升级核对完整旧摘要，真实冲突继续仲裁；本机 1414 条误报修复，
  事件与用量不变，见 [冲突验收](parser-conflict-fix.md)。Kilo 独立快照只作对账，
  真实库错误的 1 个核对文件提示已修复，4,050 条事件、用量及来源字节不变，
  旧游标自动重评/重复扫描幂等，见 [选区与健康验收](trend-range-kilo.md)。
- 全局/逐源到期、后台意图、单写者与竞争合并已持久化；暂停覆盖启动和残留触发。
  定点时区独立，DST 缺失顺延、重复只执行首次。Windows 分数据库注册分钟任务，
  核对实际状态；开机启动用原生注册表 API，只操作本应用对象。
- 手动清理/清空支持提交前取消，SQLite 中断和事务回滚不改变统计修订，取消不重扫；
  提交开始后明确拒绝取消，不完整备份移除自身文件。
- 仅扫描手工目录的设置默认关闭，开启后同时限制 GUI 和后台的默认来源/额度发现。
  无 GUI 的实际分钟任务在界面重开前已独立核对新增数据；OS 注册/删除失败注入
  证明持久化意图、失败可见及重试恢复，不能替代真实权限拒绝验收。
- 十语言真实保存、HTML lang、设置控件可访问名称及 100%–200% CSS 页面缩放通过；
  图表键盘选区使用实际 IPC，统计不随语言改变。
- 日汇总、Agent 分布和日粒度图表按行合并，明细读取使用覆盖索引；两个只读连接
  复用有上限的汇总缓存。同连接/其他连接写入、选区、日期和保留边界回归通过，
  查询不留下读事务；会话身份、未知字段和归档覆盖口径保持一致。
- 日查询有可选派生表，供应商会话跨日/来源/供应商去重，模型/Agent 使用表达式索引；
  旧写者与布局更新、NULL/未知值、事务回滚、溢出及保留/清空有回归。
  六类图表使用 SVG，主题和选区行为保留。
- Windows 托盘隐藏/恢复/退出、节能暂停、最多 32 目录的文件通知及逐源单调计时已实施。
  协作式单源/轮次期限和瞬时失败重试有界；保存暂停意图先于数据库锁等待生效，
  重试等待及下一次读取均检查暂停，失败保留游标，恢复可继续采集。
- 两个来源实例槽并行，同 Agent 不同根可占两个槽；同实例合并，写入仍串行。
  JSON/JSONL 分块、SQLite VM/分页备份、归档及保留/费用事务均检查中断。
  指纹/代数与事件/游标同事务；同大小替换失败可恢复，未访问/合并/中断不推进期限。
- 接收器只保留精确字段，真实 HTTP 验收 gzip/正文边界和 120 次/分钟、4 活动连接限制；
  拒绝连接无落盘，429 响应使用有界关闭序列。Windows Claude/Codex logs 使用
  逐源令牌与当前用户凭据库，真实 IPC/HTTP 通过预览、应用、失败恢复、吊销和来源隔离。
  CodeBuddy CLI 2.98.0 依首个 PATH launcher 同目录 manifest 配置 generic headers，
  输出保持隔离；其他版本/安装形式另验。Linux 默认持久 Secret Service 与真实 HTTP
  在 WSL 隔离验证通过；macOS 非同步/无认证 UI 的 Keychain 已实现并交叉类型检查，
  原生运行保留。失败写入/回查仅精确回收自有项，Linux 操作限时 3 秒。
- 仅今天新增用量时跳过稳定归档的重复物化；迟到数据、删除、缺失归档、日期/策略
  变化及异常修订仍重算。采集期间每 500 ms 核对状态，完成后恢复 10 秒空闲检查；
  已接受但尚未启动、两次检查间完成的短任务均保留完成通知。
- Windows 独立卸载精确清理当前安装的任务/启动项并关闭已有库后台意图，其他值保留；
  运行中写者或清理失败阻止程序删除，升级临时卸载保留任务。实际 NSIS 往返通过。
- Debian rootless Podman 的真实 deb 生命周期、GTK/WebKit 窗口/IPC/五页与 AppImage
  提取及 FUSE GUI 通过；FUSE 仅在自有容器显式加 SYS_ADMIN，普通应用用户有效能力 0、
  默认 seccomp，实际只读挂载与退出释放均核对。GTK 缩放 1/2 各 40 项通过。
  Qwen 0.25.0 / OpenCode 1.18.34 官方客户端的真实本地主循环通过；后台记忆/标题
  调用覆盖缺口保留。Qwen file 已找到后台逐次记录，形状及权威分区未接入现有解析。

## 最新检查

| 检查 | 实际结果 | 范围 |
| --- | --- | --- |
| npm run verify | 退出 0；Rust 908（核心 810、应用 98）、前端 21、脚本 4；类型无错误/告警 | Markdown 189 文件、资源、fmt、Clippy -D warnings、单元/合同/集成、前端构建；默认忽略 8 项环境/显式测试 |
| npm run test:headless | 退出 0；11 项，3 事件 / 75 token | 实际 release、SQLite、隔离非空合成文件及持久化规则 |
| npm run test:desktop -- --runs 20 | 退出 0；17 项，20 次小数据首屏 P95 770.1 ms | 实际 WebView2/IPC、鼠标/键盘、十语言、原生 DPI 144、UI Automation 名称、托盘关闭/恢复、文件通知/暂停、普通用户分钟触发；辅助技术测试标志与产品默认分开 |
| 原生 Windows 显式测试 | 退出 0；COM 任务及 HKCU 独立测试键各 1 项 | 注册/查询/漂移修复/幂等删除；不修改真实开机项 |
| npm run build:desktop | 退出 0；Windows release/NSIS；Linux release/deb/AppImage | 已实际安装；当前受测包大小/校验和见安装记录，跨平台凭据记录中的旧包摘要保留历史范围 |
| npm run test:install:windows | 退出 0；12 项 | 真实 0.2.0→0.2.1→0.2.0→0.2.1 NSIS、快捷方式/IPC/任务、卸载/重装/失败中止与其他值保留；数据始终 1 事件/15 合成 token，自有集成残留 0 |
| npm run test:install:linux | 退出 0；提取 8 组/32 项；最终 FUSE + GTK 缩放 1/2 各 8 组/40 项 | rootless Podman 实际 deb 往返，AppImage 实际只读 FUSE/GUI/退出释放；中文字体、截图、devicePixelRatio 与五页无横向溢出；普通用户 CapEff=0、默认 seccomp、无网络/宿主挂载；不认证宿主完整桌面或其他发行版 |
| Qwen 0.25.0 容器真实来源 | 对照 4 项退出 0；脱敏合同 3 项通过 | 官方客户端与本地模型；默认主循环 10,228/对照 8,905 token，原生载体/CLI/模型服务/SQLite 对照，二次扫描不新增；默认后台调用的会话覆盖缺口保留 |
| Qwen 0.25.0 真实 file 导出 | 5 项独立核对退出 0 | 25 个连续多行 SDK JSON 对象，主循环与后台两条日志/span 与 CLI 合计 16,423 token 一致，含后台缓存读 3；现有 OTel JSONL 解析及权威分区尚未覆盖 |
| OpenCode 1.18.34 容器真实来源 | 默认/对照各 5 项核对；Windows/Debian 合同各 8 项通过 | 默认 API 2 次/848 token、主循环载体/应用 1 次/299；明确标题对照 API/CLI/库/应用 1 次/299，缓存读字段与重扫修订核对；仍标 latest_fallback，逐记录版本升级另验 |
| npm run test:receiver | 退出 0；8 项，自有凭据残留 0 | Claude/Codex/CodeBuddy 三源真实 release/IPC/HTTP/Windows 凭据库；安装清单与 exporter 用合成载体，不运行 Agent、不宣称真实产品导出验收 |
| 显式原生凭据测试 | Windows 两项、Debian 六项退出 0 | 跨进程读取/撤销、真实 HTTP 来源隔离；Linux 缺服务/锁定/重复/临时集合与默认别名拒绝；仅自有凭据与一次性 keyring；Windows 一次并行失败见跨平台记录 |
| Debian Clippy / Rust / 制品无界面 | 退出 0；核心 808、应用 95；ELF 与提取 AppImage 各 11 项；本轮全 workspace Clippy / OpenCode 8 项通过 | 此前跨平台凭据轮次；新增 Qwen/OpenCode 脱敏合同另验，实际 GUI/FUSE 见安装行；其他发行版仍独立 |
| macOS 凭据模块交叉检查 | 退出 0；aarch64-apple-darwin 类型检查 | 未原生链接/运行；三平台 Rust CI 已配置，未触发远端 |
| ai-maintenance quick_validate.py | 退出 0；Skill is valid | 静态格式；description 未改变，不声称模型路由评估通过 |
| npm run test:browser | 退出 0 | Edge 模拟 IPC：选区/联动、十语言、主题、筛选、分页、短采集完成交接及空闲轮询 |
| 百万 / 千万合成库完整查询 | 退出 0；未命中缓存 P95 47.05 / 133.86 ms，命中 0.024 / 0.026 ms | 各 20 次，366 日、50 模型、20 Agent；另测模型/Agent/供应商/组合筛选，最高 196.62 ms；开发机达到 200 ms |
| 首次标准化事件导入 | 退出 0；百万事件 184.5 秒、5,421 条/秒，private bytes 峰值 53.45 MiB | 当前索引的 release 内核、每批 50,000，全部子进程每 500 ms 采样；不含 WebView/全部原始载体 |
| npm run test:import | 退出 0；百万 GUI 首次回填到可见卡片/图表 668.55 秒，private bytes 峰值 371.09 MiB，符合 512 MiB 预算 | 空库、100 个隔离 Codex 根，各 10,000 条合成事件；默认保留、真实 IPC/SQLite/UI；采样 669.86 秒、1,031 样本、最多 8 进程；15,000,000 token 与重扫幂等通过，不认证其他真实载体 |
| 百万原生首屏 / 10 分钟资源 | 退出 0；20 次首屏 P95 1,260.7 ms，空闲 CPU 单核 0.244% | 连续 601.19 秒、118 样本、最多 7 进程；private bytes 均值 299.90 / 峰值 363.30 MiB，符合调整后的 350/400 MiB 预算（原 180 MiB 未达）；GPU 均值 146.24 MiB；调度循环 1,202 次；两项 IPC 与设置页按钮取消通过 |
| 百万库新增 1,000 条至界面更新 | 退出 0；20 轮 P95 718.0 ms，2 秒预算通过 | 实际来源发现/解析、SQLite、归档、IPC 与可见卡片；token 合计、缓存失效及重扫幂等通过；保留关闭仅作用于隔离副本 |
| 暖 WebView 导航空页诊断 | 退出 0；600.36 秒、118 样本；private bytes 均值 270.63 / 峰值 283.63 MiB | 默认 GPU 148.12 MiB，渲染器 41.81 MiB；仍超目标；不代替成品、全新空页或拟定硬件基线 |
| 最终文档 / 差异检查 | 退出 0；Markdown、受影响本地引用/锚点及 git diff --check | 临时产物均在根 build/，自有凭据/keyring/守护进程已回收；Skill description 未改 |

日志只在根 build/plan-execution/、build/query-next/、build/plan-finalization/、
build/plan-completion/、build/plan-continuation/、build/platform-auth/、build/install-lifecycle/ 及专项修复目录。
合成数据不证明真实 Agent 版本；模拟 IPC 浏览器和实际 IPC 分开记录。
WebView 已观测请求无外部 HTTP、无脚本异常，不能扩大为全进程网络审计。
原生首屏和资源在当前开发机测量；特定硬件要求已取消。CSS 页面缩放、Windows
原生 DPI 144 与 Debian GTK 缩放 1/2 已分别验收，其他 DPI/平台仍分开记录。
UI Automation 控件名称不代替 Narrator/NVDA 完整使用。
详细方法和不能扩大的结论见专项验收。

## 剩余条件

- 查询及增量刷新、调整后的空闲均值/峰值及百万 GUI 导入预算在开发机达标；
  保留原测量和调整依据。OS 级全部唤醒尚未测量，特定硬件要求已取消，
  其他真实版本/载体不从合成 Codex 回填推断。应用调度循环与 OS 唤醒不是同一指标。
- Windows 其他 DPI、真实辅助技术、持续远端 CI 与宿主登录/注销仍未验收；Windows
  安装往返与 Debian Podman 包/GUI/FUSE/GTK 缩放 1/2 已完成，macOS 桌面已取消要求。
- Gemini 非空样本、Qwen 多行 SDK JSON 解析/权威分区及其他版本/云端/主循环缓存
  命中、OpenCode 标题载体与逐记录版本/旧游标升级仍待验证；Windows Zed 库仍 0 行，
  其余真实版本与遥测关联/采样/安全缺口保留。
- 协作式预算覆盖载体内部及后处理，但不能强制中止 OS 阻塞读取。
- Windows 一次并行凭据失败遗留的一项自有凭据已精确回收，40 次诊断未复现，失败后回收可靠性待查；macOS 原生凭据及其他 exporter 的认证配置、真实遥测重传/采样/父子 span、跨载体关联及全进程出站审计仍未完成。

当前进度和后续条件统一在 [Plan.md](../../../Plan.md)，不保留逐轮实施历史。
