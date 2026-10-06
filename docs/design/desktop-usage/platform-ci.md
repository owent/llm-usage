# 平台、GitHub CI 与 WSL 验证

状态：2026-10-06 已按用户授权提交并推送独立测试分支，两轮三平台 Rust/release 及
公共检查各八作业成功；macOS 两项显式原生凭据通过。三份归档/四个包的下载摘要及
源码 revision 已核对，结果见 [本批 CI](../../validation/desktop-usage/ci-plan-validation.md)。
用户已确认 Windows 11 x64 首发，
同时保留 macOS/Linux 的 GitHub CI。本文件保留验收合同；实际执行情况见验证记录。
用户已确认按本方案推进：runner/工具链/Linux 基线在 M0 固定，WSL/WSLg 可用性在 M0 探测；
macOS/Linux 不承诺首发发行支持。这些属于实施核验，不再作为开工前待用户确认事项。
2026-10-05 验收范围调整：不要求 macOS 桌面或特定硬件；保留 macOS 构建与 Rust CI。
Linux 实际 GUI/安装生命周期接受 WSL/Debian 内的独立 Podman 环境，
记录发行版、镜像 digest、用户、显示与沙箱条件；不扩展为宿主登录/完整桌面支持。
Windows 本机安装/升级/回滚/卸载已授权；新增合同见 [安装生命周期](installation-lifecycle.md)。

## 平台与制品矩阵

| 平台 | 首发定位 | CI 初始目标 | 交付和验证结果 |
| --- | --- | --- | --- |
| Windows 11 x64 | 正式首发目标 | windows-2022；x86_64-pc-windows-msvc | NSIS 安装包候选、无界面采集、原生 IPC/安装/任务验证；另需真实 Windows 11 验收 |
| Linux x64 | 持续兼容构建 | ubuntu-22.04；x86_64-unknown-linux-gnu | release 编译、Debian 包及 AppImage 候选；原生/虚拟显示测试与打包分别记录 |
| macOS arm64 | 持续兼容构建 | macos-15；aarch64-apple-darwin | Rust/前端测试、release .app 构建；未签名或仅临时签名制品标作 CI 产物 |
| 本地 WSL 2 Linux x64 | 开发及已授权的容器验收 | 记录实际发行版与工具链 | 构建、测试、打包单独记录；独立 Podman 的实际 GTK/WebKit GUI 与软件包生命周期按本轮范围验收 |

以上 runner 是当前可用候选，M0 复核并锁定版本化标签；不使用浮动 latest 隐式改变架构。
macOS Intel、Linux arm64、Windows arm64 不在首批必需矩阵，后续扩展需独立验证。
GitHub Windows runner 的构建结果不等于 Windows 11 用户环境通过；macOS/Linux CI 保持运行，
其制品不自动变成首发支持承诺。未通过项必须在结果中可见，不能长期 continue-on-error 掩盖失败。

依据：[GitHub runner 列表](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)、
[Tauri 多平台流水线](https://v2.tauri.app/distribute/pipelines/github/)、
[平台依赖](https://v2.tauri.app/start/prerequisites/)。Linux 最低运行系统需按实际链接依赖在 M0 确认，
不能把 CI 镜像版本当作已经验证的全部 Linux 支持范围。

## GitHub CI 合同

三平台作业已存在；M7 持续验收须有实际运行结果。任务触发为 PR、主分支 push
和手动执行，维护本地文件不意味着获准推送或触发远端 CI。

1. 公共检查：Markdown/本地链接、前端类型和单元测试、Rust fmt/clippy；命令从实际锁文件确定。
2. OS 矩阵：在每个原生 runner 运行领域/适配器 fixture、真实临时 SQLite、调度器、路径和锁测试，
   构建同一源码的前端与 Rust release。外部 Agent 不需安装，不读取 runner 个人目录。
3. 制品：Windows 安装包、Linux 包、macOS .app，附源码 revision、OS/arch、依赖版本、大小和校验和。
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
Windows 构建后执行真实可执行文件的 `test:headless`，隔离合成来源并检查来源类别。
Rust fmt/Clippy/test 作业分别在三个原生 runner 执行，平台存储实现纳入各自编译。
Linux 另运行 `test:credentials:linux`，使用独立 D-Bus 与一次性加密 keyring；
凭据只在 stdin 传给测试子进程。macOS Rust 作业另显式运行系统凭据跨进程往返与
真实 loopback HTTP 撤销，仅创建并精确回收随机自有项，不启用同步或认证 UI。
Windows 原生凭据在本机显式执行，默认忽略项不能当作系统存储已验收。
本机 `test:desktop` 经 WebView2 CDP 检查真实 IPC，不在发布包增加监听代码。
最新本机结果及远端 CI 缺口见 [最新验收](../../validation/desktop-usage/current-acceptance.md)。

矩阵使用 fail-fast=false 留下全部结果；每作业有超时，重跑只针对已定位的临时基础设施故障。
缓存键包含 OS、架构、Rust/Node 版本和锁文件摘要，隔离不同 target，不缓存真实 Agent 数据。
Actions 使用 v 主版本号浮动引用而非固定提交 SHA，PR 使用只读权限，无发布/签名密钥；不在特权 pull_request_target 中执行 PR 代码。
CI artifact 与 GitHub Release 发布分开，发布/签名/公证待相应授权，不复制官方示例中的自动发版步骤。
上传仅包含安装包、macOS `.app.tar.gz` 与大小/校验和报告；Debian 的
`control.tar.gz`/`data.tar.gz`、AppDir 和其他打包暂存目录不计为发布制品。

桌面测试依据：[Tauri WebDriver](https://v2.tauri.app/develop/tests/webdriver/)。
Linux 本轮直接使用 WebKitWebDriver 原生 capabilities，启动真实打包应用；
它与 tauri-driver 的 WebKit 参数映射一致。macOS 构建/凭据原生测试与桌面验收分开。
不依赖付费平台服务完成基础构建。

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
