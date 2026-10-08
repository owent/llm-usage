# Claude Code 国内镜像与原生用量验收

<a id="claude-code-domestic-mirror-and-native-usage-acceptance"></a>

2026-10-07，Claude Code 2.1.197，WSL Debian/rootless Podman 5.4.2，
普通 uid 1000，Node 24；应用版本 0.2.1、Windows 11 x64。
本轮使用用户授权的智谱 Coding Plan 凭据、glm-5.3-flash 与 glm-5.3 各一次最小请求。
全部临时下载、脚本、日志和核对库在根 `build/claude-cn-20261007/`，没有提交或发布。

<a id="domestic-download-and-installation"></a>

## 国内下载与安装

主包 `@anthropic-ai/claude-code` 和原生包 `@anthropic-ai/claude-code-linux-x64`
均固定为 2.1.197，实际 tarball 从国内
[npmmirror](https://npmmirror.com/) 下载；各包的 name/version/SHA-512 integrity
与独立读取的官方 npm 元数据相符，下载字节也通过 SHA-512 检查。

| 包 | 字节 | SHA-256 |
| --- | --- | --- |
| claude-code | 19,918 | `0481de729ef296a62291f26227f76d47741536a4fd81097237448d7769b83199` |
| claude-code-linux-x64 | 77,071,446 | `42b12aa7a1d57d9f48b49acecca37643a81528ad873629e0fc5f043730621b14` |

两次国内下载分别约 0.22/6.93 秒。离线安装已核验的两个本地包，安装阶段不继承密钥。
执行包自带 `install.cjs`，生成原生 `bin/claude.exe`；Linux 使用该路径的可执行文件，
不是旧 `cli.js`。`--version` 返回 `2.1.197 (Claude Code)`，help 与安装步骤均退出 0。
二进制 245,517,112 字节，SHA-256
`f54e69cbc89b2da61a415700af7ff52a147e862517d4f1b0eecf768448cf7f83`，
与平台包内原文件一致。原生平台包与安装入口见
[官方安装说明](https://code.claude.com/docs/en/setup)。

<a id="real-requests-and-saved-records"></a>

<a id="真实请求与载体"></a>

## 真实请求与保存记录

按[智谱官方 Claude 配置](https://docs.bigmodel.cn/cn/coding-plan/tool/claude)，
将已授权密钥仅传入目标进程的 `ANTHROPIC_AUTH_TOKEN`。
用户提供的 OpenAI 兼容 Coding Plan 地址对应 Anthropic 接口
`https://open.bigmodel.cn/api/anthropic`。隔离 HOME、项目与配置目录，禁用工具、更新
及非必要遥测，命令为 `claude -p <常量 OK 请求> --model <模型> --max-turns 1
--tools '' --output-format json`。没有登录产品账户或构造认证状态。

容器内一次性本地转发器只允许指定两个模型和该供应商端点，原样转发流式响应；
不保存请求正文、响应正文、请求头或密钥，只记录路径、模型、HTTP 状态与 usage。
每次只有一个 `/v1/messages?beta=true` 请求，两次均 HTTP 200、CLI 退出 0、
`subtype=success`、`is_error=false`、`num_turns=1`。

| 模型 | 非缓存输入 | 输出 | HTTP 次数 | 原生 assistant 条目 |
| --- | --- | --- | --- | --- |
| glm-5.3-flash | 1,339 | 33 | 1 | 2 |
| glm-5.3 | 1,338 | 23 | 1 | 2 |

API 最终 `message_delta.usage`、CLI 输出和原生 `message.usage` 的正输入/输出
逐项相符。两个原生会话各 6 行，包含队列、user、两条 assistant 及 last-prompt。
assistant 自带 `version=2.1.197`，两内容块共享 `message.id`，没有 `requestId`；
不同 uuid/完成时间不拆成额外调用。总计 **2 次调用、2,677 非缓存输入、56 输出**。

API `message_start` 先报告输入/输出 0，最终 delta 才含正值；最终 delta 未报告
`cache_creation_input_tokens`，客户端却在 CLI/原生日志补入 0。
原生记录没有零桶有效性标记，因此所有默认零桶保留未知；本次 API 明确报告的
cache read=0 也不能替确认本地其他历史零值。缓存读/写、完整输入/总量及 reasoning
保持未知，正桶独立可用。记录不含供应商/渠道，provider 保持未知，不按协议或 glm
模型名猜测。CLI `total_cost_usd` 是客户端估值，不写成账单或发生时按量费用；
指定端点为 Coding Plan，渠道条件仍保留。

公开测试样本为原生文件中只含允许保留统计字段的提取版本：正文、路径和认证数据剔除，稳定 ID 哈希化，
原始文件 SHA-256 保留在 provenance。应用成品回读使用该提取版本，未声称读入原文件全部字节。
原始格式事实来自实际 CLI，未用合成 exporter 代替。

<a id="application-corrections-and-checks"></a>

## 应用修正与验证

逐 assistant 版本绑定原生规则；2.1.197 已核验，其他版本仅兼容，
缺版本的旧文档核对的格式标识独立保留。原生正桶为 reported、默认零未知，不推导缺项总量。
文档格式偏离仍拒绝，未知字段诊断保留；正文不提取。

旧成品首次回读已记录：2 次调用/2,677 输入/56 输出，却误将缓存读写记为 0、
完整总量记为 2,733、provider 固定为 Anthropic。这是统计依据缺陷，不作为验收通过。
新规则重评已消费且字节未变化的旧处理位置，只有完整旧事件摘要匹配时纠正；
正 token、模型、质量、归属及修订的真实变化仍按冲突规则处理。保留首次观察、原完成时间、
冲突和历史，同事务重算未封存汇总；读取上限耗尽时可续读，坏快照不标记规则完成。

| 检查 | 命令/入口 | 退出码与结果 |
| --- | --- | --- |
| 国内下载、原生安装及两模型 | 任务 download.py、run.py/driver.py/launch.py | 0；两个包 integrity 相符，安装/版本/help 成功，两个真实请求成功，容器残留 0 |
| Claude 初始专项 | cargo test：claude_contract、claude_gaps_synthetic、claude_incremental_v12、claude_native_contract | 0；28 项通过 |
| 统一检查 | npm run verify | 0；Rust 1,024 项通过/8 项平台条件忽略，前端 22/脚本 4 项通过，类型无错误/告警，构建通过 |
| 完整旧摘要及同批冲突 | claude_native_contract 追加同批冲突后重跑 | 0；6 项通过，包含未变游标、两种旧摘要、回滚、模型/质量/修订冲突与按读取上限续读；与统一检查重叠，不重复累计 |
| Rust 最终格式与静态检查 | cargo fmt --all --check、cargo clippy --workspace --all-targets --locked -- -D warnings | 0 |
| Windows release 可执行文件 | npm run build:desktop -- --no-bundle | 0；未重新打发行包 |
| 成品全注册表、SQLite 与原生字段提取样本 | readback.mjs：原旧库升级、新独立库各读取并重复扫描 | 0；均两调用/2,677 输入/56 输出，缓存/完整总量/provider/费用未知，文件各仅归属 Claude |
| 通用无界面回归 | npm run test:headless | 0；11 项通过，来源环境隔离 |
| 文档/本地链接/密钥检查 | npm run lint:md、check-public.py、git diff --check | 0；208 文档无 lint 问题，296 个本地链接存在，63 个公开变更文件未命中实际密钥 |

成品旧库升级没有重建数据库或改写样本；原事件键、首次观察与原完成时间逐项相符，
旧诊断保留、追加两条 parser_policy_updated。重复扫描的 data_revision 保持不变。
新库独立验收得到相同内容摘要和统计。后台仅回读本地提取版本，没有发起额外供应商请求。
此处成品与 headless 是原生可执行文件/SQLite 核验结果，未增加一次 GUI/IPC 或 Linux 包生命周期验收。

首次失败保留：此前官方源尝试触发有界超时，未取得用量；本轮查明固定版本已使用
原生平台包，旧 `cli.js` 路径不能用于该包。首次回归的能力列表期望仍只有文档核对的格式标识、
API 测试样本误读包装层，分别纠正后重跑；不把失败当作通过。

<a id="tested-scope"></a>

## 受测边界

这次核验 Linux x64 原生 Claude CLI 的智谱兼容主循环及只读允许保留的用量字段。
Anthropic 自家模型、正缓存桶、子 Agent、辅助调用、重试/失败/取消、迁移与其他版本
仍无对应非空实样，不由本次成功推导。未进行 Windows 原生 Claude 安装/GUI 验收。
自有容器已退出并删除，按标签查询没有残留。
