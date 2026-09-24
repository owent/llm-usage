# M0/M1 提交审查与回归修复

2026-09-24 审查 `3588345`（M0）和 `9394264`（P1/M1）。开始时工作区干净；
本次修改未提交、未推送。环境为 Windows x64、PowerShell 7.6.6、Node 24.21.0、
Cargo/Rust 1.98.0；依赖继续使用现有锁文件。

## 已修复的问题

| 优先级 | 触发与影响 | 修复与回归 |
| --- | --- | --- |
| P1 | 日汇总与原生区间汇总把 estimated 字段加入“已知用量”，派生 total 又丢失估算质量 | 按字段只汇总 reported/derived；保留已报告的其他字段和调用数；校验值与质量一致 |
| P1 | Codex/ZCode/Kilo 在缺少缓存创建、缓存读取或推理字段时补出已知拆分；来源 total 不一致时被作为规范化总量；缓存求和可溢出 panic | 缺失保持未知；规范化总量与 source_total 分离；检查加法；比例保留 i128 整数对 |
| P1 | 普通事件接口接收累计/区间/额度类型，按日直接累加；混源批次推进错误来源游标；不存在或已结束的作业仍能提交批次 | 类型走专用存储接口；批次来源、作业归属与 running 状态在事务内核验 |
| P1 | 带 `#` 的身份组件可发生主键碰撞；重扫只改变 observed_at 也会产生冲突 | 转义身份组件；忽略采集观察时间的内容比较；兼容 v1 摘要与现有别名 |
| P1 | 来源区间汇总同修订号冲突被覆盖，诊断与用量不在同一事务，数据修订号不更新 | 不确定更新保留旧值并记录冲突；汇总/诊断/修订号原子提交；未知调用汇总返回 None，并附已知/未知行数 |
| P1 | 明确重置后新累计值大于旧值时误算差值；迟到值移动当前基线 | 先判断采样时序与重置证据；迟到/同刻冲突保留基线；新 series 独立处理 |
| P1 | 保留清理遇到 event_aliases 外键失败；重启后漏传截止会复活过期数据；硬期限未约束诊断与来源区间 | 先清理涉及过期事件的别名；保存只前进的保留下限；硬期限作用到诊断、额度和原生区间及重扫 |
| P2 | 周/月 token 按选定日期过滤，会话数却查整个自然周期；无 session 的活动日丢失；明细与排除计数筛选不一致 | 会话/活跃日限定同一日期范围；活动日不依赖 session；所有查询使用一致筛选和读快照 |
| P1 | 拒绝新 schema 前先修改 journal_mode；改完计算逻辑后旧日汇总仍保留错误值 | 先读取版本再变更设置；schema v2 事务迁移身份与别名，重算有明细的汇总；迁移中途失败整体回滚 |
| P1 | M0 安装后的文件探测依赖编译机器源码路径；并发探测共用临时数据库 | 配置 bundle.resources，通过 Tauri resource_dir 读取；每次 SQLite 探测使用独立临时目录并清理 |
| P1 | Linux Rust 检查作业未安装 Tauri 系统库；制品检查把 `.app` 当文件而漏掉它 | 补齐 Rust 作业依赖；Node 脚本检查实际制品、写大小/SHA-256/revision；先归档 `.app` 再上传以保留权限和符号链接 |

`review_regressions.rs` 新增 27 个核心回归，桌面入口新增 2 个测试，
`bundle-report.test.mjs` 新增 3 个测试。核心回归使用真实临时 SQLite 文件库，
包括并发读写、v1 升级、迁移失败回滚、封存缺失明细和故障触发器。
第一批 17 个用例已先在原实现上运行，17 个全部失败；修复后全部通过。
后续新增的区间冲突与原子性用例也先复现失败再修复。

## 兼容与证据边界

- v2 重算仍有明细的旧日汇总；已封存的 estimated 分区无法恢复逐字段已知量，
  保留调用数并将 token 标为不可恢复，记录 `sealed_estimate_unavailable`，不推测为零。
- 硬期限下，起点未知或横跨截止的原生区间不能精确拆分，整条移除并阻止普通重扫恢复。
  v1 在清理时没有保存截止；已被 v1 删除且没有留下封存记录的历史无法倒推出旧截止。
- 源适配器、跨来源别名选择/去重、时区切换重建、迁移前一致备份/空间检查、
  应用管理备份清理及容量/性能基准仍按原计划推进；本次不把表结构测试记为这些功能验收。
- Tauri 资源映射依据本地锁定的 `tauri-utils 2.9.3` 配置定义与
  `tauri 2.11.6` 的 `path/desktop.rs` 实现，已核对平台资源目录行为。
- GitHub 三平台 CI 尚未实际运行；本机归档脚本测试不能代替 macOS 桌面运行或制品下载验收。

## 命令与结果

除注明外，cwd 为仓库根。Windows 沙箱首次启动报 `CreateProcessAsUserW failed: 5`，
平台允许的执行路径恢复后才开始读库/测试；该错误不是产品测试失败。

| 命令 | 退出码 | 结果 |
| --- | --- | --- |
| 修复前 `cargo test --manifest-path desktop/src-tauri/Cargo.toml --locked -p llm-usage-core` | 0 | 原有 72 个测试通过 |
| 第一批 `cargo test ... --test review_regressions` | 1 | 17 个新用例均复现失败 |
| 最终 `cargo test ... --test review_regressions` | 0 | 27 passed / 0 failed |
| `npm run verify` | 0 | 最终 99 个核心测试 + 2 个桌面测试 + 3 个脚本测试全部通过；34 个 Markdown 文件无问题、67 项派生资源/82 个文件核验通过；svelte-check 无错误或警告，fmt/clippy/前端构建通过 |
| `npm run build:desktop -- --bundles nsis` | 1 | npm 12 嵌套脚本参数转发拒绝 `--bundles`；改用锁定 CLI 直接调用 |
| `node node_modules/@tauri-apps/cli/tauri.js build --bundles nsis`（desktop） | 0 | Windows x64 release 与 NSIS 成功；安装包 1,894,207 B |
| `7z l desktop/src-tauri/target/release/bundle/nsis/llm-usage-m0_0.1.0_x64-setup.exe` | 0 | 安装包内含 190 B 的 sample-data.txt；构建资源与源码样本 SHA-256 相同 |
| `node desktop/scripts/bundle-report.mjs desktop/src-tauri/target/release/bundle` | 0 | 实际 NSIS 大小与 SHA-256 写入 bundle-report.json |
| `git diff --check` | 0 | 无空白错误 |

本次安装包 SHA-256：`4e382336b14980fd5a400061a8d958bd402ff7d0886ffa714e43187a78cf43da`。
制品保存在已忽略的 `desktop/src-tauri/target/release/bundle/`。

未启动或调用真实 Agent，未读取私人会话；未安装或发布本次生成的安装包。
