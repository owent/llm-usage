# 最新实施与验收

日期：2026-10-04。cwd：D:/workspace/projs/github/owent/llm-usage；
Windows 11 Pro x64 10.0.26300，Ryzen 9 9950X3D（16 核/32 线程）、约 125 GiB RAM。
Node 24.21.0/libuv 1.52.1、Rust 1.98、Tauri CLI 2.12.0、WebView2 154.0.4258.53。
版本 0.2.0；具体依赖以锁文件为准。本页替换当前验收结论，不追加历史日志。

## 当前交付

- 近 2 个自然日文案、趋势三图与总览历史/今日小时拖选；模型/Agent 分布、模型表及
  API 参考费用同范围联动，热力图和周分布置后。实际 Edge 及原生鼠标/IPC 已核对。
- Kilo 独立快照不一致不降级，逐行错误与未知版本保留；真实统计库 1 个文件的
  错误核对提示已修复，4,050 条事件及全部用量不变，见 [专项验收](trend-range-kilo.md)。

- 解析器元数据升级只在完整旧摘要一致时更新，真实内容冲突仍仲裁；本机 1414 条
  误报已消除，事件和用量保持不变，见 [专项验收](parser-conflict-fix.md)。
- 手动刷新包含独立计划来源；自动暂停覆盖启动、逐源及残留后台触发。
- 后台意图、全局期限及逐源到期持久化；手动和系统竞争请求分开合并，单写者。
  GUI 全量完成同步系统期限，逐源完成不推迟全局来源；期限保存失败保留失败状态。
- Windows COM 分数据库注册分钟任务，保存意图并核对实际路径、权限、触发与限制；
  不冒认他人任务。开机启动使用原生注册表 API，查询不启动 reg.exe。
- 定点时区独立保存、旧规则固定有效时区一次；DST 缺失时间取首个有效时刻，
  重复时间取第一次。界面提供三次预览与任务期望/实际状态、失败重试。
- 新增真实可执行文件的无界面及原生 IPC 验收；Windows 构建 CI 接入无界面回归。
  计划、设计和受影响 AI 指导已同步精简。

## 实际检查

以下均在本机执行；合成来源不证明产品实际版本已验收。详细日志只放根
build/plan-completion/、build/conflict-fix/ 与 build/trend-range-kilo/，永久结论在本页及专项验收；
没有提交、推送、触发远端 CI 或安装应用。

| 命令/方法 | 退出码与结果 | 范围 |
| --- | --- | --- |
| npm run verify | 0；脚本 3、前端 20、Rust 838 通过；类型 0 错误/告警 | Markdown、资源、fmt、全工作区 Clippy -D warnings、合同/集成及前端构建；5 项显式/环境测试未在默认测试中执行 |
| npm run test:browser | 0 | Edge 模拟 IPC：自然日、松开前无选区请求、正反向拖选、全部联动/恢复及五页、价格、十语言等；不是原生证据 |
| npm run test:headless | 0；11 项，最终 3 事件/75 token | 真实 release、真实 SQLite 与文件，隔离非空合成来源；全量完成与系统期限协调 |
| cargo test --manifest-path desktop/src-tauri/Cargo.toml --locked --bin LLMUsage system_tasks::tests::native_task_roundtrip -- --ignored --exact | 0；1 通过 | 普通当前用户 COM 注册/查询/漂移修复/幂等/删除；中文及空格路径、规范化路径别名 |
| cargo test --manifest-path desktop/src-tauri/Cargo.toml --locked --bin LLMUsage commands::win_tasks::tests::registry_roundtrip -- --ignored --exact | 0；1 通过 | 独立 HKCU 测试键的中文/空格值读写、幂等删除及清理；不改真实开机启动项 |
| npm run build:desktop | 0；NSIS 3.62 MiB | 最新 release + 前端嵌入；GUI 显示/IPC 另经下一项验收；未安装 |
| npm run test:desktop -- --runs 20 | 0；10 项，20 次首屏 P95 741.5 ms | 真实 release/WebView2/IPC；两小时拖选/单点/恢复 → 实际 SQLite，2 事件/40 token；普通用户分钟触发与 GUI 竞争；非百万事件目标 |
| Kilo 真实库副本及单写者锁下正常扫描重评 | 0；需核对文件 1 → 0，保持 active_compat | 34 条解析器元数据更新；4,050 条事件、全部指标、来源字节与诊断历史保留；重复扫描幂等 |
| python -X utf8 …/quick_validate.py .agents/skills/ai-maintenance | 0；Skill is valid | 静态格式；description/触发未改变，不声称模型路由评估通过 |
| npm run lint:md；本地引用检查；git diff --check | 0；176 Markdown、24 个修改/新增文档及 174 本地引用，无错误 | 最终文档与差异检查；临时产物均在已忽略根 build/ |
| 只读核对测试进程/任务/注册表键 | 0；均无残留，当前用户非管理员 | 仅匹配本任务目录和测试键前缀，未清理真实应用/其他任务 |

无界面 11 项覆盖：默认残留触发不扫、显式手动/重扫幂等、全量完成后系统触发等待、
全局暂停、后台意图关闭、
未到期独立规则排除、到期推进/分钟触发不反复扫、禁用/恢复来源、源字节不变、
新 schema 拒绝写入、无效数据路径退出 2。全部 SQL 修改仅针对合成验收库。

原生脚本只连接现有 WebView2 CDP，使用真实 Tauri IPC；不修改 IDE/代理配置。
合成子进程不提供 home，并显式隔离 CODEX_HOME、APPDATA/LOCALAPPDATA/TEMP/TMP；
Windows libuv 补入父 USERPROFILE 的路径由同步 spawn 辅助函数阻断，来源类别另断言。
本机损坏的 Tauri 生成资源缓存经独立 Brotli 解码确认全为零，已定向重建；
没有依据判断最初损坏原因，不把构建退出 0 当作资源显示成功。

原生 10 项覆盖实际发现/统计、两小时鼠标拖选/单点/恢复与真实范围统计、
暂停时手动独立计划/时区保持、非空交换与重复导入、
手动 CLI 与 GUI 竞争、任务注册/状态/分钟触发/删除、关闭后残留触发、离线 IPC、
暂停重启/设置恢复、执行预览及中英保存。已连接 WebView 的观测请求无外部 HTTP，
没有界面脚本异常；不把此观察扩大为全进程网络审计。
首屏从进程启动至总览卡片可见，含 CDP 连接等待，混合首次采集与暂停重启的小数据
样本；没有测百万事件或固定 4 核/16 GiB 基准。

## 保留的缺口

- 分钟触发器已真实启动；无 GUI 的系统触发、注销/升级/卸载及注册/删除失败注入仍须验收。
- 首屏合成小样本不能认证百万事件目标；全进程资源、10 分钟空闲 CPU、
  百万/千万事件与三平台持续验收仍按 M7/V20/V21 排期。
- GUI 缩放、完整键盘/可访问性、清理取消和升级回滚尚未覆盖全部验收条件。
- 真实 Agent 缺失样本、遥测重叠/采样/安全场景、文件监听、托盘/节能、
  逐源单调计时、时间预算和完整重试仍按 Plan.md 保留；未实施不写成完成。
