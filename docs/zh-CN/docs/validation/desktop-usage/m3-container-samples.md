# MiMo、Zoo 与 DSH 真实来源及成品验收

<a id="mimo-zoo-and-dsh-native-source-and-artifact-acceptance"></a>

2026-10-06，版本 0.2.1。沿用 [实现准备要求](../../design/desktop-usage/implementation-readiness.md)
与 [三源字段规则](../../design/desktop-usage/m3-runtime-samples.md)。官方客户端在独立
rootless Podman 调用真实本地模型；原始文件、模型 API、应用 SQLite 分别核对。
没有个人账户或云端付费请求，运行时无外部网络、宿主挂载，普通 UID 1000、有效能力 0、
默认 seccomp。联网下载/镜像准备与离线运行分开，不以安装或空会话核验用量。

<a id="fixed-distributions-and-actual-source"></a>

## 固定分发物与实际源码

| 产品 | 官方依据 | 实际分发物 |
| --- | --- | --- |
| Zoo Code 3.86.0 | [发布](https://github.com/Zoo-Code-Org/Zoo-Code/releases/tag/v3.86.0)，提交 `6aa9d0174a9ecae155c6c5db9134bead4b67197d` | VSIX 34,635,449 bytes，SHA256 `25c338c866bf7dacde840d8039093de867c2b0f70070d68d50031a04fe3b5f73` |
| MiMo Code 0.1.15 | [发布](https://github.com/XiaomiMiMo/MiMo-Code/releases/tag/v0.1.15)，提交 `14dfe68a1c121f859544ba810b3c308e8501bfb2` | tar 46,617,071 bytes，SHA256 `3530927a2d69eb0f809c11f663ecd6939b82c0b598ff0fbe1f82f431d764ca0c`；ELF 134,998,144 bytes，SHA256 `872728c1547b9910bb8d98b5d2fe9b1a461649b6a3e147232188f020f0ddc2e8` |
| DSH 0.2.0-rc.2 | [官方 npm manifest](https://registry.npmjs.org/@deepseek-ai%2fdsh/0.2.0-rc.2)，实际安装锁及编译模块 | SHA1 `dfc8f7e09cfa96b854d6f0cf3a973ce7f2948925`，SHA512 integrity 见下文；当前 GitHub 提交不能核验 rc.2 |

DSH integrity 为
`sha512-EAJ3gPNcVt/uv8X19PMm9NkVhWgT7xXNMk0UKCVm+IQ5rpSQOcsMUa0HWlnYYVybKMsccjcRB21vVVsaXQ6IdA==`。
逐项检查实装 session-format、JSONL persistence、LLM codec、token-meter 及 pi-ai 0.87.1，
没有将当前 GitHub 源码当成 npm 分发行为的核验结果。Zoo 核对完整消息枚举和 provider 默认值；
MiMo 核对该发布 session getUsage、Global/DB 路径及 SDK 6 归一函数。

模型是官方 Qwen3.5-0.8B GGUF BF16，1,557,662,496 bytes，SHA256
`9a7bed4041b7975e0f71fa34670d1e9025213bc92905ac0db75d36c4fa3fa623`；
本地 llama-server context 64,000，透明代理原样传递真实 SSE/usage，不构造响应或 token。
这是生成样本的环境，未作为硬件性能验收。

<a id="native-samples-and-coverage"></a>

## 真实样本与覆盖

| 来源 | 原生文件与 API 核对 | 不能扩大的结论 |
| --- | --- | --- |
| Zoo | 一条主调用，输入 6,118、输出 53、派生总量 6,171；五条原生 UI 消息、公开扩展用量回调和 API 一致 | 扩展使用真实 VS Code 1.140.0 GUI/公开 API；收到用量后取消任务，不确认任务完成。缓存/费用默认零未知，未缓存、模型、供应商、源总量未知；CLI/删除/压缩另验 |
| MiMo | 同一会话初次及续会话八条 step-finish，对应八次 API；输入 25,588、输出 1,024、总量 26,612，正非缓存 4,855、缓存读 20,733 | 两次 CLI 退出 0，八次 finish 均为 length；不确认任务成功。只采 part，不叠加 message；零缓存写/推理/费用未知，所属会话版本仍逐记录 latest_fallback |
| DSH | 两轮 headless/续会话完成；原生两条 settlement 与主循环 API 一致：未缓存 5,606、缓存读 5,568、总输入 11,174、输出 105、总量 11,279 | 标题另一次 API 为 205 token，原生未保存结果，不补造。stream usage 是副本；原生自算 total 不作 source_total。其他协议及真实 seed/retry/fail 场景未核验 |

原始根在 WSL 独立工作区 `build/plan-final-push/`：Zoo `zoo-real-1791290947`，
MiMo `mimo-real-1791291189`，DSH `dsh-real-1791291190`。Zoo 原始 UI 986 bytes，
SHA256 `402e3148ae2242dfa0840193071fc922c78bf34316c6324c779420dc95bd5fec`；
MiMo 主库 401,408 bytes，SHA256 `0a60be4932cb9b3cd543479a713885ca9ad9045fc16991f488b26681299641cd`，
WAL 1,516,192 bytes，SHA256 `ca0343b73d1c1e281ed06fdd321ca046aad640bd8e4cce6c58b43478f53726df`。
MiMo 后四条记录尚在 WAL，主库文件本身不能核验全部八条。
DSH 原始 zstd 17,575 bytes，SHA256 `bfe960a2d99eb2726c08d839a261dbe3ce645801e10b69af205e07159d20fee7`；
解压 51,114 bytes，一条 v4 header 和 27 条连续 seq 事件。

fixture 位于 core/tests/fixtures 的 zoo/real-3.86.0、mimo-code/real-0.1.15、
dsh/real-0.2.0-rc.2。各记录类型只保留允许的字段，匿名化身份；正文、请求、配置、私有路径、
凭据及原库不提交。DSH 脱敏记录保持事件类型/顺序/时间与用量，其他 payload 删减，
不声称脱敏记录可供上游完整恢复。新增样本和此前样本审计 68 文件/171 JSON 对象，
凭据键和私人路径均零。

<a id="corrections-and-targeted-regressions"></a>

## 修正与专项回归

Zoo 补实际 ask/say 枚举，未知枚举整文件拒绝；默认零纠正只匹配完整旧事件摘要。
MiMo 使用产品独立归一函数与环境路径，保护版本归属、质量、模型和真实冲突。
DSH 新增独立有界 v4 JSONL/zstd 读取，按 token-meter 的末 settlement/retry/继承
边界统计；未来生成版本拒绝，不能回退旧代，半写/超限保留游标。
默认、手工根和物理文件去重均沿完整注册表验收；完整原生头部可恢复无统计历史
的旧错误归属，转移自有文件/checkpoint 同事务，其他坏文件及诊断保留。
Kimi Work 的观察绝对根限制为当前进程 home 语境，异用户测试上下文不探测该根；
原生测试仍必须隔离实际子进程的 USERPROFILE/HOME 和来源环境。

新增 Zoo 5、MiMo 5、DSH v4 9 项设计要求检查通过，旧设计要求检查 Zoo 5/MiMo 5/DSH 4 与
OpenCode、Roo、Kimi Work、路由等既有回归通过。覆盖 canonical/legacy 完整旧摘要、
旧游标/处理位置、重复、并行、事务回滚、同批冲突、保护字段、缺失/坏桶、继承与
重试、快照回退、zstd window/展开上限和未知事件；边界构造明确为合成回归。

<a id="artifact-acceptance-for-this-batch"></a>

## 本批成品验收

本批命令、退出码与失败日志均保存在根 build/plan-final-push；历史首次失败保留。

- Windows `npm run verify` 退出 0：Rust 1,007（核心 906、应用 101，默认忽略 8），
  前端 21、脚本 4；当时 Markdown 201 文件，Svelte 无错误/告警，fmt、Clippy
  `-D warnings`、资源及前端构建通过。后续文档另验。
- Debian Rust 1,004（核心 906、应用 98，默认忽略 9）、原生 Clippy、release/deb/
  AppImage 构建退出 0。
- Windows NSIS 12 项、无界面 11 项、WebView2/IPC 17 项/20 次启动及接收器
  8 项通过；当前首屏 P95 751.36 ms，自有凭据及安装集成残留 0。
- Linux 根 `1791295818331`：deb 安装/升级/回滚/卸载/重装/清除及 FUSE/GTK/
  Orca 共 9 组/47 项退出 0；十语言五页当次焦点与语音均核对。仅自有容器
  加 SYS_ADMIN，普通用户能力 0、默认 seccomp；FUSE 挂载和退出释放均核对。
  不核验宿主登录/注销、完整辅助技术、各语言发音或物理音频。
- 最终 Markdown 202 文件、44 个受影响 Markdown/262 条本地引用、fmt、Skill
  quick_validate 与 git diff --check 均退出 0；Skill 检查显式使用 Python UTF-8。
  只读清理核对无 Podman 容器残留，旧自有未启动 Roo 回读容器已移除。

| 本批成品 | bytes | SHA256 |
| --- | --- | --- |
| Windows exe | 10,106,880 | `faec1e22d20756c41a301bc5727e68ed9b7325bd53d76ff3a183ecab2169b56c` |
| Windows NSIS | 3,985,629 | `cba1db1a17432b3ab56625f3d55d7b5b692b303c685ee8139e2a7e9b1cbaa7ab` |
| Linux deb | 5,527,382 | `4afb1bd7958983de49a121966055a297f5ee8e50432e70b3c6511db71bd22854` |
| Linux AppImage | 111,204,856 | `c08b20ad16569ce1225360426ef515b6fc7c1bbfb5ee78f1e95ffbb01501bc99` |

Linux `m3-readback-1791295919` 与 Windows `m3-windows-1791296331720` 均用上阶段
OpenClaw 成品建立旧库，再升级本批成品并重复读取原始文件。旧包 Zoo 0/MiMo 8/
DSH 0，新包为 1/8/2；三个物理文件/checkpoint、无冲突、健康 ok。旧事件的身份/
修订/首次观察、两条旧审计保留，重复没有新增或更改已提交事件；原始主库/WAL/
JSON/zstd 摘要不变，SHM bookkeeping 独立排除。Windows 验收的是应用读取同一
Linux 客户端文件，未核验 Windows 官方客户端。

此前十五源 Aider、AtomCode、Continue、Crush、gajae-code、Goose、Hermes、jcode、
Junie、OpenCode、Qwen、Roo、Xum、Cline、OpenClaw 同包回读全部退出 0，汇总日志根
`m3-prior-readbacks-1791296018`；数值、未知分项、旧库升级及既有覆盖缺口保持。

<a id="first-failures-and-corrections"></a>

## 首次失败与纠正

- DSH 开发期重复入口/文件 probe API、TokenQuality 缺少 Eq、测试错误 SQL 表名/
  期望计数等失败保留，按实际接口修正。完整检查发现 Option::is_none_or 高于
  项目 MSRV 1.77.2，改用兼容写法后通过，未提高最低版本。
- DSH 原生 type=session 被 Pi 手工发现抢占的全注册表失败已修复；只有完整
  原生指纹与无统计历史才恢复，其他文件和历史保护测试通过。
- Linux 首次回读只复制 MiMo 主库而漏 WAL，实际只能读四条，且 SQLite 创建空
  WAL 导致摘要断言失败；完整复制主库和 WAL后八条与 API 一致。临时脚本 Agent 标识
  误写 dsh 而非 deepseek-harness 的断言失败独立保留。
- Windows 临时脚本最初广播手工根，产生旧 Cline/其他产品误识别诊断，TMP 路径
  无效导致 SQLite 提交失败；独立单变量对照确认 TMP 缺失时退出 1、创建目录后
  退出 0 并提交八条。改用产品已核验的隔离来源路径及有效临时目录。
  随后遗漏仓库隔离函数，libuv 自动补入 USERPROFILE 并扫描额外默认来源；该次
  结果作废，未提交提取数据，修正启动隔离后只发现三个测试来源。
- 原生 GUI 第四次启动选中 6669，Node fetch 在连接前拒绝，出现 CDP 超时。
  本机 fetch 复现 bad port，且 [Fetch 标准](https://fetch.spec.whatwg.org/#port-blocking)
  明确阻止此端口。共享 HTTP 端口预检现在先用同一 fetch 完成真实本地 204 请求，
  有界重选被拒端口，关闭自有 server 后用于 CDP；二十轮预检、七个脚本语法及
  真实二十次 GUI 启动通过。没有用后续通过替代首次失败说明。

本批未进行 macOS 桌面或特定硬件验收。其余客户端、真实多版本/协议和宿主/
远端 CI 条件继续在 [Plan.md](../../../Plan.md)，不将本批完成写成全部里程碑完成。
2026-10-06 只读确认 [基线 CI 37330787284](https://github.com/owent/llm-usage/actions/runs/37330787284)
在 2026-10-05 对 HEAD c2c8f6f 的八作业已全部成功，三平台制品存在；本批未提交改动
不在该 revision 中。现有五个 Action tag/branch 已逐项查询官方仓库引用存在。
用户现已授权独立测试分支提交、推送和三平台 CI，结果统一见 [本批 CI](ci-plan-validation.md)。
