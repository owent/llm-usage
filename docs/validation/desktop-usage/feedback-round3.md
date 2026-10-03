# 第三轮反馈：今日曲线、费用布局与 Junie 来源

2026-10-03，续 [第二轮修复](feedback-round2.md)。本轮按源码和本机数据库只读快照
定位问题；不写正式库或 Agent 文件。临时脚本、快照、回放库和日志位于
build/feedback-round3/，浏览器截图位于 build/browser-smoke/，均为忽略目录。

## 原因与修复

- 今日逐小时的分组分支使用 calls；单小时另走调用柱图，因此只改总览曲线不能修复
  分组视图。现模型、Agent、Agent＋模型共用逐小时 token 曲线，单小时也保留时间轴。
  已知零显示零；缺失完整总量保留缺口，不用调用数或输入下界代替完整总量。
- token 提示给每个缺失总量的序列重复追加整段 Copilot 解释及输入/输出。
  现统一短格式 `名称: 数值`、`名称: ≥ 数值` 或 `名称: —`，下界说明只显示一次；
  完整说明保留在短提示的悬浮说明。共用图表层限制提示宽度，并允许长名称换行。
- 费用摘要卡重复展示部分估算计数，费用面板展开全部覆盖解释，造成额外高度。
  摘要卡紧凑显示原币金额及适用的美元参考，覆盖计数保留在提示中；面板保留金额、
  缺口计数与曲线，将覆盖说明和模型单价放在可展开入口，窄窗口自动换行。
- Junie 报错来自旧版遥测目录广播形成的空误登记。真实快照中该实例的目录正是
  应用遥测父目录，无文件、用量或诊断，两次失败均为 source_files.file_id 唯一键冲突。
  Junie 的 discover 会把传入 events.jsonl 所在目录提升到父目录；上一轮按原根精确
  路径恢复，漏掉了这个变换后的实例，并非此次找到真实 Junie 会话后读取失败。

来源恢复现用整个适配器注册表的 discover 重现旧管理根产生的候选路径，再按适配器
和精确路径恢复无用量历史的误登记。保留设置、文件、诊断和周期历史；其他真实来源的
错误仍可见。回归测试覆盖父目录变换及重复恢复，避免只为 Junie 再增加特例。
合同同步见 [看板交互](../../design/desktop-usage/dashboard-polish.md)、
[Copilot 遥测](../../design/desktop-usage/copilot-otel.md)与根 AGENTS.md。

## 验证

环境：Windows x64、Node.js 24.21.0、已安装 Edge；依赖以锁文件为准。

| 检查 | 结果与范围 |
| --- | --- |
| npm run verify | 退出 0；Markdown、资源、3 项脚本测试、20 项前端测试、Svelte、fmt、clippy、820 项 Rust 测试及 Web 构建通过 |
| source_routing 测试 | 5 项通过；真实 discover 生成候选路径、管理根定向路由、物理文件归属、历史保留及幂等恢复 |
| 本机快照副本恢复 | 两轮退出 0；Junie 可见错误实例 1→0，误登记保留为 not_applicable；事件、文件和诊断计数不变 |
| npm run test:browser | 退出 0；真实 Edge + mock IPC，三种分组各覆盖多小时/单小时曲线，校验实际 token 数据、零值和缺口 |
| 提示与布局 | 提示宽度不超过 482 px，不重复长说明；双币种费用摘要卡低于 115 px，费用面板 478 px；760 px 窗口展开覆盖说明和单价后页面不横向溢出 |
| npm run check | 最后布局调整后再次退出 0；Svelte 0 错误、0 警告 |
| npm run build:desktop | 退出 0；Windows x64 release EXE 和 NSIS 0.2.0 安装包生成，安装包约 3.59 MiB |

完整验证日志为 verify.log；最后布局调整后另由 browser-final.log 与桌面构建中的
Web 构建验证。Junie 副本恢复日志为 probe.log。截图已目视核对：today-token-groups.png、
token-tooltip-compact.png、cost-summary-compact.png、cost-panel-compact.png、cost-details-narrow.png。
桌面构建日志为 build-desktop.log，安装包位于
desktop/src-tauri/target/release/bundle/nsis/LLMUsage_0.2.0_x64-setup.exe。

浏览器回归不等于原生 GUI/IPC 验收；本轮不安装或启动新应用，不回写正式库。
更新后正常采集自动恢复误登记，无需清库；没有提交、推送或部署。
