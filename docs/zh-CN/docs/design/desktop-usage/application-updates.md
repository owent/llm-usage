# 软件更新

## 设置与通知

更新设置独立于采集、价格刷新和系统任务。自动检查默认为每天一次，可以选择每次启动、
每天、每周或仅手动。每天和每周分别表示距离上次成功检查满 24 小时或七天，检查时间
跨启动保存。自动请求失败后间隔一小时重试。GUI 启动及每分钟的计时器检查是否到期；
headless 采集不检查或安装更新。手动检查跳过时间限制。检查和下载共用一个操作槽位。

自动下载默认关闭。发现新版本后显示下载提示；启用自动下载后，应用下载并验证对应包，
然后提示安装更新或重启并更新。安装始终需要用户点击。全局通知条和“设置 > 软件更新”
显示版本、包类型、已下载字节、总字节、进度、取消和错误。切换页面保留下载进度。
显示成功检查的时间；设置保存后生效。

设置页在英文中名为 Software updates，简体中文为“软件更新”，繁体中文为“軟體更新”；
其他语言采用各自常见的软件界面用语。“关闭通知”隐藏当前通知，直到阶段、版本、包或
错误变化；设置页仍可操作更新。关闭通知不会安排稍后提醒。

## 发行与包身份

主要更新来源是 `owent/llm-usage` 的公开 HTTPS GitHub Releases API。读取最新公开稳定版，
严格比较语义版本并匹配完整资产名。禁止降级、跨架构或改变安装类型。Windows NSIS
安装版选择 `LLMUsage_<version>_x64-setup.exe`；便携版选择
`LLMUsage-<version>-<platform>-<arch>-portable.tar.zst`。

新便携归档包含 `llmusage-package.json`，记录 schema、版本、平台、架构、原生可执行
文件和受管理的顶层条目。声明的原生可执行文件必须解析到当前运行文件。Windows 安装
身份通过 NSIS 注册表的 InstallLocation、MainBinaryName、DisplayVersion 与实际可执行文件路径核对。
未识别的独立二进制和未支持的安装包格式不能自动选择更新包；开发构建需要先打包再
测试安装。

检查版本不依赖安装类型。开发构建和未识别安装类型的可执行文件可以查询公开的
正式版本并显示新版本，但不选择安装版或便携版资源，也不能通过更新器下载或安装。
开发构建使用本地化提示解释这一限制。自动检查仍遵循已保存的频率，自动下载则
必须先核实安装类型并匹配资源。

请求只发送应用 User-Agent 和公开发行标识，不发送用量、本地路径、凭据或账户标识。
生产端点及重定向仅允许 GitHub HTTPS 发行基础设施和下述精确的文档元数据 URL。
限制元数据读取量并流式下载到
磁盘，限制下载大小。解压前、安装更新前和重启后复用完整下载时，核对精确大小及
GitHub API 资产 SHA-256。缺失摘要、资产重复、URL、版本或大小不匹配时拒绝更新。
`.part` 文件不能安装；中断下载从零重新开始。

GitHub 元数据请求失败（包括 HTTP 403/429、超时、无法读取或响应超限）时，
仅尝试一次 `https://llm-usage.atframe.work/updates/latest.json`。不立即重试 GitHub API，
也不通过回退绕过成功 API 响应中的版本或资源错误。两次请求之间检查取消操作，
两个来源均失败时保留两个错误码。元数据限制为 1 MiB，拒绝重定向到其他 URL。

站点快照包括 schema 1、repository `owent/llm-usage`、UTC `generated_at`，以及仅含
精确资源名、GitHub 下载 URL、大小和 SHA-256 的公开发行信息。超过七天或比当前时间
超前五分钟以上的快照被拒绝。站点仍引用 GitHub 下载，不镜像安装包；两个来源使用
相同的资源匹配及大小/摘要校验。信任范围增加本仓库文档发布的 HTTPS 站点，但没有
独立的发布者签名。

文档 CI 在 main 发布、每日任务和 Release 发布/编辑/取消发布/删除时刷新快照。
Release 事件构建当前 main，并保留其源码修订，避免部署旧 tag 的文档。生成器确认
六个 portable 包和 Windows x64 NSIS 包均已上传且摘要有效后才发布元数据。
CI 的受限 token 只用于 GitHub 读取，不进入快照，也不发送到文档站点。
刷新失败时可以复用仍有效的站点或已提交快照，不修改原生成时间；没有有效快照时
保留现有发布。普通本地文档构建可以离线使用已提交快照，但桌面客户端仍拒绝过期数据。

本设计信任 HTTPS 和仓库的 GitHub 发行管理权限。SHA-256 检测损坏及资产变化，不是
独立的发布者签名。现有 CI 已校验 GitHub 上传资产摘要。Tauri 标准 updater 还要求
发布者密钥，但不能处理当前 Windows 便携 tar.zst 包；采用该协议需要另一种包格式
及受管理的签名密钥。

## 执行更新

NSIS 更新使用已有 `/UPDATE /P` 模式和原安装目录，安装器处理权限，退出后必须登记
请求的版本，保留数据和已有系统任务授权。便携更新在目标文件系统暂存、保留备份，
并在原路径替换受管理的包条目。
保留包根目录中的其他文件；新引入的受管理条目不能覆盖已有的无关文件。应用数据及
系统凭据保留在原有用户级位置。

便携解压拒绝路径穿越、绝对路径、重复成员、硬链接、特殊文件和越界符号链接，并限制
解压字节数及条目数。内部 Unix 符号链接在普通成员之后创建，最终解析拒绝悬空、循环
或越界链接。提示可以重启更新前核对包元数据及原生架构。

辅助实例在数据库初始化、接收器或 GUI 启动之前进入更新模式。Windows 使用受管理
条目之外的当前程序副本；Unix 可以在替换包文件时继续使用运行中的 inode。辅助实例
获取目标锁并报告就绪后，主程序才退出；辅助实例在限定时间内等待主程序结束再执行
更新。日志在任何重命名前记录原条目是否存在。替换失败恢复原条目；中断日志在同一
目标锁下恢复。保留最近一次备份和结果；启动了新进程不能证明新 GUI 或数据库成功
启动。数据库迁移沿用既有一致性备份规则；文件回退不擅自恢复或丢弃用户数据库。

已有普通文件在 Windows 使用 `ReplaceFileW` 及同卷备份，在 Unix 使用备份链接和
重命名；目录变更仍需要在两次重命名之间依靠日志恢复。如果中断的 Windows 替换导致
主程序缺失，可以打开保留的 `.llmusage-update/helper.exe`，恢复原文件并启动应用。

采集或数据库维护运行时拒绝安装更新。重启保留当前数据目录；headless 和后台集成
继续使用同一可执行文件路径。明确显示空间不足、权限、杀毒软件锁定、辅助程序启动、
超时、摘要、解压、安装器和重启失败，包括重新启动后显示已保存的辅助程序错误。
检查、下载及准备期间可以取消；安装文件后不能通过取消撤销更新。阻塞的网络读取
会在有限的请求超时内返回。

## 验证

回归默认值和旧设置、持久化到期时间及系统时钟回拨、手动检查、并发操作、稳定版本
排序、两种架构、安装版与便携版的精确选择、身份未知、无效元数据、重定向、缺失摘要、
下载截断、损坏、取消、缓存复用、恶意归档、文件冲突、替换回退及中断日志。浏览器
检查设置、全局进度和显式安装。原生 Windows 测试使用隔离的打包副本及独立合成
数据库，辅助程序文件替换与真实 NSIS 生命周期分别记录。Linux/macOS CI 检查包
布局和平台代码；本机 Windows 结果不能建立其他平台的桌面更新验收。公开的新版本
升级需要后续公开发行，并与注入发行元数据的测试分开记录。

## 来源依据

2026-10-08 检索：

- [Tauri updater](https://v2.tauri.app/plugin/updater/)：支持的资产、强制签名、独立下载与安装，
  以及 Windows 安装模式。
- [GitHub releases](https://docs.github.com/en/rest/releases/releases?apiVersion=2026-03-10)：
  最新稳定版和无需认证的公开访问。
- [GitHub 发行资产](https://docs.github.com/en/rest/releases/assets?apiVersion=2026-03-10)：
  资产名、上传状态、大小、摘要和下载 URL。
- [Windows 文件替换](https://learn.microsoft.com/en-us/windows/win32/fileio/moving-and-replacing-files)：
  替换文件并保留原件。
- [ReplaceFileW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-replacefilew)：
  同卷替换与备份、保留属性及部分失败状态。
- 仓库 `desktop/scripts/portable.mjs`、固定到 tauri-cli-v2.12.0 的 NSIS 模板和
  `.github/workflows/ci.yml`；已读取实际 v0.2.2 发行元数据，核对七个包的大小和 SHA-256。

2026-10-09 补充核对：

- [GitHub REST 限频规则](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api)：
  HTTP 403/429 及有限重试要求；失败后不立即重试 API。
- [Release 工作流事件](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#release)：
  Release 事件使用标签对应的提交。
- [触发工作流](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/trigger-a-workflow)：
  显式 workflow_dispatch 可使用 GITHUB_TOKEN 触发工作流。
- [工作流 dispatch API](https://docs.github.com/en/rest/actions/workflows#create-a-workflow-dispatch-event)：
  选择 main ref 及限定范围的 Actions 写权限。
