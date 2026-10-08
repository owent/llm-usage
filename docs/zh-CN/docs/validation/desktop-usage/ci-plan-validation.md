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

## Portable 版本 v0.2.2

2026-10-08，用户授权提供 Windows/Linux/macOS 的 x64 与 arm64 portable 归档，
不提供 Debian 包，提高 zstd 压缩率，并创建 v0.2.2 tag 发布。
根目录与 desktop 的 npm 清单/锁文件、Tauri 配置、两个 workspace Cargo 清单及
Cargo.lock 中本项目自身的条目均已统一为 0.2.2。annotated tag 指向
39e74d09fa2b4e300dca3f7d0ba2edf4b6e95ba2；依赖版本仍由原锁文件确定。

六种构建分别使用原生 windows-2022/windows-11-arm、ubuntu-22.04/ubuntu-22.04-arm、
macos-15-intel/macos-15 runner 对应 x64/arm64。每个归档包含带版本号的目录、
可执行程序、完整资源及双语解压/运行说明。Windows 运行 LLMUsage.exe；
Linux 包含解出的完整 AppDir，以 AppRun 启动，无需 FUSE 挂载；macOS 保留完整
LLMUsage.app。Windows 11 使用系统 WebView2，Linux 保留 Ubuntu 22.04/glibc 2.35
构建基线及兼容桌面条件，macOS 保留兼容系统 WebKit/Gatekeeper 条件。
数据仍在既有用户目录；移动程序前须关闭其自有定时任务。

打包先生成 tar 文件，再执行 zstd -19 -T2 --long=27，随后 zstd -t 校验，并重新
解压核对全部文件摘要、符号链接与 Unix 权限。可执行文件头必须符合原生矩阵架构。
六个解压成品均通过实际无界面 SQLite/采集检查；两个 Linux 包还在隔离来源/数据环境下
通过原生 Xvfb/WebKitWebDriver GUI/IPC 采集及五页导航。
Linux 报告核对 370 个树条目，macOS 四个，Windows 两个。

本机 Windows x64 使用 PowerShell 7、Node 24.21.0、npm 12.0.2、Rust 1.98.0、
锁定的 Tauri CLI 2.12.0 及 zstd 1.5.7。以下命令退出 0：
node --test desktop/scripts/*.test.mjs（13 项）、提取实际流水线脚本的检查器
（14 项合成创建/更新/拒绝用例）、npm run lint:md（451 文件）、npm run check:docs
（199 组仓库文档、21 组指南、32 个 Astro 文件，无诊断）、npm run test:docs（31 项）、
npm --prefix desktop run check、Cargo fmt 和 git diff --check。
直接运行 desktop Tauri CLI 构建真实 Windows 0.2.2 程序/NSIS，回查版本资源为 0.2.2。
其 portable 包通过重新解压与无界面检查，native-smoke.mjs 通过 19 项实际
WebView2/IPC/UI 检查及三次启动测量。同一份 10,283,520 字节的 tar 以 zstd 级别 3
压缩为 4,843,925 字节，级别 19 为 4,113,788 字节，在不裁剪资源的情况下减少 15.1%。
fixture 测试和流水线模拟与原生成品执行、真实发布分别记录。

首次 main [运行 37757778777](https://github.com/owent/llm-usage/actions/runs/37757778777)
对应 9d14217a5ce36c74dda90f61507e076225826c66，两个 Linux 目标均编译成功，
但重新解压的权限比较失败：默认 tar 解压受 umask 影响，移除了组写权限。
修复在 Unix 解压时使用 -p，并增加 775/664 fixture 回归。
tag 对应受测源码的 [main 运行 37759068722](https://github.com/owent/llm-usage/actions/runs/37759068722)
全部 11 个必需作业通过，包括文档/前端、三个 Rust 和六个构建；仅 tag 发布作业跳过。
Linux GUI 与原生凭据检查通过；完整 main 运行成功后才创建 tag。

[tag 运行 37761919575](https://github.com/owent/llm-usage/actions/runs/37761919575)
恢复 macOS x64 制品上传失败后整体成功；相同源码的全部 12 个逻辑作业均有成功结果。
草稿 [v0.2.2](https://github.com/owent/llm-usage/releases/tag/untagged-564360182cf0e15d7e8a) 的 Release ID 为 406718916，名称/tag 精确为
v0.2.2，目标提交与受测源码一致，共 13 个附件：六个 portable .tar.zst 归档、
保留的 Windows x64 NSIS 安装器及六份平台报告。没有 deb、原始 AppImage 或 gzip 归档。
发布作业上传前对实际包计算摘要，上传后核对 GitHub 全部大小/摘要；另独立下载六份报告，
核对其自身摘要及七个发布包的 API 大小/SHA-256。

覆盖测试显式传入 tag_name，写入受控标题/正文标记并回查。只重跑发布作业后，
原来唯一的草稿 ID 保持，标题/正文/目标元数据恢复，13 个附件 ID 全部替换，
每个文件大小和摘要保持一致。最终发布 attempt 为 3，
替换后再次核验了独立下载的报告。

| 包 | 字节 | SHA-256 |
| --- | ---: | --- |
| LLMUsage-0.2.2-windows-x64-portable.tar.zst | 4,117,621 | 0eca2969168447cb2c1e755b2c89626d4f7a44734778241bbd788e7c01c385d1 |
| LLMUsage_0.2.2_x64-setup.exe | 4,046,000 | fdbcf85d1ed6ff73de55336669d5f427d8430fb2c72e847c94f8b9961c37b91a |
| LLMUsage-0.2.2-windows-arm64-portable.tar.zst | 3,951,219 | ad91aa491272f2c13ac0f4b90489c3929f736d152a4ab584d8f30f1fc7f590d3 |
| LLMUsage-0.2.2-linux-x64-portable.tar.zst | 72,899,343 | 67f2f64b5e054b6471d5daf907b7b1bc6d916985cf3e0b4233ac7d467c06b703 |
| LLMUsage-0.2.2-linux-arm64-portable.tar.zst | 71,499,124 | 9151080f8e2fb18207b8823564e3c870b1da777b065d1032d1554316ba1e4eab |
| LLMUsage-0.2.2-macos-x64-portable.tar.zst | 4,228,412 | 4d19e09a3f354a6b416b0345e0262c2d0690d96dbab1e274704914363d5ba1b9 |
| LLMUsage-0.2.2-macos-arm64-portable.tar.zst | 3,965,860 | de180b5e1dc24b76ca3b8451d5bf9de533e7e2f633ade0bd731b993f5f82f114 |

本机首次通过根 npm 脚本转发 --bundles 参数被 npm 12 拒绝；直接 desktop Tauri CLI
执行成功。旧 0.2.1 与新 0.2.2 NSIS 构建输出最初造成选择歧义，现要求精确匹配
当前版本的文件名，保留旧输出。错误日志路径在执行前已改为仓库根 build/。
Windows sandbox 启动器出现 CreateProcessAsUserW 错误 5，改用获准路径执行相同检查。
首次 tag 运行的 macOS x64 归档与无界面检查通过，但 GitHub CreateArtifact 遇到
DNS ENOTFOUND；等待整次运行结束后重跑已完成的失败作业，未改变受测源码。

原生 Windows x64 GUI、Linux x64/arm64 CI GUI 与六种原生无界面结果只适用于各自
实际环境，不建立 Windows arm64/macOS GUI、新安装生命周期、其他 Linux 发行版或
签名/公证验收结论。Release 保持草稿。任务日志、fixture 和原始报告在根目录已忽略的
build/portable-release-022/；既有原生 smoke 的隔离证据位于根 build/plan-completion/native/。
后续验收记录提交保持源码、流水线和锁文件与 tag 相同。
