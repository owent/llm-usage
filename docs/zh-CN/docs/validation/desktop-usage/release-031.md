# 0.3.1 依赖与发布核验

## 范围与依据

2026-10-09 用户授权将所有依赖升级到尽可能新的兼容版本、更新版本号到 0.3.1、
创建 v0.3.1 tag 并推送驱动发布，同时授权纳入 pnpm 配置并同步锁文件。
更新检查修复单独保存在前置提交 `28952cb9`，原始核验依据保留在
[修复记录](update-check-repair.md)。

逐项核对官方 npm registry 的 latest tag 和 crates.io 的最新稳定版，覆盖所有直接依赖。
Actions 根据上游发布 tag 核对，并解析为固定提交 SHA。
锁文件选择兼容的间接依赖版本。

| 组件 | 选定版本 |
| --- | --- |
| Astro / Starlight | 7.3.8 / 0.42.6 |
| Svelte / Vite / Playwright | 5.57.2 / 8.3.4 / 1.64.0 |
| Tauri API 与 crate / CLI / build crate | 2.12.2 / 2.12.1 / 2.7.1 |
| zstd / fs4 / SHA-2 / Unix getrandom | 0.14.0 / 1.1.0 / 0.11.0 / 0.4.3 |
| TypeScript | 6.0.3；编译器 API 兼容性例外 |
| pnpm / 本地生成锁文件的 npm | 12.10.1 / 12.2.0 |
| CI 与仓库 Rust 工具链 | 1.99.0；桌面 MSRV 1.90 |

TypeScript 7.0.2 是 registry 的最新版，但 Astro 和 Svelte 检查器声明支持 TypeScript 5 或 6，
且调用编译器 API。TypeScript 7 尚无该 API，详见
[官方发布说明](https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/)。
两个 manifest 和全部锁文件保留 6.0.3，没有强制忽略 peer 兼容性。

Cargo 保留四个受上游约束的较旧间接依赖。`crypto-common` 将 `generic-array` 固定为 0.14.7。
GTK 的 `proc-macro-crate` 将 `toml_datetime` 固定为 0.6.3、`toml_edit` 固定为 0.20.2，
使 `toml` 保持在 0.8.2。隔离尝试解析 `toml` 0.8.23 时，因精确 datetime 约束失败；
探测没有修改产品锁文件。

## 兼容性修复与包管理器

Rust 1.99 检出了不必要的借用，并弃用了测试计数器的原子更新方法。
移除该借用，计数器改用桌面所声明 MSRV 支持的 compare-exchange 循环。
SHA-2 0.11 返回的数组没有 `LowerHex`；显式逐字节补零的小写格式保留文件、目录及缓存摘要。
固定向量测试验证空文件和非空文件的摘要，并拒绝错误摘要。

使用 npm 12.2.0 和官方 registry 根据 manifest 重新生成根目录与 desktop 的 npm 锁文件。
首次干净安装因 npm 12 下载策略拒绝原锁文件中的腾讯镜像 URL；新锁文件仅含官方 npm
registry URL。没有强制安装不兼容 peer，也没有放宽全局策略。

pnpm 工作区包含根目录与 desktop；锁文件记录两个 importer 和固定的包管理器。
隔离冻结安装执行获准的 esbuild 脚本，跳过嵌套 npm postinstall。
CI 在 npm 前端检查后，还验证 pnpm 冻结安装、前端类型与构建及文档类型。

Tauri CLI 2.12.0 和 2.12.1 的官方 NSIS 模板逐字节相同：
32,061 字节，SHA-256 为 `dabed59013b1d78b879a1a85bc7f2eed2993b33a9a90cdabe5946de3d3950597`。
现有安装与回滚定制继续适用。

## 本地核验

环境：Windows 11 x64、Node 24.21.0、Rust 1.99.0，干净恢复使用 npm 12.2.0，
pnpm 为 12.10.1。任务日志与独立合成数据库位于根目录 `build/`；
本轮依赖核验未使用个人 Agent 数据。

| 命令或检查 | 结果 |
| --- | --- |
| 根目录与 desktop 的 npm 干净恢复 | 均通过，使用官方 registry 锁文件 |
| pnpm 冻结安装、严格 peer、前端检查与构建 | 通过；两个 importer 与每项 manifest 依赖一致 |
| `npm run verify` | 通过：1,054 项 Rust 测试，8 项平台测试显式忽略，13 项脚本测试，22 项前端测试，无类型警告 |
| `npm run test:browser` | 通过：软件更新、设置、费用布局、十种语言及现有交互 |
| `npm run build:desktop` | 通过：0.3.1 Windows x64 可执行文件与 NSIS 安装包 |
| `npm run test:headless` | 11 项通过，隔离的可执行文件与 SQLite |
| `npm run test:update:windows` | 通过：三组原生合成便携版替换与恢复 |
| `npm run test:update:check` | 两项通过：未知包类型真实检查公共版本，缺少类型身份时禁止下载 |
| 内嵌前端的 debug 构建与开发模式 IPC | 两项通过：`package_kind=development`，真实公共版本检查无错误、不选择制品；拒绝直接下载 |
| `npm run test:receiver` | 八项通过，无遗留的自有凭据 |
| `npm run test:desktop` | 19 项原生 WebView2/IPC 检查通过 |
| Windows 便携版打包 | 通过：解压后 headless 检查、zstd 完整性、大小与 SHA-256 报告 |
| 文档检查、构建与浏览器 | 通过：Astro 类型无错误、39 项单元测试、1,379 页及 2,889 个文件、31 项浏览器检查 |

2026-10-09 根据真实公开的 v0.3.0 发布刷新了源码中的更新快照；v0.3.1 草稿不属于公开稳定更新。
下述远端结果与上述本地检查分别核验。
现有 tag 工作流在所有平台作业通过后生成发布草稿；这不建立签名、公证、
新的 NSIS 安装生命周期或其他平台更新 GUI 的验收结论。

## 远端发布与部署

源码提交：`55b440e379a020e991c501f74b84d6b7d2ef0706`，附注 tag 为 `v0.3.1`，
2026-10-09 与 main 原子推送。核验后未移动该 tag。

- [Tag CI 37879955917](https://github.com/owent/llm-usage/actions/runs/37879955917)：
  12 个作业全部通过，包含发布制品检查。六种原生归档通过解压后的可执行文件检查；
  两种 Linux 归档还通过 GTK/WebKit GUI 检查。
- [Main CI 37879956064](https://github.com/owent/llm-usage/actions/runs/37879956064)：
  11 个适用作业全部通过，仅 tag 执行的发布作业被跳过。
  两轮 Windows Rust 检查均在 105 个测试二进制中通过 1,054 项测试，八项显式忽略。
- Release ID 为 `407495650`，发布草稿含 13 个文件：六个便携归档、Windows x64 NSIS 安装包
  和六份平台报告。全部下载后与 API 大小及 SHA-256 一致。
  六个 zstd 流、TAR 结构与 NSIS 压缩归档的完整性检查通过。
  每份报告均与准确的 tag 源码提交及其安装包一致。
- 下载的 Windows x64 便携包通过 19 项原生 WebView2/IPC 检查；
  20 次查询采样的本地 P95 为 912.61 ms。该结果对应 CI 构建的可执行文件；
  本地构建的原生检查结果在上文分别记录。
- [文档 CI 37879956018](https://github.com/owent/llm-usage/actions/runs/37879956018)
  构建与发布成功，公开发布标记与源码提交一致。
  `/updates/latest.json` 从 HTTP 404 变为 200；有效期内的 v0.3.0 快照与七个公开包的名称、
  大小和摘要逐项一致。

首次草稿制品探测使用 GitHub 的已发布版本 tag 接口，得到 404。
分页枚举发布列表后定位到准确的草稿 ID，最终检查通过；没有重建发布或 tag。
草稿说明已补充更新检查修复、依赖升级、pnpm 工作区及制品核验结果。

验收记录后续提交仅修改文档与计划，其文档工作流独立于 tag 的产品 CI 验证并发布更新记录。
v0.3.1 仍为草稿时，公开 Release 触发的元数据刷新和更高公开版本更新尚未实测。
