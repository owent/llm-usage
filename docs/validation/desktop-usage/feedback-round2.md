# 第二轮反馈：费用匹配、来源路由与控件

2026-10-03。复核提交 ab647fd、de8727f、当前实现与本机只读数据后修复。
规则见 [看板修正](../../design/desktop-usage/dashboard-repair.md)、
[价格](../../design/desktop-usage/pricing.md)与 [Copilot 遥测](../../design/desktop-usage/copilot-otel.md)。

## 本机证据

通过 SQLite 只读连接的一致备份验证，不写正式库或 Agent 原始文件。
仅输出模型、计数字段、token 桶、状态和错误类别，不输出对话、工具内容、凭据或项目路径。
临时脚本、库和日志均在忽略目录 build/feedback-round2/。

Asia/Shanghai 的 2026-10-03 快照恰有 199 条未计价，构成为：

| 型号 | 条数 | 核验结果 |
| --- | ---: | --- |
| copilot-nes-lysithea-24 | 178 | 本机 OTel 补全模型 ID，provider 缺失，现有已核验价目没有对应行；不能按 Claude/GPT 猜实际型号 |
| codex-auto-review | 18 | 来源只报告路由 ID，没有真实底层型号；不能套 Codex 主模型价格 |
| hy4-preview-f | 2 | CodeBuddy 官方扩展本地模型目录明确 id→Hy4 preview；缺参考别名和价目，已修复 |
| kimi-for-coding | 1 | Kimi 官方确认 K2.8 Preview，原厂公开按量价尚未核到；已识别型号，保留缺价 |

因此不是 199 条都由连字符匹配失败造成。修复后同一快照今日未计价 **197**；
仍保留明细的全部日期则为 770：582 条自动审查、178 条 NES、9 条 Kimi、1 条无 token 的 Opus。
数量属于这次快照，后续真实使用会变化。

HY4 官方名称是 **Hy4 preview**。[腾讯价目](https://cloud.tencent.com/document/product/1823/130055)
核验广州标准 API 每百万 token：CNY 输入 6、输出 18、缓存命中 0.3。
新快照生效日为 2026-10-03，不虚构历史价格。CodeBuddy 临时免费入口不代表 API 免费。

[Kimi 官方模型表](https://www.kimi.com/code/docs/en/kimi-code/models.html)与
[2026-09-11 发布说明](https://www.kimi.com/code/docs/en/kimi-code/whats-new.html)
确认 kimi-for-coding 已原地升级到 K2.8 Preview。
[Moonshot 公开价目](https://platform.kimi.ai/docs/pricing/chat)未核到该型号的按量单价；
腾讯云 TokenHub 虽公布 K2.8 渠道价，但不是 Moonshot 原厂价。
本轮维持既有原厂参考规则，不套 K3/K2.7，不把其他渠道费率冒用为原厂价。

Opus 4.8 在全部保留明细范围内的 **5 条部分估算、1 条未计价**，名称均能匹配：

- 1 条原生 VS Code observation：输入 67,162、输出 14,715，输入为末次调用下界，
  缓存拆分和完整总量未知，只能对已知可计价分量估算。
- 4 条 Visual Studio 调用：输入、缓存读取和输出已知，缓存写入/未缓存输入未知。
- 1 条原生 round 调用标记：全部 token 字段为空，计调用但没有可计价用量。

## 修复

1. 模型匹配分三层：保留来源原名；统一空格/下划线/连字符的比较键；按证据与日期
   解析 API 参考别名。Claude 小数拼写单独处理，其他型号的小数版本和未知后缀不删除。
   精确 ID 优先，多个目录拼写冲突拒绝计价；档位选择在型号确定后进行。
   SQL 筛选、用量分组、费用缓存和界面查找共用相同规则，动态别名缓存包含日期解析结果。
   费用规则升级为 official-reference-4，仅修复仍有明细的未封存日期一次；价格刷新不改历史。
2. 按模型显示未计价原因和实际参考型号。ECharts 缺口可能以字符串 `-` 传给 tooltip，
   原来金额格式化产生 NaN；现在只格式化有限数值，缺口显示破折号，已知零金额显示零。
3. Copilot 缺总量但输入输出已知时，表格和图表提示展示两者和。原生 turn 只能保证
   已观测下界，显示 `≥` 与说明；不写回完整总 token，不改变完整总量图的缺口和导出语义。
   完整逐次载体仍由既有解析器派生精确总量；大整数求和不丢精度。
4. 来源报错的共同根因：scanner 把应用管理的 Copilot 遥测根广播给所有适配器。
   pi 先把 events.jsonl 登记为未知格式，其他适配器随后触发 source_files.file_id 唯一键冲突。
   oh-my-pi 与原生 VS Code 的真实目录均正常，报错来自另一个虚假来源。
   现改为管理根定向路由，物理文件只保留一个有效归属；空白误登记自动退出展示，
   无游标/用量历史的错误文件登记可恢复给正确适配器。停用设置、诊断、修订、周期历史保留。
   真正损坏的手工文件仍显示诊断，不存在的历史来源变灰，权限错误不伪装为未安装。
5. otel 读取本机导出文件，vscode-copilot-chat 读取 chatSessions，载体不同。
   同主机/用户/会话/本地日按既有规则选择贡献，file/HTTP 副本按 trace+span 身份去重；
   不按相近时间或相同 token 猜身份。OTel 首行 metrics 也能识别载体，但不把 metrics 计为调用。
6. 设置开关统一尺寸、焦点、键盘与可点击标签，多选继续采用复选框语义；
   保留系统高对比度和减少动画偏好。

## 验证与范围

环境：Windows x64，Node.js 24.21.0；npm 依赖版本以当前锁文件为准。
测试保留发生过的失败日志；Vite 在沙箱内 spawn EPERM，获准在沙箱外运行既有 npm 检查。

| 验证 | 结果与范围 |
| --- | --- |
| npm run verify | 最终退出 0：Markdown、资源、3 项脚本测试、19 项前端单测、Svelte、fmt、clippy、819 项 Rust 测试、Web 构建通过 |
| npm run test:browser | 最终退出 0：真实 Edge + mock IPC；缺失哨兵/零金额、按模型原因/参考型号、Copilot 下界、来源变灰、标签/键盘开关、明暗主题及既有十语言回归通过 |
| cargo test 的 model_reference/source_routing/pricing_v29 | 退出 0；别名日期、歧义、档位优先、全部适配器路由、旧登记恢复、周期历史保留、快照幂等通过 |
| 真实库副本两轮采集 | pi、oh-my-pi、原生 VS Code、OTel 各 1 个正常来源，全部无读取错误；第二轮四者新增均 0 |
| 历史对照 | 8 个无用量的误登记退出展示；既有事件删除 0，修订倒退 0；四个来源不再有未识别文件 |
| npm run build:desktop | 退出 0：Windows x64 release EXE 和 NSIS 0.1.2-dev 安装包生成，安装包约 3.59 MiB |

最终日志为 build/feedback-round2/verify-final.log、browser-final.log、build-desktop.log、
probe.log、replay-comparison.json。开关明暗主题截图在 build/browser-smoke/，已目视核对。
安装包位于 desktop/src-tauri/target/release/bundle/nsis/LLMUsage_0.1.2-dev_x64-setup.exe，
独立程序位于 desktop/src-tauri/target/release/LLMUsage.exe。

第一轮 oh-my-pi 相对备份新增 2 条，第二轮零新增；其他三个来源两轮均零新增。
正式库未回写、不需要清库，更新后的正常采集自动修复。浏览器通过不等于原生 GUI/IPC 验收，
本轮没有启动或替换正在运行的应用，也未执行安装、提交、推送或部署。
