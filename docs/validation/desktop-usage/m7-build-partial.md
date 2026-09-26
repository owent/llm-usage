# M7（部分）：release 构建与包体证据

M7 的安装/资源/调度/三平台完整验收（V19–V27）未开始；本记录仅登记
Windows release 构建与包体实测，以及 WSL Linux 编译/测试分项（V27 部分）。
macOS 按用户指示不经本机验收，后续经 CI 流水线测试。

## 元信息

| 项目 | 内容 |
| --- | --- |
| 日期 | 2026-09-25 |
| 执行环境 | Windows 11 x64；rustc 1.98.0；Node v24.21.0；WSL2 Ubuntu rustc 1.98.1 |
| 代码 revision | 未提交工作树（M3 文档级批次之后） |

## 命令与结果

| # | 命令（cwd） | 退出码 | 结果摘要 |
| --- | --- | --- | --- |
| 1 | `npm run build:desktop`（仓库根） | 0 | release 构建 + NSIS 打包成功 |
| 2 | WSL `cargo check -p llm-usage-core / -p llm-usage-m0` | 0 | core 与 app 在 Linux 编译通过 |
| 3 | WSL `cargo test -p llm-usage-core` | 0 | 全绿（Linux 侧） |

## 包体实测（对照 architecture.md 资源目标）

| 项目 | 实测 | 目标 |
| --- | --- | --- |
| NSIS 安装包 | 首测 2,343,869 B ≈ 2.24 MiB；改名重建 LLMUsage_0.1.0_x64-setup.exe（同量级） | ≤ 20 MiB ✓ |
| 主程序二进制 | 5,808,128 B ≈ 5.54 MiB | 安装目录 ≤ 60 MiB（组件分项待测） |
| 前端 gzip | 220.32 kB | ≤ 1 MiB ✓ |

说明：M0 报告的空壳试验包体已被包含 17 适配器/查询/调度/导出/i18n 的
当前实现取代；WebView2 Evergreen 复用前提下的安装包增量达标。
应用已按用户要求改名：二进制与安装包 LLMUsage（2026-09-26 起，
旧 llm-usage-m0 制品已删除）。
空闲内存（≤180 MiB）与首屏/查询分位数（V20）未测，留 M7 正式验收。

## 未完成项（M7 剩余）

真实安装/升级/卸载与重启恢复（V19）、全进程资源（V21）、离线运行、
Windows 系统任务对账（V24）、三平台 CI 制品（V26，待推送后首次运行登记）、
WSL 编译已证/打包与 WSLg 未做（V27）、支持矩阵与 README 同步。
