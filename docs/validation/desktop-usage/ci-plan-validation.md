# 本批三平台 CI 验收

日期：2026-10-06。用户已明确授权将本批改动提交到独立测试分支、推送并验收三平台 CI。
当前 main 工作区保持原有修改，测试 worktree、日志和下载均放根 build/ci-plan-validation/。
本次授权覆盖测试分支与 CI；合并主分支、发布、签名或公证仍需相应授权。

## 受测范围

基线为 c2c8f6f1c44f3dc9e398842d9dca2c93da76680c。本批包含真实本地样本修正规则、
旧处理位置与完整旧摘要升级、来源路由/凭据及 Windows/Linux 生命周期与辅助技术验收工具，
同步测试、脱敏 fixture、Plan、AI 规则与文档。原始数据库、WAL、会话正文、客户端配置和
临时模型均保留在已忽略的 build/，不加入提交。

流水线以 .github/workflows/ci.yml 为准：文档与前端各一个作业，windows-2022、ubuntu-22.04、
macos-15 各自 Rust fmt/Clippy/test 和 release 构建，共八个作业。Linux Rust 作业另验独立
D-Bus/临时 keyring，Windows release 作业另验隔离来源无界面采集。
提交 61223e591815a4369a85a00fa23ff1ab2819d6d5 为 macOS Rust 作业增加两个已有的
显式系统凭据测试：跨进程往返与 HTTP 撤销。只创建随机自有项，测试令牌仅通过 stdin
传给子进程，精确清理且无同步/认证 UI；该提交单独重跑完整矩阵。
测试分支 push 不触发 CI，须在推送后使用 workflow_dispatch 指定该分支。
运行必须核对分支与完整 head_sha，不能用 main 基线成功认证本批。

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

## 下载制品

三份 GitHub artifact 归档均实际下载：Windows 3,969,947 字节、macOS 4,660,995 字节、
Linux 88,869,759 字节。落盘归档 SHA-256 与 API digest 相符，ZIP CRC 通过，解包仅含
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
落盘摘要、CRC 和包报告。临时提取脚本结果路径拼接错误发生在两份内容已解包后；
修正记录写入，并对既有文件重新核对对应 ZIP 成员，不覆盖或删除。Linux 结束日志
首次下载遇到 EOF，保留诊断后一次只读重试成功，没有重跑产品测试。

## 记录同步与边界

后续提交仅同步 Plan、验收文档与 Skill reference；源码、锁文件和流水线须与上述
受测 revision 无差异，并单独检查 Markdown、本地引用及 git diff --check。
本记录以 61223e5 为远端受测 revision，不把记录提交表述为另有一次远端 CI。

本批 Windows 与 Debian 原生/容器成品结果见 [最新验收](current-acceptance.md)；
剩余真实版本、协议和宿主桌面条件统一保留在 [Plan.md](../../../Plan.md)。
