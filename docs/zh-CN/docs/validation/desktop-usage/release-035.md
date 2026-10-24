# 0.3.5 发布准备

日期：2026-10-25（Asia/Shanghai）。用户已授权提升版本、提交、创建附注 tag 并推送，
触发打包。候选版本在父提交 `f2df0a6e1e45ba1cef96494459f3a0e035dd1417` 上，
包含已有依赖和数据源维护改动，以及 Kimi K3.1 Pro 计价修复。

## 改动与版本一致性

- 增加 K3.1 Pro/Highspeed 独立的国内、国际离线价目，明确 Kimi Code 模型别名，
  覆盖旧库导入及当前价参考回归，保留历史金额。
- 包含 [维护记录](maintenance-2026-10-24.md) 中的兼容依赖和数据源修复：SQLite 取消、
  来源分页、Codex 冲突、有界 Kimi 读取、Qoder 检测及 NSIS 模板更新。
- 根目录与 desktop 的 npm 清单和锁文件、两个 Rust 包及 Cargo.lock、Tauri 配置和
  Plan 当前版本统一为 0.3.5。共享 pnpm 锁中没有需要修改的工作区版本字段，依赖解析
  保持已有维护结果。

打包脚本的 `releaseVersion()` 检查确认所有版本来源均为 0.3.5。此前 0.3.4 的原生及
来源验证仍为历史结果，提升版本号不代表重跑这些检查。
[计价记录](kimi-k31-pricing.md) 保留原始核验日期和结果。

## 验证与发布

Windows x64、Node 24.21.0、Rust 1.99.1。检查均在仓库根执行，退出码为 0：

| 命令 | 结果 |
| --- | --- |
| 打包脚本 `releaseVersion()` | npm、Tauri、Rust 的版本声明及锁文件均为 0.3.5 |
| `npm run verify` | 1,127 项 Rust 测试通过，九项需显式启用或限定平台的测试忽略；20 项脚本及 32 项前端测试通过；Markdown、资源、Svelte、rustfmt、Clippy 和前端构建通过 |
| `npm run check:docs` | 211 对仓库文档和 21 对指南；Astro 无错误、警告或提示 |
| `npm run test:docs` | 41 项测试通过，包含新增计价及发布记录的导航条目 |
| `git diff --check` | 通过；另行审阅新增文件 |

首次文档测试因菜单固定数量仍为 229 而失败，新增两篇记录后实际为 231。
同步数量预期并明确断言两个新路由后，完整 41 项测试重跑通过。临时日志及独立
测试数据库位于已忽略的 `build/release-035/`，临时文件不进入暂存区。

附注 tag 为 `v0.3.5`，非空附注提供 Release 正文。现有 `.github/workflows/ci.yml`
由 tag 推送触发，检查文档、前端和 Rust，并构建六种原生平台与架构组合。全部前置作业
成功后，创建或更新 Release 草稿，包含六个便携压缩包、Windows x64 NSIS 安装程序及
六份报告。草稿完成前检查提交版本、文件大小及 SHA-256。

推送 tag 和触发工作流与远程打包成功、公开 Release 分别判断。本地准备记录不代表
远程已完成或下载制品已核验，工作流沿用已有草稿发布行为。

## macOS 取消测试修复

用户报告的 [main CI 作业](https://github.com/owent/llm-usage/actions/runs/40112357943/job/120926546175)
对应提交 `03443085d4900f8f268ebead2b21a465a2e3693d`，在 macOS 15 上以退出码 101
失败：281 项 core 单元测试通过，`cancellation_rolls_back_sql_and_stops_the_interrupt_worker`
失败。格式、Clippy 和测试编译均通过。Windows/Linux Rust 作业及六个便携包构建作业
均通过。同一提交的独立 [tag 运行](https://github.com/owent/llm-usage/actions/runs/40112357959)
通过，符合测试偶发失败的表现。这些结果不代表已核验下载后的发布包。

原测试在取消操作后，预期固定写入 100,001 行的递归语句失败。取消线程每 20 毫秒轮询
一次，语句可能在下次中断前完成。[SQLite 文档](https://www.sqlite.org/c3ref/interrupt.html)
说明，接近完成的语句可能继续完成；没有运行中语句时调用中断，不会影响后续语句。
SQL 工作量无法确定测试需要的执行顺序。

两项 SQL 取消测试现使用标量函数回调和条件变量，保持 SQL 执行，直到真实工作线程调用
`sqlite3_interrupt()`，随后由短递归步骤检查中断标志。仅测试使用的计数器在中断调用和
通知期间持锁，防止此前没有运行中语句时的中断满足回调的等待条件。测试明确要求
`OperationInterrupted`、事务自动回滚、原有数据保留，以及等待工作线程退出后连接可复用。
提前取消测试还会在启动后续语句前，等待一次没有运行中语句时的中断。生产环境的取消和
轮询行为保持不变；两秒等待仅限制缺少工作线程通知的情况，不要求 SQL 执行至少多长时间。

本地首次仅使用回调的 INSERT 在中断后仍成功完成，因为后续没有检查中断标志的执行步骤。
补充短递归步骤后，两项测试均通过。临时仅禁用 `interrupt.interrupt()` 后，即使通知成功，
两项测试仍以退出码 101 失败；随后已逐字节恢复源码。

最终修复检查在仓库根执行，环境为 Windows x64、Node 24.21.0、Rust 1.99.1。
下列命令退出码均为 0；上文单独保留临时禁用中断的预期失败结果。

| 命令 | 结果 |
| --- | --- |
| core 测试程序，`cancellation::tests --quiet`，200 轮 | 600 项测试通过 |
| `npm run test:rust` | 1,127 项通过，九项需显式启用或限定平台的测试忽略 |
| `npm run clippy` / `npm run fmt:check` | 整个工作区严格 Clippy 及格式检查通过 |
| `npm run lint:md` | 475 个文件，无问题 |
| `npm run check:docs` | 211 对仓库文档、21 对指南；Astro 无错误、警告或提示 |
| `npm run test:docs` | 41 项通过 |
| `git diff --check` | 通过；仅修改本次修复及同步记录 |

日志位于已忽略的 `build/ci-macos-40112357943/`。这些本地检查不代表修复已在 macOS
执行；推送后仍需根据修复提交核对 CI。
