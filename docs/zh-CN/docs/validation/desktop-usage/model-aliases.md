# 模型映射验证

核对日期：2026-11-16。功能检查使用基于 0.3.10 的工作区。发布准备将全部应用版本来源和
锁文件同步为 0.3.11；用户已授权提交、带 changelog 的附注 tag 及推送。以下可执行文件
摘要和测试先于此次纯版本号变更，0.3.11 安装包由 tag CI 单独验证。
环境为 Windows 11 x64、Node 26.13.0、Rust 1.99.1 及本机 Edge/WebView2。
临时日志、隔离数据库和探测程序位于 `build/model-aliases-20261116/`。
来源、有效期、冲突和隐私要求见[映射说明](../../design/desktop-usage/model-aliases.md)。

## 已实现行为

- 带版本的元数据设置保留候选、精确范围、有效区间和 SHA-256 内容标识。导入及成功
  响应原文缓存更新共用一个 SQLite 事务，无须改变数据库结构或清空数据。
- 本地 JSON 导入和默认关闭的在线刷新独立于计价。默认缓存三天，可设置为 1–365 天。
  下载仅访问两个指定 HTTPS 地址，不允许重定向，不发送凭据或本机用量；失败保留成功缓存。
- 完整状态标记和单调递增的设置时间戳拒绝过期下载结果，涵盖手动导入、其他写入器
  以及下载期间关闭再开启设置的情况。
- 后端按精确 Agent/供应商范围及用量查询的相同保留快照解析名称。名称混合、缺口、
  较新映射到期或存在冲突时保留原始身份。明细优先使用已保存的原生 canonical 模型。
  原始 key、计数、未知值和计价保持独立。
- 设置提供十语言的导入、刷新、缓存天数和状态。元数据修订更新表格、筛选及图表名称，
  不改变用量修订或筛选/分组值。Qoder 内置回退仍为用户授权的
  `qmodel_latest` → `Qwen4-Max-Preview` 显示覆盖。

## 检查

未另行注明工作目录的命令均从仓库根目录执行。

| 命令/检查 | 结果与范围 |
| --- | --- |
| `cargo test --locked --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core --test model_alias_catalog` | 十项通过：解析边界、Agent/供应商/后缀范围、时间冲突与到期、幂等、回滚、旧结果/跨进程写入、只读缓存失效、加速/小时/归档查询、原生 canonical 优先级及公开种子的显式覆盖依据。 |
| `cargo test --locked --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-desktop model_alias_refresh` | 四项通过：旧设置/默认值、缓存时限、固定来源回退、首次错误/缓存保留和单调设置标记仲裁。HTTP 响应为注入数据，不代表公开部署验收。 |
| `cargo clippy --manifest-path desktop/src-tauri/Cargo.toml --workspace --all-targets --locked -- -D warnings` | 退出码 0。 |
| `cargo test --manifest-path desktop/src-tauri/Cargo.toml --locked` | 正常 Windows 凭据库权限、隔离临时路径下，1,181 项通过、十项跳过、零失败；既有跳过检查仍单列。 |
| `npm run test:ui` / `npm run check` | 34 项通过；Svelte 零错误、零警告。后端因冲突保留的原始名称优先于内置回退。 |
| `npm run test:browser` | 退出码 0。本机 Edge、合成 IPC：默认关闭、缓存天数草稿/保存验证、导入失败/成功、下载失败、仅元数据轮询、原始筛选/序列键和明细实时名称。既有交互断言保持启用。 |
| 在 `desktop/` 执行 `node node_modules/@tauri-apps/cli/tauri.js build --no-bundle` | 退出码 0，前端及 Windows release 程序构建完成。SHA-256：`fea71f04ecc6c480e687f9817db7126de43f65a0abffd2f3f90de346c48b137a`；不据此推导安装包生命周期验收。 |
| `node build/model-aliases-20261116/native-check.mjs` | 退出码 0，真实 WebView2/Tauri IPC，独立数据/来源/临时目录。核对默认关闭、原生采集、JSON 导入/幂等、仅元数据变化的界面刷新、原始图表/明细身份、完整用量行/修订号/总量不变、冲突拒绝、联网刷新关闭及重启持久化。使用合成 Codex 记录，不认证新的 Qoder 原生格式。 |
| `npm run lint:md`、`npm run check:docs`、`npm run test:docs`、`npm run build:docs` | 退出码 0：483 份 Markdown、215 对已复核仓库文档/21 对指南、Astro 零错误/警告、41 项文档测试、1,427 个构建页面，公开链接与标记检查通过。生成的映射目录与 396 字节源码完全一致。 |

## 失败与修正

首次受限进程的完整 Rust 检查在既有接收器测试中失败：`CredReadW` 返回 `0x80070005`，
在预期的端口占用分支之前产生 `credential_store_unavailable`。相同最终源码在正常
Windows 凭据库权限下通过。`rust-full.log` 和 `rust-full-native.log` 均保留，断言未删除。

浏览器映射检查发现，首次元数据状态轮询前发生更新，可能让旧名称继续显示。现已直接
比较实际加载的汇总修订号与后端状态。另一个首次运行断言计入了先前测试执行的费用
重算；映射检查改为比较进入时的计数，确认本身没有增加重算。初始日志与第三次通过
的浏览器日志均保留。

## 剩余限制

公开静态目录已准备在 `docs/site/public/models/aliases.json`，两个在线地址仍须随对应
文档/仓库发布，并独立核验 HTTPS。已授权的 main 分支推送会触发文档部署，实际 HTTPS
及部署完成状态另行检查。不声称存在 Qoder 官方的 key 到模型名称 API。
维护者仍须核对来源后更新目录。显示名称不能识别隐藏路由、
供应商回退或历史实际服务模型。这些检查不建立新的 Agent 原生读取器、账号 API、
其他平台 GUI 或安装包生命周期验收结论。
