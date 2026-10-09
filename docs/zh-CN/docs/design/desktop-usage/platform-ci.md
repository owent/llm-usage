# 平台、GitHub CI 与 WSL 验证

<a id="platforms-github-ci-and-wsl-validation"></a>

状态：2026-10-06 已按用户授权提交并推送独立测试分支，两轮三平台 Rust/release 及
公共检查各八作业成功；macOS 两项显式原生凭据通过。三份归档/四个包的下载摘要及
源码 revision 已核对，结果见 [本批 CI](../../validation/desktop-usage/ci-plan-validation.md)。
用户已确认 Windows 11 x64 首发，
同时保留 macOS/Linux 的 GitHub CI。本文件保留验收要求；实际执行情况见验证记录。
用户已确认按本方案推进：runner/工具链/Linux 基线在 M0 固定，WSL/WSLg 可用性在 M0 探测；
当前发行范围包含 Windows、Linux 和 macOS 的 x64、arm64 便携包。
这些属于实施核验，不再作为开工前待用户确认事项。
2026-10-05 验收范围调整：不要求 macOS 桌面或特定硬件；保留 macOS 构建与 Rust CI。
Linux 实际 GUI/安装生命周期接受 WSL/Debian 内的独立 Podman 环境，
记录发行版、镜像 digest、用户、显示与沙箱条件；不扩展为宿主登录/完整桌面支持。
Windows 本机安装/升级/回滚/卸载已授权；新增设计说明见 [安装生命周期](installation-lifecycle.md)。

<a id="platform-and-artifact-matrix"></a>

## 平台与制品矩阵

| 平台 | 首发定位 | CI 初始目标 | 交付和验证结果 |
| --- | --- | --- | --- |
| Windows x64 | Windows 11 首发桌面目标 | windows-2022；x86_64-pc-windows-msvc | 便携 exe 归档及现有 NSIS 安装包；原生 IPC/安装/任务验收仍分别记录 |
| Windows arm64 | 必需的便携发行目标 | windows-11-arm；aarch64-pc-windows-msvc | 原生 release exe 归档及解压后的真实无界面测试 |
| Linux x64 | 必需的便携发行目标 | ubuntu-22.04；x86_64-unknown-linux-gnu | 解压后的 AppDir 归档及原生无界面测试；Release 不包含 Debian 包 |
| Linux arm64 | 必需的便携发行目标 | ubuntu-22.04-arm；aarch64-unknown-linux-gnu | 原生 AppImage 构建、解压后的 AppDir 归档及原生无界面测试 |
| macOS x64 | 必需的便携发行目标 | macos-15-intel；x86_64-apple-darwin | 应用包归档及解压后的真实无界面测试 |
| macOS arm64 | 必需的便携发行目标 | macos-15；aarch64-apple-darwin | 应用包归档、原生无界面测试及现有 Rust/凭据检查 |
| 本地 WSL 2 Linux x64 | 开发及已授权的容器验收 | 记录实际发行版与工具链 | 构建、测试、打包单独记录；独立 Podman 的实际 GTK/WebKit GUI 与软件包生命周期按本轮范围验收 |

使用版本化 runner 标签和明确 Rust target，并核对 runner 与二进制架构；
不使用浮动 latest 隐式改变架构。发布需要全部六种组合，缺少任何归档都会阻止更新草稿。
GitHub Windows runner 的构建结果不等于 Windows 11 用户环境通过。
归档可下载和原生无界面测试不建立完整桌面支持结论。
未通过项必须在结果中可见，不能长期 continue-on-error 掩盖失败。

依据：[GitHub runner 列表](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)、
[Tauri 多平台流水线](https://v2.tauri.app/distribute/pipelines/github/)、
[平台依赖](https://v2.tauri.app/start/prerequisites/)。Linux 最低运行系统需按实际链接依赖在 M0 确认，
不能把 CI 镜像版本当作已经验证的全部 Linux 支持范围。

<a id="github-application-ci-contract"></a>

<a id="github-application-ci-requirements"></a>

<a id="github-ci-合同"></a>

## GitHub CI 要求

三平台作业已存在；M7 持续验收须有实际运行结果。任务触发为 PR、主分支 push、
tag push 和手动执行。用户已授权自动发布 Draft Release，并删除后重建 v0.2.1
验证发布，并授权通过 v0.2.2 发布版本 0.2.2 的便携包。2026-10-09 用户授权升级依赖、
更新到 0.3.1、创建 v0.3.1 tag 并推送驱动发布，同时纳入 pnpm 配置并同步锁文件。
其他远端写入仍需相应授权。

1. 公共检查：Markdown/本地链接、前端类型和单元测试、Rust fmt/clippy；命令从实际锁文件确定。
2. OS 矩阵：在每个原生 runner 运行领域/适配器 fixture、真实临时 SQLite、调度器、路径和锁测试，
   构建同一源码的前端与 Rust release。外部 Agent 不需安装，不读取 runner 个人目录。
3. 制品：六种便携归档及 Windows x64 安装包，附源码 revision、OS/arch、依赖版本、大小和校验和。
   打包失败与测试失败分别报出，不能用 cargo check 代替成品构建。
4. 桌面集成：Windows 使用实际 WebView2 CDP，Linux 使用实际 GTK/WebKit WebDriver 与
   Tauri 自动化环境开关。发布包不增加常驻监听；macOS 桌面已取消本轮要求。
5. 独立 release 资源任务测包体和性能；通用托管 runner 时序只作回归信号，不能代替固定机器 P95。
6. 构建依赖下载可以联网；统计运行和验收禁止出站访问用量/计费服务。fixtures 全部合成或脱敏，
   不上传个人数据库/原始会话。离线用例覆盖有遥测开关时的行为。

检查入口包括 `cargo fmt --all --check`、
`cargo clippy --workspace --all-targets --locked -- -D warnings` 覆盖核心 crate 与测试目标；
前端作业运行 TypeScript 纯逻辑测试和 Playwright 浏览器回归。后者使用合成 IPC 数据，
覆盖主题、时区、快速筛选、用户隔离和刷新，不计为第 4 项原生桌面验收。
六种构建都从解压目录执行真实可执行文件的 `test:headless`，隔离合成来源并检查来源类别。
Rust fmt/Clippy/test 作业分别在三个原生 runner 执行，平台存储实现纳入各自编译。
Linux 另运行 `test:credentials:linux`，使用独立 D-Bus 与一次性加密 keyring；
凭据只在 stdin 传给测试子进程。macOS Rust 作业另显式运行系统凭据跨进程往返与
真实 loopback HTTP 撤销，仅创建并精确回收随机自有项，不启用同步或认证 UI。
Windows 原生凭据在本机显式执行，默认忽略项不能当作系统存储已验收。
本机 `test:desktop` 经 WebView2 CDP 检查真实 IPC，不在发布包增加监听代码。
最新本机结果及远端 CI 缺口见 [最新验收](../../validation/desktop-usage/current-acceptance.md)。

矩阵使用 fail-fast=false 留下全部结果；每作业有超时，重跑只针对已定位的临时基础设施故障。
缓存键包含 OS、架构、Rust/Node 版本和锁文件摘要，隔离不同 target，不缓存真实 Agent 数据。
应用 Action 使用完整提交 SHA 固定版本；PR 保持只读权限。
仅 tag push 的发行作业在公共检查和全部平台构建成功后，通过 GitHub 短期 token
取得 contents: write 权限；不在特权 pull_request_target 中执行 PR 代码。
CI artifact 与 GitHub Release 分开。tag push 按 tag_name 创建或更新同一个草稿，覆盖
名称、正文和目标提交；同一 tag 的发布串行执行。更新前核对远端 tag 的当前提交，
防止旧构建覆盖重建的 tag；查找草稿时读取全部 Release 分页。
[上传 Action](https://github.com/xresloader/upload-to-github-release) 使用明确的 release ID
和 overwrite: true 替换同名附件，重复运行覆盖同一草稿的元数据和附件。
平台报告使用不同附件名，发布前核对包的 revision、大小和 SHA-256，上传后再次核对附件。
此流水线不建立签名、公证或桌面安装验收结论。
上传仅包含六种 `.tar.zst` 归档、现有 Windows x64 NSIS 安装包及不同名称的大小/校验和报告。
不上传 Debian 包或原始 AppImage。

## 便携归档要求

文件名包含版本、系统和架构：`LLMUsage-<version>-<windows|linux|macos>-<x64|arm64>-portable.tar.zst`。
每个归档包含同名目录，内含 Windows exe、带 `AppRun` 的 Linux AppDir 或 macOS `.app`，
以及完整的中英文使用说明。便携指解压后无须安装应用；仍使用现有的用户数据目录和
系统凭据存储，移动归档不会迁移这些数据。保留完整资源、执行权限和符号链接，不裁剪语言。

Windows 使用系统 WebView2 Runtime，Windows 11 通常自带；本包不包含固定版本离线浏览器运行时。
macOS 使用系统 WebKit，流水线不进行发行公证。Linux 两种架构都使用 Ubuntu 22.04 构建，
归档 AppImage 解压后的依赖，避免 FUSE 和第二层压缩容器；仍需要兼容的 glibc（2.35 或更高）、
桌面显示环境，以及可选凭据集成所需的 D-Bus/Secret Service 等系统设施。
其他发行版需独立验证；包内说明标明这些条件，不承诺适用于所有运行环境。

打包脚本先生成完整 tar，使用 `zstd -19 -T2 --long=27` 压缩并运行 `zstd -t`，
解压到全新的校验目录后比较全部文件摘要、符号链接目标和 Unix 权限。
核对解压后的 PE/ELF/Mach-O CPU 类型是否匹配预期架构，再从解压目录运行真实无界面/SQLite 测试。
Linux 两种架构还在 Xvfb 与独立 D-Bus 下，通过原生 WebKitWebDriver 启动解压后的 AppRun，
检查真实窗口、五个页面、IPC 和合成 SQLite 结果。
仅通过校验的归档生成报告并进入发布。构建或压缩失败、架构错误、归档损坏、平台报告缺失都会阻止发布。
重复打包在校验后替换同名归档；重复发布保留草稿身份并替换同名附件。
回滚通过修复源码并重新运行已授权的 tag 流水线完成；tag 移动后不得使用旧构建。

依据：[Tauri ARM AppImage](https://v2.tauri.app/distribute/appimage/#appimages-for-arm-based-devices)、
[AppImage 解压](https://docs.appimage.org/user-guide/run-appimages.html#extract-the-contents-of-an-appimage)及
[Tauri WebView2 条件](https://v2.tauri.app/distribute/windows-installer/#webview2-installation-options)。

桌面测试依据：[Tauri WebDriver](https://v2.tauri.app/develop/tests/webdriver/)。
Linux 本轮直接使用 WebKitWebDriver 原生 capabilities，启动真实打包应用；
它与 tauri-driver 的 WebKit 参数映射一致。macOS 构建/凭据原生测试与桌面验收分开。
不依赖付费平台服务完成基础构建。

<a id="documentation-ci-and-pages"></a>

## 文档 CI 与 Pages

明确授权的文档发布使用 .github/workflows/docs.yml 独立流水线，按[文档要求](../documentation-site.md)执行。
使用已核验的 Action 固定 revision、Node 24、双语内容和注释检查、Astro 检查、单元测试、
生产输出校验及浏览器回归，全部通过后上传静态制品。文档变化自动触发；源码变化也检查注释对照。
PR 只读，不能发布。

主分支成功构建将编译根目录发布到 gh-pages，不强制推送；核对源码和输出摘要，显式请求并观察
已配置的 Pages 构建。首次创建 Pages 需要管理员，普通发布只使用 contents/pages 权限。
分支发布、实际 Pages 构建、自定义域名 DNS 和 HTTPS 分别记录。
域名为 llm-usage.atframe.work，DNS CNAME 应指向 owent.github.io。
已有流水线文件或本机生产构建不能据此确认远端 CI/部署成功。

<a id="local-wsl-execution-order"></a>

## 本地 WSL 执行顺序

仅在获得实施授权后尝试，记录每步结果：

1. 探测已安装发行版、WSL 版本、CPU 架构、可用空间及 WSLg；没有环境时记录缺口，
   不以用户允许尝试构建推断本轮已授权安装、升级或重启整机/发行版。
2. 在 Linux 文件系统创建本任务独立工作副本，记录 Windows 工作树 revision 和未提交差异摘要。
   不只 clone HEAD 而漏掉待验修改；不将 Windows node_modules、target 或个人 Agent 数据复制进去。
3. 按固定锁文件建立 Linux Rust/Node 工具链与 GTK/WebKitGTK 4.1、SSL、托盘和打包所需依赖。
   不修改 Windows 默认工具链，不依赖跨系统复用编译缓存；具体安装命令在实施时复核。
4. 顺序运行纯逻辑/fixture 测试、release 构建、Linux 打包；记录命令、退出码、产物和依赖。
   AppImage 的 FUSE/打包失败单独记录；不能把产出可执行文件写成安装包验收通过。
5. 按已授权范围使用独立 Podman、普通用户、Xvfb、窗口管理器和 D-Bus，实际安装包并
   验证 GTK/WebKit 窗口、IPC、页面和隔离数据；`test:install:linux` 是可复用入口。
   无实际显示/GUI 结果时只保留编译/测试结论，不以安装了桌面依赖报 GUI 通过。
6. 仅停止/清理本任务拥有的进程与目录，不使用全局 wsl --shutdown 干扰其他工作。
   应用临时库位于 Linux 本地文件系统；不与 Windows 程序共用活跃数据库。

WSLg 依据：[Microsoft Linux GUI 说明](https://learn.microsoft.com/en-us/windows/wsl/tutorials/gui-apps)。
该机制需要 WSL 2 且不提供完整 Linux 桌面。本轮容器 deb/AppImage 与实际 GUI
按用户范围验收；宿主托盘、通知、登录/注销和开机启动仍单独验证。
WSL 构建试验和本机 WSL Agent 用量验证分别记录；本机真实数据验证已获允许，
仍按开工准备文档限定只读范围，不由构建测试隐式启动采集。

验收见 V26/V27；未运行的 CI、WSL 或桌面测试不能用文档检查代替。
