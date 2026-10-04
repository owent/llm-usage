# 应用图标、静态资源与 Git LFS 验证

日期：2026-09-24。环境：Windows x64、PowerShell 7、Node.js 24.21.0、Git LFS 3.7.1；
Tauri CLI 2.11.5、tauri-build 2.6.3，使用现有锁文件，未增加依赖。
工作区已有其他未提交的 M0 修改，本任务只叠加资源及其消费、构建和 LFS 配置。
设计、文件清单与复现命令见 [资源合同](../../../desktop/assets/README.md)。

## 实际结果

| 命令或检查 | cwd | 退出码 | 结果 |
| --- | --- | --- | --- |
| `node desktop/scripts/assets.mjs --generate` | 仓库根 | 0 | 导出 67 个派生文件，连同源文件/UI/插画共检查 82 个资源 |
| `npm run assets:check` | 仓库根 | 0 | 重建并比较派生文件；PNG RGBA8、ICO 尺寸、ICNS 图层、Tauri 引用和 LFS 属性通过 |
| `node desktop/scripts/assets.mjs --help` | 仓库根 | 0 | 无交互输出参数、依赖与副作用 |
| `npm run check` | 仓库根 | 0 | svelte-check：0 errors、0 warnings |
| `npm run build:web` | 仓库根 | 0 | favicon、品牌 SVG、UI 和插画进入前端产物；审阅页不进入 release 入口 |
| `cargo fmt --manifest-path desktop/src-tauri/Cargo.toml --check` | 仓库根 | 0 | 构建脚本格式通过 |
| `node node_modules/@tauri-apps/cli/tauri.js build --bundles nsis` | desktop | 0 | 修复图标依赖后重新构建 Windows release 和 NSIS 包 |
| Edge headless + CDP（临时 `build/check-assets-browser.mjs`） | 仓库根 | 0 | 35 张图片均加载；0 脚本异常；主题按钮切换通过；1440 px/390 px 宽度无横向溢出 |
| LFS 内容检查（临时 `build/check-assets-lfs.mjs`） | 仓库根 | 0 | 94 个索引指针与工作树、本地 LFS 对象的 SHA-256/大小一致；已跟踪二进制无遗漏 |
| `git lfs fsck --objects --pointers` | 仓库根 | 0 | 最新 HEAD 的 LFS 对象及指针检查通过 |
| 原型文件迁移前后比较 | 仓库根 | 0 | 12 个原有文件与迁移前 `f7c689c` 的原字节一致，包括 9 个 pyc、数据库、预构建 JS 和静态 JSON |
| YAML 解析 | 仓库根 | 0 | 4 个 CI job 的 checkout 均设置 `lfs: true`；前端与三平台 build 加入资源检查 |
| `npm run lint:md`、`git diff --check`、`git diff --cached --check` | 仓库根 | 0 | 文档和差异格式通过 |

LFS 文件逻辑大小合计 9,121,435 字节，包含已有原型文件；不代表远端传输量。
迁移只更新当前索引，历史普通 Git blob 保留；本任务未发起提交、推送或历史改写。
收尾时检测到并行工作创建了 `3588345`（M0），已包含资源、LFS 配置和构建修复；
在该 HEAD 上再次运行资源检查通过；此验证记录及引用在工作区另行补充。
本轮新增设计源、图标、UI 和插画均为 SVG 几何图形，没有外部字体或图片下载。

最终 NSIS：`desktop/src-tauri/target/release/bundle/nsis/llm-usage-m0_0.1.0_x64-setup.exe`，
1,892,808 字节，SHA-256：
`98698514D0626D30D0AAB7B572D7A47F951081B2F8A373D3DE765D2DC25779FD`。
从复制到新路径的 EXE 提取图标，目视确认已是 Usage U，排除原路径 Shell 缓存。
本次未安装该包或启动桌面进程。

## 发现并修复的问题

- Tauri CLI 2.11.5 两次输出的 ICNS 各图层哈希相同，但排列顺序不同。
  生成脚本固定图层顺序并保留旧式 RGB/透明度配对；修复后全部派生资源字节一致。
- 首次 release 打包成功，但新路径 EXE 提取仍得到旧蓝色图标。
  已安装的 tauri-build 2.6.3 源码在 Windows 资源编译时未声明 ICO 的 `rerun-if-changed`；
  `build.rs` 显式追踪 `icons` 目录后，重新编译、打包及提取图标通过。
- 初次预览服务在配置重载期间遇到目标文件监听 `EBUSY` 并退出。
  在同一验证进程中启动/关闭服务后重跑浏览器检查通过；本任务未改写其他任务的 Vite 配置。
- 截图检查发现空状态标题与行内图片并排换行，已改为块级布局；显式设置浅色预览的 color-scheme。

<a id="证据与未执行项"></a>

## 验证结果与未执行项

本机忽略目录 `build/assets-validation/` 保存浅色/深色/390 px 截图、浏览器检查 JSON、
LFS 检查 JSON 与最终 EXE 图标；临时浏览器和 1428 端口服务已关闭。
截图只包含本任务资源审阅页，没有截取桌面其他窗口。

GitHub CI、远端 LFS 上传/下载、macOS/Linux 桌面渲染和 Windows 安装后任务栏显示未执行。
托盘、导航及空状态业务接入仍属 M6；本次交付这些静态资源，不启用后台任务或采集功能。
Windows release/EXE 提取不能替代上述平台与安装验收。
