# 检查软件更新修复

在 Windows 11 x64、Node 24.21.0、Rust 1.98、Tauri CLI 2.12.0 和 WebView2 上验证。
基础提交为 `cfaf3b64fbea044f39579b6c43ec7f862b745b1a`，版本为 0.3.0。
这是两项独立修复，均无须修改发行标签。

## 开发模式检查版本

用户报告的启动命令为 `npm run dev:desktop`。安装类型检测会对照正在运行的程序，
核实 NSIS 注册表信息或便携包清单。开发程序没有这两类信息。旧的手动检查获取发行
元数据后，在比较版本前就要求识别安装类型，因而出现 `update_package_identity_unknown`。
未识别安装类型的程序也不会执行自动检查。

版本查询现已独立执行。开发模式和未识别安装类型的程序可以显示新的正式版本，但不
选择更新文件。自动检查遵循已保存的设置；自动下载和直接下载仍要求核实安装类型。
十种语言均包含开发模式说明。浏览器回归验证开启自动下载时发现新版本、没有报错，
也没有下载或安装操作。

隔离复制原始 0.3.0 程序后，通过真实 WebView2 IPC 复现了用户报告的错误。
修复后的内嵌前端调试构建完成同一公开版本检查，结果为 `package_kind=development`、
`phase=up_to_date`，没有错误，也没有更新文件。直接下载被拒绝。使用独立的应用数据
和来源环境，保留用户正在运行的软件和数据库。

| 检查 | 命令 | 结果 |
| --- | --- | --- |
| 修复前 | node desktop/tests/native-update-check.mjs | 退出 1；复现 update_package_identity_unknown |
| 安装类型单元回归 | cargo test --manifest-path desktop/src-tauri/Cargo.toml --bin LLMUsage updates::tests --locked | 退出 0；站点回退修改前共六项 |
| 内嵌前端调试构建 | node node_modules/@tauri-apps/cli/tauri.js build --debug --no-bundle（desktop 工作目录） | 退出 0 |
| 修复后的开发模式 IPC | node desktop/tests/native-update-check.mjs --exe desktop/src-tauri/target/debug/LLMUsage.exe --development | 退出 0；站点回退修改前两项检查 |
| 浏览器交互 | npm run test:browser | 退出 0；开发模式新版本提示、无错误/下载/安装/自动下载，以及既有更新交互 |

首次运行修复后的原生探针时，与新增的启动检查重叠，程序按预期返回
`update_operation_running`。探针现等待该操作结束后才请求手动检查。首次浏览器断言
早于状态轮询；将受控时钟推进至轮询完成后修正了探针。这两处修改均未放宽产品的
并发操作限制。已审阅截图 `build/browser-smoke/update-development-check.png`，
其中模拟当前 0.2.2、可用 0.3.0；该结果与公开发行检查分列。

## 文档站点元数据回退

用户报告的安装类型错误并非 GitHub 限频所致。另行增加 GitHub 元数据请求失败后，
从精确的 HTTPS 文档端点 `https://llm-usage.atframe.work/updates/latest.json`
尝试一次的回退。成功的 API 响应中若有版本或资源错误，不通过回退绕过校验。
两次请求间检查取消操作，元数据限制为 1 MiB，两个来源失败时保留各自错误码。

schema 1 快照包含仓库、UTC 生成时间和最少的正式发行字段。客户端拒绝超过七天，
或生成时间晚于当前时间五分钟以上的快照。两个来源使用相同的完整文件名、平台、
架构、上传状态、大小和 SHA-256 校验。站点引用 GitHub 下载，不镜像更新包。

文档 CI 在 main 构建时及每天 UTC 04:17 刷新元数据。Release 发布、编辑、撤回或删除
会触发 main 分支构建，保留已有 Pages 环境仅允许 main 的限制及源码修订校验。
仅 GitHub 读取携带 CI token。可以复用仍有效的站点或已提交快照，但不延长其时间戳；
否则构建失败，现有发布保持不变。本地离线构建使用已提交快照，桌面客户端仍执行
有效期限制。

| 检查 | 命令 | 结果 |
| --- | --- | --- |
| 原生回退回归 | cargo test --manifest-path desktop/src-tauri/Cargo.toml --bin LLMUsage updates::tests --locked | 退出 0；共 11 项，含两项安装类型测试；覆盖请求失败、取消、有效期、包校验及来源限制 |
| 生成器/传输/CI 回归 | node --test docs/site/tests/update-feed.test.mjs | 退出 0；八项，含 token 隔离、响应限制及 Release 触发 main 构建 |
| 真实公开元数据提取 | node docs/site/scripts/update-feed.mjs --output docs/site/public/updates/latest.json | 退出 0；正式版 v0.2.2、七个包的 API 大小/摘要；生成时间 2026-10-09T02:07:18.849Z |

检查时公开的最新发行返回 v0.2.2。本地 v0.3.0 标签不能建立公开 Release 结论。
运行时测试使用真实生成的快照，检查六种便携版平台/架构选择；生成器核对包括 NSIS
在内的七种包。发行正文、上传者、凭据、用量和本地路径均不进入快照。

## 最终本地验证

| 检查 | 命令 | 结果 |
| --- | --- | --- |
| 产品统一检查 | npm run verify | 退出 0；Rust 1,053 项通过，八项平台条件测试忽略；脚本 13 项、前端 22 项；格式、clippy、资源和 Svelte 类型检查通过，前端构建完成 |
| Windows release | npm run build:desktop | 退出 0；生成 0.3.0 可执行文件及 NSIS 包 |
| 最终未打包 release IPC | npm run test:update:check | 退出 0；两项，up_to_date、安装类型 unknown，无错误/更新文件，拒绝直接下载 |
| 最终内嵌开发构建 IPC | node desktop/tests/native-update-check.mjs --exe desktop/src-tauri/target/debug/LLMUsage.exe --development | 退出 0；两项，up_to_date、安装类型 development，无错误/更新文件 |
| 无界面成品 | npm run test:headless | 退出 0；11 项 |
| 便携版原生更新 | npm run test:update:windows | 退出 0；三组：公开版本检查/设置、保留文件及 SQLite 的合成原地替换、中断替换恢复 |
| 文档类型及内容 | npm run check:docs | 退出 0；203 对仓库文档、21 对指南；35 个 Astro 文件无错误、告警或提示 |
| 文档单元测试 | npm run test:docs | 退出 0；39 项 |
| 静态文档构建 | npm run build:docs | 退出 0；1,377 页、2,885 文件，包含经校验的更新元数据 |
| 文档浏览器 | npm run test:docs:browser | 退出 0；31 项，含本机 JSON 端点 |

两项修复完成后重新构建了最终开发程序。npm 在启动探针前因自定义参数转发而返回
EUNKNOWNCONFIG；文档改用直接 Node 命令后检查通过。首次失败日志已保留。
最终验证使用隔离的自有可执行文件和数据。

新公开端点、远程 Release 触发刷新
及真实较新的公开版本升级，需要发布后另行观察。局部样本和本机静态端点不能建立
已部署回退验收。真实 NSIS 执行及 Linux/macOS 桌面更新验收保留
[原有范围限制](application-updates.md)。
2026-10-09 直接探测公开端点返回 HTTP 404，新快照尚未部署。
Markdown 检查 459 个文件，无问题，退出 0。

## 来源与制品

规则见[软件更新](../../design/desktop-usage/application-updates.md)。
2026-10-09 核对的一手来源：

- [GitHub REST 限频规则](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api)。
- [最新 Release](https://docs.github.com/en/rest/releases/releases#get-the-latest-release)。
- [Release 工作流事件](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#release)。
- [触发工作流](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/trigger-a-workflow)
  及[dispatch 权限](https://docs.github.com/en/rest/actions/workflows#create-a-workflow-dispatch-event)。

日志、独立数据库和报告位于忽略的 `build/update-check/`；浏览器制品位于根目录
`build/browser-smoke/` 及 `build/documentation-site/`。
