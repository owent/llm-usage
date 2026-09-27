# 平台、GitHub CI 与 WSL 验证

状态：三平台 workflow 已建立，首次远端运行仍待推送后登记。用户已确认 Windows 11 x64 首发，
同时保留 macOS/Linux 的 GitHub CI。本文件保留验收合同；实际执行情况见验证记录。
用户已确认按本方案推进：runner/工具链/Linux 基线在 M0 固定，WSL/WSLg 可用性在 M0 探测；
macOS/Linux 不承诺首发发行支持。这些属于实施核验，不再作为开工前待用户确认事项。

## 平台与制品矩阵

| 平台 | 首发定位 | CI 初始目标 | 交付和证据 |
| --- | --- | --- | --- |
| Windows 11 x64 | 正式首发目标 | windows-2022；x86_64-pc-windows-msvc | NSIS 安装包候选、无界面采集、原生 IPC/安装/任务验证；另需真实 Windows 11 验收 |
| Linux x64 | 持续兼容构建 | ubuntu-22.04；x86_64-unknown-linux-gnu | release 编译、Debian 包及 AppImage 候选；原生/虚拟显示测试与打包分别记录 |
| macOS arm64 | 持续兼容构建 | macos-15；aarch64-apple-darwin | Rust/前端测试、release .app 构建；未签名或仅临时签名制品标作 CI 产物 |
| 本地 WSL 2 Linux x64 | 可选开发验证 | 优先与 CI 相同发行版和工具链 | 构建、测试、打包、WSLg 冒烟分别记录；不能替代原生 Linux 桌面验收 |

以上 runner 是当前可用候选，M0 复核并锁定版本化标签；不使用浮动 latest 隐式改变架构。
macOS Intel、Linux arm64、Windows arm64 不在首批必需矩阵，后续扩展需独立验证。
GitHub Windows runner 的构建结果不等于 Windows 11 用户环境通过；macOS/Linux CI 保持运行，
其制品不自动变成首发支持承诺。未通过项必须在结果中可见，不能长期 continue-on-error 掩盖失败。

依据：[GitHub runner 列表](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)、
[Tauri 多平台流水线](https://v2.tauri.app/distribute/pipelines/github/)、
[平台依赖](https://v2.tauri.app/start/prerequisites/)。Linux 最低运行系统需按实际链接依赖在 M0 确认，
不能把 CI 镜像版本当作已经验证的全部 Linux 支持范围。

## GitHub CI 合同

M0 在代码骨架具备后建立三平台作业；M1–M6 逐步加入实际测试。M7 验收时三个 OS 的作业必须存在
且运行，不只留下注释或矩阵占位。任务触发为 PR、主分支 push 和手动执行；本轮不推送触发它们。

1. 公共检查：Markdown/本地链接、前端类型和单元测试、Rust fmt/clippy；命令从实际锁文件确定。
2. OS 矩阵：在每个原生 runner 运行领域/适配器 fixture、真实临时 SQLite、调度器、路径和锁测试，
   构建同一源码的前端与 Rust release。外部 Agent 不需安装，不读取 runner 个人目录。
3. 制品：Windows 安装包、Linux 包、macOS .app，附源码 revision、OS/arch、依赖版本、大小和校验和。
   打包失败与测试失败分别报出，不能用 cargo check 代替成品构建。
4. 桌面集成：Windows/Linux 选择已核验的 Tauri WebDriver 路径；macOS 评估当前官方的嵌入式
   WebDriver 支持。测试插桩仅在测试构建启用，不进入发布包；暂缺真实 GUI 验证时如实记录。
5. 独立 release 资源任务测包体和性能；通用托管 runner 时序只作回归信号，不能代替固定机器 P95。
6. 构建依赖下载可以联网；统计运行和验收禁止出站访问用量/计费服务。fixtures 全部合成或脱敏，
   不上传个人数据库/原始会话。离线用例覆盖有遥测开关时的行为。

2026-09-27 检查入口已补齐：`cargo fmt --all --check`、
`cargo clippy --workspace --all-targets --locked -- -D warnings` 覆盖核心 crate 与测试目标；
前端作业运行 TypeScript 纯逻辑测试和 Playwright 浏览器回归。后者使用合成 IPC 数据，
覆盖主题、时区、快速筛选、用户隔离和刷新，不计为第 4 项原生桌面验收。
本轮仅本机 Windows 执行，未触发远端 CI，见
[本轮记录](../../validation/desktop-usage/review-2026-09-27.md)。

矩阵使用 fail-fast=false 留下全部结果；每作业有超时，重跑只针对已定位的临时基础设施故障。
缓存键包含 OS、架构、Rust/Node 版本和锁文件摘要，隔离不同 target，不缓存真实 Agent 数据。
Actions 固定完整提交 SHA，PR 使用只读权限，无发布/签名密钥；不在特权 pull_request_target 中执行 PR 代码。
CI artifact 与 GitHub Release 发布分开，发布/签名/公证待相应授权，不复制官方示例中的自动发版步骤。

桌面测试依据：[Tauri WebDriver](https://v2.tauri.app/develop/tests/webdriver/)。
当前文档区分直接 tauri-driver 的 Windows/Linux 支持与嵌入式服务的 macOS 路径；
M0 核验选定工具实际版本后固定方案，不依赖付费平台服务完成基础构建。

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
5. WSLg 可用时人工冒烟检查窗口、图表、IPC、目录选择、临时 fixture 导入及退出；
   没有显示能力时仍保留编译/测试结果，GUI 项记未执行。
6. 仅停止/清理本任务拥有的进程与目录，不使用全局 wsl --shutdown 干扰其他工作。
   应用临时库位于 Linux 本地文件系统；不与 Windows 程序共用活跃数据库。

WSLg 依据：[Microsoft Linux GUI 说明](https://learn.microsoft.com/en-us/windows/wsl/tutorials/gui-apps)。
该机制需要 WSL 2 且不提供完整 Linux 桌面；托盘、通知、开机启动、安装和系统集成仍须原生验收。
WSL 构建试验和本机 WSL Agent 用量验证分别记录；本机真实数据验证已获允许，
仍按开工准备文档限定只读范围，不由构建测试隐式启动采集。

验收见 V26/V27；未运行的 CI、WSL 或桌面测试不能用文档检查代替。
