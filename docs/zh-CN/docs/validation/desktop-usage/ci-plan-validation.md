# 本批三平台 CI 验收

<a id="three-platform-ci-acceptance-for-this-batch"></a>

日期：2026-10-06。用户已明确授权将本批改动提交到独立测试分支、推送并验收三平台 CI。
当前 main 工作区保持原有修改，测试 worktree、日志和下载均放根 build/ci-plan-validation/。
本次授权覆盖测试分支与 CI；合并主分支、发布、签名或公证仍需相应授权。

<a id="tested-scope"></a>

## 受测范围

基线为 c2c8f6f1c44f3dc9e398842d9dca2c93da76680c。本批包含真实本地样本修正规则、
旧处理位置与完整旧摘要升级、来源路由/凭据及 Windows/Linux 生命周期与辅助技术验收工具，
同步测试、脱敏 fixture、Plan、AI 规则与文档。原始数据库、WAL、会话正文、客户端配置和
临时模型均保留在已忽略的 build/，不加入提交。

61223e5 的流水线以 .github/workflows/ci.yml 为准：文档与前端各一个作业，windows-2022、ubuntu-22.04、
macos-15 各自 Rust fmt/Clippy/test 和 release 构建，共八个作业。Linux Rust 作业另验独立
D-Bus/临时 keyring，Windows release 作业另验隔离来源无界面采集。
提交 61223e591815a4369a85a00fa23ff1ab2819d6d5 为 macOS Rust 作业增加两个已有的
显式系统凭据测试：跨进程往返与 HTTP 撤销。只创建随机自有项，测试令牌仅通过 stdin
传给子进程，精确清理且无同步/认证 UI；该提交单独重跑完整矩阵。
测试分支 push 不触发 CI，须在推送后使用 workflow_dispatch 指定该分支。
运行必须核对分支与完整 head_sha，不能用 main 基线成功确认本批结果。

<a id="results"></a>

## 执行结果

测试分支为 [codex/plan-validation-20261006](https://github.com/owent/llm-usage/tree/codex/plan-validation-20261006)，
受测提交为 819ca6eda6a6f3e88bc2e62e08f2b8ea15c0c766，共 223 文件。
提交前 Markdown 203 文件、本地引用 269 项、git diff --check 通过；新增样本 68 文件/
171 个 JSON 对象扫描未发现凭据字段或未脱敏私人路径。
[运行 37485522153](https://github.com/owent/llm-usage/actions/runs/37485522153)
已以 workflow_dispatch 执行，分支及完整 head_sha 均已回查相符，八作业全部成功。
Windows 默认 Rust 测试 1,007 项通过、默认忽略八项，三平台 release 制品均生成。
[运行 37486581699](https://github.com/owent/llm-usage/actions/runs/37486581699)
对应上述 macOS 增补提交，也已八作业全部成功。以下数量从本轮各作业日志核对，
默认测试与显式原生凭据分别统计，不重复计数。

| 作业 | 实际结果 |
| --- | --- |
| 文档 | Markdown 203 文件，0 问题 |
| 前端 | 类型 0 错误/告警；脚本 4、UI 单元 21 与 Playwright Chromium 浏览器检查通过；资源/构建成功 |
| Windows Rust | fmt/Clippy 通过；默认测试 1,007，0 失败，默认忽略 8 |
| Linux Rust | fmt/Clippy 通过；默认测试 1,004，0 失败，默认忽略 9；另六项独立 D-Bus/keyring 原生验证通过 |
| macOS Rust | fmt/Clippy 通过；默认测试 1,003，0 失败，默认忽略 6；另 Keychain 跨进程往返/真实 HTTP 撤销两项通过 |
| Windows release | release/NSIS、11 项真实无界面采集回归、报告与上传成功 |
| Linux release | release/deb/AppImage、报告与上传成功 |
| macOS release | release/.app 归档、报告与上传成功 |

macOS 桌面和特定硬件不在本轮范围。

<a id="downloaded-artifacts"></a>

## 下载制品

三份 GitHub artifact 归档均实际下载：Windows 3,969,947 字节、macOS 4,660,995 字节、
Linux 88,869,759 字节。下载的归档 SHA-256 与 API digest 相符，ZIP CRC 通过，解包仅含
下列四个包及三份 bundle-report.json。每份报告 revision 均为
61223e591815a4369a85a00fa23ff1ab2819d6d5，包大小及重新计算的 SHA-256 与报告一致。

| 平台/包 | 字节 | SHA-256 |
| --- | ---: | --- |
| Windows x64 NSIS | 3,984,826 | 8bf368b7ae1a0e494e6851ccbb7e65be2ab1fa1cdc6a1c0a1a309c92f69feb83 |
| Linux x64 deb | 5,542,774 | 759ce028ffab4e1359b2ef661b7b4d8cccebaffc23adbdf7539d32511cafbec4 |
| Linux x64 AppImage | 84,007,416 | 5729705271795aab9023979007b87bd6b18205a5c1620e90fd18e6e83f116780 |
| macOS arm64 .app.tar.gz | 4,661,581 | 95def48f9a4399d9f3e23dd89fff6712a6b1c9cd6874ede00a0a9027a078393e |

这些是 CI 制品，未执行发行签名、公证或 GitHub Release 发布。本次下载核验与既有本机 GUI/安装验收分别记录，
不将包完整性检查当作本机安装或 macOS 桌面通过。

<a id="first-failures-and-recovery"></a>

## 首次失败与恢复

创建深目录 worktree 首次遇到 Windows Git 长路径限制，退出 128；实际回查未留下
worktree，只创建了指向原基线的测试分支。仅对恢复命令启用 core.longpaths=true 后
复制、暂存、提交与推送成功，main 分支及 HEAD 均保持原值，未修改全局 Git 配置。
GitHub CLI 在整轮未结束时拒绝读取已完成作业日志；改按
[官方 job logs 接口](https://docs.github.com/en/rest/actions/workflow-jobs#download-job-logs-for-a-workflow-run)
只读获取。CLI 的 ANSI 输出保护仅对捕获到文件的调用放行，显示前过滤控制序列；
执行中 Windows 作业接口返回 404，未据此报告产品测试失败或重跑。
GitHub CLI 制品下载在 300 秒后超时，未接受任何解包结果。改用官方 archive 接口，
认证与签名 URL 只在内存，逐归档流式写入根 build/ 并记录字节进度；Windows/macOS
分别 36/38 秒，Linux 实际 502 秒，超过最初限时。三份归档完成后核对 API 摘要、
本地文件摘要、CRC 和包报告。临时提取脚本结果路径拼接错误发生在两份内容已解包后；
修正记录写入，并对既有文件重新核对对应 ZIP 成员，不覆盖或删除。Linux 结束日志
首次下载遇到 EOF，保留诊断后一次只读重试成功，没有重跑产品测试。

<a id="record-updates-and-scope"></a>

## 记录同步与边界

后续提交仅同步 Plan、验收文档与 Skill reference；源码、锁文件和流水线须与上述
受测 revision 无差异，并单独检查 Markdown、本地引用及 git diff --check。
本记录以 61223e5 为远端受测 revision，不把记录提交表述为另有一次远端 CI。

本批 Windows 与 Debian 原生/容器成品结果见 [最新验收](current-acceptance.md)；
剩余真实版本、协议和宿主桌面条件统一保留在 [Plan.md](../../../Plan.md)。

## Draft Release 发布

2026-10-08，用户授权由 tag 触发草稿发布、覆盖 Release 元数据和同名附件，并删除后
重建 v0.2.1。旧 annotated tag 指向 bda55d3b49959e9390c5d5c48fff91c3574b8724；
重建的 tag 和受测流水线/源码指向 a16ae4e74a7b3a9f2365b1991500f12555b66407。

本机 Windows x64 检查使用 Node 24.21.0、npm 12.0.2 和 PowerShell 7。
以下命令均退出 0：node --test desktop/scripts/*.test.mjs（5 项）、临时流水线提取/检查
脚本（10 项不联网的合成发布用例）、npm run lint:md（451 文件）、npm run check:docs
（199 组仓库文档、21 组指南、32 个 Astro 文件，无诊断）、npm run test:docs（31 项）
及 git diff --check。本机模拟与下述真实 GitHub 发布分开记录。
前一次 main CI 和本机 VS 探测测试因空 Measure-Object 结果在严格模式下没有 Sum 属性而失败；
修复对空文件数组单独处理，隔离测试确认文件数量/字节为零且没有警告。

[tag 运行 37749146960](https://github.com/owent/llm-usage/actions/runs/37749146960)
首次执行九作业全部成功，包含三平台构建及草稿发布。
[main CI](https://github.com/owent/llm-usage/actions/runs/37749146944) 与
[文档流水线](https://github.com/owent/llm-usage/actions/runs/37749147002) 也均通过。
草稿 Release ID 为 406615500，名称/tag 为 v0.2.1，目标提交与受测源码一致；
共七个附件：四个包和三份平台报告。发布作业在上传前核对包的 revision、大小和摘要，
上传后核对 GitHub 附件的大小/摘要。另独立下载三份报告，四个发布包的 API 大小/SHA-256
均与报告相符。

| 平台/包 | 字节 | SHA-256 |
| --- | ---: | --- |
| Windows x64 NSIS | 4,046,322 | 5c51b575d14cafe8744ab08915eb701445ec7a2dca8ebaba7169644b0788e6f3 |
| Linux x64 deb | 5,631,732 | 6dfe3750cb9ea1781db706b1f67366586f82a783d3ad0bebdff0cf82f9188d9c |
| Linux x64 AppImage | 84,097,528 | 9508a3e65dcf8217e9750e89df9c4ab9db8a15096298f8ef4897d09f691f0b9e |
| macOS arm64 .app.tar.gz | 4,740,489 | 5b01cfee42fcf95f367a949b723c0393cc4ec0f996ef4e4f60155df9ff3af9ca |

首次覆盖测试的标记 PATCH 漏传 tag_name，GitHub 将原草稿的 tag 改为自动生成的
untagged 名称，因此 attempt 2 按 v0.2.1 查询时未找到原草稿，新建了额外草稿 406616857。
核对其完整源码/附件后，仅删除本次测试创建的额外草稿。恢复原草稿时显式传入 tag_name
及标记标题/正文，回查确认 v0.2.1 后重跑发布作业。attempt 3 成功：原 Release ID 保持，
标题/正文恢复，七个附件 ID 全部更新，各自大小/摘要与首次上传一致，最终仅一个 v0.2.1
草稿。流水线自身的创建/更新请求均始终包含 tag_name。

状态查询遇到网络 EOF，CI 作业没有因此失败；通过只读 jobs 查询恢复。
完成日志保留在已忽略的 build/github-release/，显示前去除 ANSI。
本机 npm 12 首次拒绝锁定的镜像 tarball；仅对安装命令使用 --allow-remote=all，
禁用安装脚本后成功，未修改锁文件或全局配置。文档/注释摘要已审阅并同步。
发布保持草稿，不建立签名、公证或新的 GUI/安装验收结论；后续仅记录提交保持
受测源码/流水线不变。
