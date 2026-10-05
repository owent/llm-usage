# 跨平台凭据与计划继续执行

日期：2026-10-05；应用版本 0.2.1。Windows 仓库 cwd：
`D:/workspace/projs/github/owent/llm-usage`。开工工作树无修改；基于
`0c5d63c19151e7d6f72a33c513f7ac9ddc0e2ad5` 的未提交工作树验证，未提交/推送/发布。
依据：[认证合同](../../design/desktop-usage/receiver-auth.md)、
[平台合同](../../design/desktop-usage/platform-ci.md)、[计划](../../../Plan.md)。

## 实现与核验依据

Linux 原实现一律拒绝系统凭据。现使用 `secret-service 5.2.0` 的 DH 会话、
当前用户默认集合与全集合查找；锁定、重复项和唯一 session 临时项都拒绝，
默认别名指向 session 同样拒绝。别名可修改的依据见
[Secret Service Aliases](https://specifications.freedesktop.org/secret-service/latest/aliases.html)。
不创建集合、不解锁；后台读取无提示，操作限定 3 秒。同步接口在独立线程运行
限时异步任务，嵌套 Tauri runtime 不引发 panic。错误只返回固定代码。
写入已保存但回复失败、或保存后回查失败时，仅回收与本次写入完全相同的项，
外部替换保留。

macOS 使用 `security-framework 3.7.0`、`security-framework-sys 2.17.0`，
所有查询/写入/删除均指定非同步 Keychain 和认证 UI Fail。库遗漏的 Apple Fail
常量以单个原生符号声明补齐；`aarch64-apple-darwin` 类型检查通过，未原生运行。
两平台随机源为 `getrandom 0.3.4`；Windows 原生实现保留。

核验了库正式发布元数据和下载源码，而后实施；来源：
[Secret Service 规范](https://specifications.freedesktop.org/secret-service/latest/)、
[secret-service 5.2.0](https://docs.rs/secret-service/5.2.0/secret_service/)、
[Keychain PasswordOptions](https://docs.rs/security-framework/3.7.0/security_framework/passwords/struct.PasswordOptions.html)、
[Apple UI Fail](https://developer.apple.com/documentation/security/ksecuseauthenticationuifail)。
这些来源不证明真实 Agent exporter 已验收。

Rust CI 从 Linux 单作业扩为 Windows/macOS/Linux 三平台矩阵，保留 fail-fast=false。
Linux 原生验证另用独立 D-Bus、一次性加密 keyring 和随机测试密码；密码/绑定仅经
stdin，服务探测先检查名称所有者，避免触发第二守护进程。真实 Linux 制品报告发现
Debian `control.tar.gz`/`data.tar.gz` 被列为发布制品；现报告及上传仅取安装包、
macOS `.app.tar.gz` 与校验和报告，新增回归及真实打包目录复核通过。

## 环境与结果

Windows：Windows 11 x64、Node 24.21.0、Rust 1.98.0；Edge/WebView2。
WSL：Debian 13.7 x86_64、内核 6.18.40.1、Node 24.21.0、Rust 1.98.1，
WebKitGTK 2.54.0、gnome-keyring 48.0。按用户授权用 sudo apt 安装缺失系统依赖，
用 rustup 补齐 Clippy/rustfmt；Windows/WSL 的 npm、target 和活库不共用。
Linux 独立副本包含当前未提交源码；revision/差异清单保存在根 build/platform-auth/。

| 命令 / 检查 | 退出码 / 实际结果 | 范围 |
| --- | --- | --- |
| Windows `npm run verify` | 0；Rust 904（核心 808、应用 96），前端 21、脚本 4；类型无错误/告警 | Markdown、资源、fmt、Clippy、SQLite/合同/集成、前端构建；应用默认忽略 8 项环境测试 |
| Windows `npm run build:desktop` | 0；release/NSIS 产出 | 未安装 |
| Windows `npm run test:headless` | 0；11 项、3 事件/75 token | 隔离合成源，真实可执行文件/SQLite |
| Windows `npm run test:desktop -- --runs 1` | 0；17 项、3 个首屏样本，P95 850.1 ms | 真实 WebView2/IPC/托盘/暂停/分钟任务；不代替完整资源或辅助技术实测 |
| Windows `npm run test:receiver` | 0；8 项，自有凭据残留 0 | 合成 exporter 安装清单，真实 IPC/HTTP/凭据库 |
| Windows 显式原生凭据测试 | 0；跨进程读取/撤销、真实 HTTP 撤销/另一来源继续，各 1 项 | 仅随机自有凭据；不写真实 IDE 配置 |
| Windows `npm run test:browser` | 0 | Edge 模拟 IPC，十语言/主题/选区/遥测批量配置 |
| Debian `cargo clippy --workspace --all-targets --locked -- -D warnings` | 0 | 最终平台实现与测试目标 |
| Debian `cargo test --locked`，最终应用专项复测 | 0；核心 808、应用 95；应用默认忽略 9 项 | 核心完整合同/SQLite/适配器，Linux 超时/嵌套 runtime；显式原生项另列 |
| Debian `npm run test:credentials:linux` | 0；6 项（五个原生测试，缺服务项也用于临时默认集合） | 缺服务且无监听、跨进程持久读取/撤销、真实 HTTP/来源隔离、重复及临时集合/默认别名拒绝、锁定；一次性 keyring 与守护进程已回收 |
| Debian `npm run build:desktop -- --bundles deb,appimage` | 0；release、deb、AppImage 均产出 | 标准系统依赖，未使用旧 sysroot/WebKit 路径补丁；未安装 |
| Debian ELF / 提取 AppImage `test:headless` | 均 0；各 11 项、3 事件/75 token | AppImage 原样提取后执行 AppRun；不代表 FUSE 挂载、WSLg 或原生 GUI 验收 |
| macOS 模块交叉 `cargo check` / `cargo clippy`（`aarch64-apple-darwin`、`--locked`） | 均 0；Clippy `-D warnings` | Windows 临时最小检查工程直接引用实际模块；类型检查，无原生链接/运行 |
| 制品报告脚本回归及真实 Linux 目录 | 0；4 项；实际报告仅 deb/AppImage 两项 | Debian 暂存文件不作为发行包 |
| 最终文档与差异 | 0；182 Markdown、9 份受影响 Markdown 的 97 个本地引用/锚点、`git diff --check` | 临时产物全部位于根 build/ |

Windows 完整命令输出及 Linux 分项日志在 `build/platform-auth/`；Linux 临时检查库
位于独立副本的根 build/。终端初始 PowerShell 启动及子进程限制以允许的 cmd/执行
边界重试，不计为产品失败。

Windows 显式原生测试的一次并行复测在第二次建立凭据时返回
`credential_store_unavailable`；单项与 `--test-threads=1` 复测均退出 0。
失败日志保留为 windows-native-final.log。实际模块的临时诊断副本仅扩充读/写错误码，
两线程 40 次建立/撤销未复现错误，自有项残留 0。沿失败日志时间戳确定唯一合成
测试目录，凭据命名空间内核对完整绑定，仅该目录的一项自有凭据仍存在，已精确回收；
不回收其他应用/测试目录。结果在 windows-vault-concurrency.log。
这证明失败路径曾留下凭据，原因尚未确认；系统存储失败后的回收可靠性继续待查，
不据诊断未复现报成已修复。

Debian 重跑曾出现两项既有夹具失败：`source_intervals` 按 PID 重开已禁用的
来源库，`process_guard` 按 PID 重开含 `active` 主键的库。只读核对旧合成库分别
发现五个禁用来源与五个已存在运行；现两项均每次独立建目录，拒绝复用并在
成功后关闭/回收。保留该状态证据于 pid-fixture-evidence.log，修复后应用 95 项通过。

## 制品

| 制品 | 字节 | SHA-256 |
| --- | --- | --- |
| Windows 主程序 | 9,901,568 | `09de6cfa17d37e9195e910db3647ea58cab2417fe059f352b4d9d62835eae1a8` |
| Windows NSIS | 3,913,043 | `993e255bff157f8e980180503bde5053378f3411f37333674ea587ba226919d8` |
| Linux deb | 5,440,738 | `3895d451211bf69beb87e570168431dc460831e1d157c701199a4891cd8b1a9f` |
| Linux AppImage | 111,122,936 | `9caebe7f4507d78190a504f0ce5a19645e006c99135a87d48ebb38fbea3ccea3` |

Windows 包在 desktop/src-tauri/target/release/bundle/nsis/；Linux 包已复制到
根 build/platform-auth/artifacts/。deb 的 Depends 与架构已查；未做安装/升级/卸载。
当前 Windows 主程序/NSIS 另复制到该目录的 windows/，报告只覆盖本轮版本，
不将原 bundle/ 中的旧版本标为本轮 revision；本轮报告均记录基准 SHA 与 uncommitted。

## 真实样本与剩余条件

审阅既有 `local_availability` 后，分别在 Windows 与 Debian 只读执行真实注册表
discover/detect，最多探测每根前三个文件，仅输出数量和版本元数据。
Windows Gemini/Qwen、Junie 及其余缺样本来源未发现；Zed 一个库的 threads 为 0 行，
不能认证用量格式。Debian 当前默认发现无可用来源，未启动 Agent/模型请求制造数据。
已有 Codex/pi/omp/Kilo/ZCode/Kimi/Copilot 等发现结果不重复当作新来源验收。

仍待真实样本、macOS 原生存储/桌面、Linux 原生 GUI、拟定硬件/DPI/真实辅助技术、
完整升级回滚/注销/卸载、其他 exporter/版本认证、重传/采样/父子 span/跨载体以及
全进程出站审计。远端 CI 未触发，三平台 workflow 修改不代表远端通过；安装按
既有指示跳过。F1、明细 Merge 与预算提醒继续按计划后置。
