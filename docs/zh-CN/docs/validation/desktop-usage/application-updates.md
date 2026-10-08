# 软件更新验证

日期：2026-10-08。版本：基于 `36d32858f11bbd63b4fdff4f553fd8d04429e064` 的
0.2.2 加未提交的更新器修改。本文记录本机实现证据；未创建
提交、tag、发行或部署。要求见[软件更新](../../design/desktop-usage/application-updates.md)。

## 源码检查与决策

读取锁定的 Tauri/HTTP 依赖、GUI 设置与 IPC、SQLite 设置与进程锁、便携脚本、平台
CI、固定版本的 NSIS 模板和安装生命周期。读取设计所链接的 Tauri updater、GitHub
发行及资产 API、Windows 文件替换官方资料。实际读取公开 v0.2.2 发行，核对七个
对应包的大小和 SHA-256 摘要。

标准 Tauri updater 不能读取现有 Windows 便携 tar.zst 格式。因此各包共用严格的
GitHub 发行选择和流式验证，再由独立辅助进程执行 NSIS 或便携更新。信任边界是
HTTPS 和仓库发行管理权限，没有独立发布者签名。自动下载默认关闭；安装始终需要点击。

## 环境与命令

Windows 11 x64、Node 24.21.0、Rust 1.98.0、Tauri CLI 2.12.0、系统 Edge/WebView2 和
锁文件依赖。在仓库根执行命令，日志及独立合成数据库位于已忽略的
`build/application-updates/`，浏览器截图和结果位于 `build/browser-smoke/`。
测试不需要真实 Agent 记录。

| 命令或检查 | 结果与受测范围 |
| --- | --- |
| `cargo test --manifest-path desktop/src-tauri/Cargo.toml --locked --bin LLMUsage update -- --nocapture` | 最终退出 0；12 项更新器测试。默认值与旧设置、持久化到期时间与时钟极值、严格的包/架构/稳定版本选择、缺失摘要、恶意重定向、有界与截断下载流、取消、归档安全、暂存元数据/CPU、无关文件冲突、中断替换、原文件缺失/重复备份、幂等恢复、拒绝已修改目标、目标锁和保存的失败 |
| `npm run test:scripts` | 退出 0；13 项，包括真实 zstd/tar 解压核对六种系统/架构布局清单 |
| `npm run test:ui` | 退出 0；22 项，包含十种语言的更新文案完整性 |
| `npm run test:browser` | 退出 0；记录 34 项检查，浏览器无错误。隔离 Edge 服务及模拟 IPC 检查设置、手动/自动下载、跨页面进度、取消（含准备操作仍在等待时）、通知关闭、按包类型显示按钮、显式安装、当前/历史错误和窄屏布局 |
| `npm run verify` | 退出 0；Rust 1,044 项通过/8 项平台条件测试忽略，前端 22 项、脚本 13 项，类型/fmt/clippy 无错误，资源检查和前端构建通过。之后的辅助程序路径修正再次通过 12 项定向测试、clippy 和 fmt |
| `npm run build:desktop` | 最终退出 0；构建 release 可执行文件及 Windows x64 NSIS |
| `npm run test:update:windows` | 最终退出 0；三个原生场景组：公开检查/设置、实际 IPC/辅助程序替换、主程序缺失的恢复/重新启动 |
| `npm run test:headless` | 退出 0；11 项隔离可执行文件/SQLite 检查；最终便携打包从真实解压目录再次执行这些检查 |
| `node desktop/tests/native-smoke.mjs --runs 1` | 退出 0；前一个构建通过 19 项既有真实 WebView2/IPC/SQLite/任务/窗口检查及三次必要启动。之后的辅助程序路径修改只影响更新恢复，已在最终程序中独立验证 |
| `node desktop/scripts/portable.mjs desktop/src-tauri/target/release build/application-updates/packages windows x64` | 退出 0；真实便携归档、压缩/解压/原生 CPU 核对、解压成品无界面检查及 NSIS 安装包复制 |
| `npm run check:docs`、`npm run test:docs`、`npm run build:docs`、`git diff --check` | 退出 0；201 对仓库文档/21 对指南及源码注释审阅，Astro 检查无错误，31 项测试、1,371 页面/2,872 发布文件及本地链接通过。双语记录及审阅哈希已同步 |

本地包保留在 `build/application-updates/packages/`，revision 标记为 `local`，不是公开
发行。便携包：4,199,627 字节，SHA-256
`40487736aa3d9dc9f5c8a81f6e4ead25073b0592365b8d2eff0efcc2c6ed78b3`。
NSIS：4,125,060 字节，SHA-256
`d1ab8cbe6c8e8c31fa4cecb4620abab29c92fab1c4e90496c23938b510c3bf09`。

## 用语审查

本次文案修订前，每种语言有 518 个词条。逐一检查十种语言各自的 33 个更新词条及
100 个常用导航、设置、采集控件词条，并检查其余简体中文词条和受影响的中英文文档。
修订 166 个已有词条，另为每种语言新增供应商默认设置标题，分别命名章节和添加操作。
标识符、软件包选择、调度及更新安装行为保持不变。

官方来源提供常见界面用语的实例，不构成对其他词的通用禁令。Mozilla 当前德文帮助页
明确标注为未经人工审阅的机器翻译，因此术语比较采用实际本地化源码。九种非英文
about-dialog 词库按 Firefox 本地化提交
`d8def708b77353ad76db8f14d62a7b594a3b13b1` 获取；完整快照和审阅摘录保存在
`build/update-terminology/`。具体措辞依据 LLM Usage 的实际控件：

| 语言 | 来源及采用的措辞 |
| --- | --- |
| 简体中文 | [Apple 软件更新](https://support.apple.com/zh-cn/108382)及[外观说明](https://support.apple.com/zh-cn/guide/mac-help/mchl52e1c2d2/mac)采用用户要求的软件更新用语及浅色、深色外观名称。界面现使用“软件更新”“浅色”“深色” |
| 繁体中文 | [Apple 台湾](https://support.apple.com/zh-tw/108382)使用“軟體更新”。安装说明采用“安裝程式”，通知按钮表示关闭通知 |
| 英文 | [Apple](https://support.apple.com/en-us/108382)和 [Firefox 帮助](https://support.mozilla.org/en-US/kb/update-firefox-latest-release)支持 Software updates、Check for updates 及重新启动的用语。版本状态明确以 LLM Usage 为对象 |
| 日文 | [Firefox 词条](https://github.com/mozilla-l10n/firefox-l10n/blob/d8def708b77353ad76db8f14d62a7b594a3b13b1/ja/browser/browser/aboutDialog.ftl)采用软件更新、检查和重新启动的用语。章节为“ソフトウェアの更新”，采集统一使用“収集” |
| 韩文 | [Firefox 词条](https://github.com/mozilla-l10n/firefox-l10n/blob/d8def708b77353ad76db8f14d62a7b594a3b13b1/ko/browser/browser/aboutDialog.ftl)使用“업데이트 확인”，最新版本状态以软件为对象。[Apple 韩文 App Store 指南](https://developer.apple.com/kr/support/app-store/)也使用“앱 업데이트”，因此保留该标题，明确启动及确认更新的表述 |
| 西班牙文 | [Firefox 词条](https://github.com/mozilla-l10n/firefox-l10n/blob/d8def708b77353ad76db8f14d62a7b594a3b13b1/es-ES/browser/browser/aboutDialog.ftl)使用“Buscar actualizaciones”。便携版、安装版标签明确版本对象，下载完成句补全主语，说明文字采用与原界面一致的非正式称呼 |
| 法文 | [Firefox 词条](https://github.com/mozilla-l10n/firefox-l10n/blob/d8def708b77353ad76db8f14d62a7b594a3b13b1/fr/browser/browser/aboutDialog.ftl)使用“Rechercher des mises à jour”，最新版本状态以软件为主语。补全版本标签，将直译的保存提示改成完整句子 |
| 德文 | [Firefox 词条](https://github.com/mozilla-l10n/firefox-l10n/blob/d8def708b77353ad76db8f14d62a7b594a3b13b1/de/browser/browser/aboutDialog.ftl)使用“Nach Updates suchen”和软件主语。采用 Updates、明确版本的标签，以及分写的动词“Neu starten” |
| 巴西葡萄牙文 | [Firefox 词条](https://github.com/mozilla-l10n/firefox-l10n/blob/d8def708b77353ad76db8f14d62a7b594a3b13b1/pt-BR/browser/browser/aboutDialog.ftl)使用“Verificar se há atualizações”。状态以 LLM Usage 为主语，下载完成、安装版标签明确版本对象，隐私说明明确用量数据 |
| 俄文 | [Firefox 词条](https://github.com/mozilla-l10n/firefox-l10n/blob/d8def708b77353ad76db8f14d62a7b594a3b13b1/ru/browser/browser/aboutDialog.ftl)使用“Проверить наличие обновлений”。下载完成状态明确版本对象，每日计划改用语法完整的时间表述 |

源码检查还确认了三类语义问题。`UpdateNotice.svelte` 只隐藏当前状态标识，没有提醒
计时器，因此十种语言的按钮均改为表示关闭通知。`commands.rs` 写入当前用户的 Run
注册表项；[Microsoft](https://learn.microsoft.com/en-us/windows/win32/setupapi/run-and-runonce-registry-keys)
定义该触发条件为用户登录，因此中英文标签明确表示登录时启动。采集器读取已启用且
到期的来源，并非只读取今日记录，因此十种语言的间隔标签均改为自动采集。供应商
默认设置选择计价渠道及 5/60 分钟提示缓存的计价假设；[Anthropic 缓存文档](https://platform.claude.com/docs/zh-CN/build-with-claude/prompt-caching)
确认这两种时长。时长控件现有翻译后的可访问名称；它不配置 Agent 的实际缓存。
其他修订明确表示已保存的设置、全局设置、每周起始日及永久保留。

目视检查还发现语言切换缺陷：保存成功提示在 `onsaved` 更新语言前就已翻译，后续
截图均保留前一种语言。保存及恢复默认值的提示现保留词条键，在渲染时按当前语言
翻译。浏览器循环在截图各更新页之前，逐一核对十种新选语言的保存成功提示。

本次文案修订的 `npm run test:ui` 通过 22 项，`npm run check` 无错误、警告。
`npm run build:web` 通过。`npm run lint:md` 检查 455 个文件，无问题；
`npm run check:docs` 通过 201 对仓库文档/21 对指南及 Astro 检查，无诊断问题；
`npm run test:docs` 通过 31 项。
`npm run test:browser` 通过所记录的 34 个场景组，浏览器无错误，包含十种语言的
更新/设置页切换及 760 像素视口、既有更新操作/进度/取消、安装版与便携版的不同按钮。
截图位于 `build/browser-smoke/updates-<locale>-narrow.png`。这是浏览器及模拟 IPC
检查，不能建立所有译文经母语审阅的结论。上文的软件包哈希及下文的原生
更新结果属于此前功能构建；本次文案修订未重跑 NSIS、便携替换或其他平台 GUI 验收。
写作 Skill、相关软件包说明及已审阅的双语文档同步维护。

## Windows 原生便携结果

`npm run test:update:windows` 将已构建程序复制到含空格和中文的独立包根目录，隔离
来源环境、TEMP/TMP 及 SQLite 数据，关闭采集并设置仅手动检查更新。真实 WebView2/
IPC 核对便携身份、无需认证的公开最新版检查、设置保存和重启后的上次检查时间。

测试编译原生替换程序，注入包身份、大小和 SHA-256 相符的完整 tar.zst 下载。真实
`install_update` 调用核对并暂存文件；原 GUI 退出，程序副本作为辅助进程等待退出，
在同一路径替换受管理文件、保留原始备份并启动新程序。替换程序写入重启回执。
独立核对新可执行文件与清单、原备份哈希、包根目录中的无关文件、SQLite 哨兵及保存
的设置；日志最终为 `applied`。

第二个恢复场景只将自有测试日志改为 `applying`，删除合成替换程序，再直接启动保留
的辅助程序。它恢复缺失的原程序、记录 `rolled_back`、启动真实应用、保留个人文件，
并通过真实 IPC 显示 `update_interrupted_and_restored`。最终报告位于
`build/application-updates/native/1791466696837-62876/report.json`。

这验证了使用合成原生程序的真实更新 IPC、进程和文件行为，不能建立公开新应用、
新应用 GUI/数据库迁移或 NSIS 更新执行结论。生产更新器没有用于覆盖端点或路径的
IPC；测试只在启动前写入自有本地缓存。

## 首次失败与修正

- 普通 sandbox 执行返回 `CreateProcessAsUserW failed: 5`；自动审阅的命令执行恢复
  访问。这是基础设施问题，不是应用测试失败。
- 浏览器测试最初遇到 Vite 端口占用及共享依赖缓存。脚本现在选择空闲端口，并在根
  build 下使用独立缓存和配置，保留既有用户服务及用户的调试应用。
- 浏览器发现更新页缺少共用保存按钮，现已支持保存设置。进度断言对齐一秒状态轮询，
  通知关闭逻辑除版本和阶段外还区分包名。
- 最终保存错误的修改最初未通过 Rust 的可变/不可变借用检查。初始化现先复制包身份，
  再更新状态。还修正了 clippy 的冗余闭包及一次补丁上下文编译错误；最终 12 项测试通过。
- Markdown 检查发现中文标题与章节重名，现使用不同章节标题。
- 安装准备的 IPC 等待期间原本禁用取消按钮，取消现有独立等待状态，安装文件前仍可用。
- 文档菜单测试保留了此前 219 页的来源计数，现要求两个更新路由及当前 221 页的完整
  来源目录，31 项测试全部通过。
- PowerShell 的 npm 包装脚本以 `EUNKNOWNCONFIG` 拒绝 `--runs`，改为用 Node 直接
  执行真实原生脚本后通过。
- 主程序缺失的恢复首次因对话框超时：直接启动的辅助程序使用普通 Windows 路径，
  日志却要求规范化路径。辅助输入现在先将普通日志文件路径规范化再验证，独立最终
  重跑通过。
- 审查将检查时间保存移到工作线程，并释放更新状态锁，使数据库写入占用期间仍可读取
  状态及取消。自动下载在同一锁下获取可下载状态；文件替换保留备份，回退处理原文件
  缺失或原文件与备份同时存在且未改变的情况。

最终测试后的原生进程检查仅发现此前的用户调试应用；自有更新/辅助程序和原生冒烟
进程均已退出。测试未停止用户正在运行的应用，也未修改既有安装或真实来源文件。

## 验收限制

公开最新版与运行版本相同，因此真实请求验证 `up_to_date`，未验证生产新版本下载。
后续公开发行必须保留完整文件名、全部资产和 API 摘要，另行验证真实升级。此前没有
新清单的便携归档需要先手动替换一次，才能采用此更新器。

NSIS 生命周期预检查发现既有用户调试应用。脚本要求没有运行应用，本轮保留该进程，
不重跑安装、卸载或任务修改。此前生命周期结果保留为历史证据；新的安装包选择和
安装后注册表/版本检查不能建立真实 NSIS 更新器验收。

Linux/macOS 原生 GUI 更新执行、平台权限和重新启动仍需独立验收。六种布局打包测试
及保留的 CI 定义不能建立本未提交源码在远端 runner 执行的结论。未添加签名凭据。
