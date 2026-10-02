# 2026-10-02 看板修正验证

状态：已完成本轮实现、完整工程检查、浏览器回归与本机数据副本验证。
环境：Windows x64，PowerShell 7.6.6，Node.js 24.21.0，Cargo 1.98.1，Python 3.14.7；
默认工作目录为仓库根，保留用户已有修改，没有提交、推送或改动依赖范围。
受限终端 CreateProcessAsUserW failed: 5，按 harness 权限重试后只读检查成功。

## 已确认根因

- 总量图在总量未知时额外绘制输入/输出，饼图只筛总量导致 Copilot 名称消失。
- 一键配置输出位于 telemetry/，不在自动扫描根；真实 span 名为 chat 加模型名，
  现有 otel 解析器只接受裸 chat。真实文件还混有 logs/metrics，必须结构化筛选。
- 本机原生数据库今日有 Copilot 用量 observation 和调用 round，但 Claude 模型的
  4-8 与 4.8 拼写分组分离，token 行无调用，调用行无 token。
- 价格回退前先返回 no_provider/channel_unknown，用户开启费用仍几乎全部未计价。
  本机 Kimi profile 为 kimi-code/k3-256k，官方模型表确认对应 K3；GLM 官方种子渠道
  使用 zai 标签，不能只接受 api 标签。另补已核验的 Opus 4.8 与 dated GPT-4o mini。
- 趋势默认费用跨 3 列后接全宽热图，周分布跨 2 列和两个跨 3 列饼图产生空列。

## 本机只读证据

工具只返回模型、统计值和字段名，没有返回提示词、回复、工具内容或真实身份。
已配置 Copilot file：1196 条混合记录，其中 30 个 CLIENT chat span；
input 937508、output 9480、cache_read 786070、cache_creation 149192、reasoning 4702。
13 个 chat span 的会话身份直接匹配原生记录；没有共同 turn/call ID，不按时间去重。
上述数值是当前文件快照，并非完整历史或当前日最终量。

## 验证结果

### 工程与界面

| 命令 | 目录 | 退出码 | 实际结果 |
| --- | --- | --- | --- |
| npm run verify | 根目录 | 0 | Markdown 166 文件、资产 82 文件、脚本 3 项、UI 13 项；svelte-check 0 错误/警告；fmt、Clippy、Rust 787 项通过（2 项显式本机审计忽略）；前端构建成功 |
| cargo test --locked -p llm-usage-core --test dashboard_repair --test pricing_v29 | desktop/src-tauri | 0 | 本轮 11 项边界回归与 V29 10 项全部通过，已包含完整 Rust 数量 |
| npm run test:ui / npm run check / npm run build:web | 根目录 | 0 | 最后提示文案修正后复核，UI 13 项通过、类型检查无错误/警告、前端构建成功 |
| npm run test:browser | 根目录 | 0 | Edge headless：总量单序列、Agent 名称、单周期悬浮、部分占比、明细未知调用、总览/趋势 API 参考、八指标密集布局、遥测批量重试/撤销、10 语言、主题、窄屏、筛选与分页 |

浏览器截图在忽略目录 build/browser-smoke/；宽屏明暗主题及窄屏交互通过，
已人工查看趋势浅色截图。浏览器命令使用模拟 bridge，不冒充 Tauri GUI 实测。
最后的 Markdown、相对链接与 git diff --check 结果见下方收尾检查。

### 本机副本端到端

先用 Python SQLite backup API 从真实应用库的 mode=ro 连接建立独立副本，
不是复制裸 DB；旧副本保留在 build/dashboard-repair/。
执行 `cargo run --locked -p llm-usage-core --example real_verify_dashboard`
（目录 desktop/src-tauri，退出码 0），该工具强制目标位于仓库根 build/，
读取原始 Agent 文件但只写副本，输出限制为统计与未计价原因。

| 阶段 | 调用 | 用量 observation | 已知输入 | 已知输出 | 已知总量 | 排除记录 |
| --- | --- | --- | --- | --- | --- | --- |
| 原生副本初始 | 235 | 13 | 3,832,423 | 487,818 | 未知 | 0 |
| OTel 补充及载体选择后 | 252 | 11 | 4,403,216 | 334,829 | 946,988 | 15 |
| 原生文件再次扫描后 | 252 | 11 | 4,403,216 | 334,829 | 946,988 | 15 |

30 个合法 OTel 调用新增成功，第二次 OTel 扫描新增 0、错误/冲突 0。
15 条原生记录只排除贡献，原始记录和修订历史仍保存；原生旧游标重放使修订
41→42 单调增加，统计不变。总量栏是可观测部分，不把原生输入下界与整轮输出相加。

价格规则修复一次回填：2,577 条事件匹配 USD 官方参考，合计 12,062 美分，
fallback_event_count=2,577；777 条发生时估算仍未计价。该金额是各可计价分量的
API 参考，不是订阅账单，已计价事件也不等于调用。
2026-09-01 至 2026-10-02 的当前价格模拟未计价原因为
channel_unknown=527、no_price_row=226；发生时金额与当前模拟分开。
codex-auto-review 没有已核验的公开模型映射，保留未知，禁止猜测套 GPT 价。
未知 token、缺少历史生效行也不补零或倒填价格。

### 兼容与限制

合成回归验证已消费且字节不变的游标升级、trace+span 旧身份迁移、不同 trace
同 span ID、同源重复导出、先后采集顺序、跨主机/用户隔离、错误记录不夺取来源、
封存分区、原生重扫、模型拼写合并、精确 Kimi profile 价格优先、官方供应商边界、
渠道/币种歧义和后台价格更新不改写发生时估算。
前期旧定价断言按本轮明确授权的新参考合同更新，保留缺证与歧义断言。

本轮没有再次修改用户 IDE 配置，没有发起模型调用或写生产应用库。
Copilot 导出仅覆盖实际导出记录，开启当日可能缺少此前调用；更新客户端后采集刷新
才能应用本轮扫描规则，无需清库。CLI/JetBrains 新版遥测、trace SQLite、
其他 Agent 的补充输出自动叠加、macOS/Linux 桌面与原生 GUI 打包安装未验收。

### 收尾检查

`npm run lint:md` 退出码 0，166 文件无问题；
`python build/dashboard-repair/check-links.py` 退出码 0，113 个相对文件链接无缺失；
`git diff --check` 与 `git status --short -- build` 检查退出码 0，
没有空白错误或临时产物进入变更列表。最终回读包含未跟踪的新源码、测试、语言资源、
价格 JSON 和设计/验证文档；依赖版本与用户此前修改均保留。
