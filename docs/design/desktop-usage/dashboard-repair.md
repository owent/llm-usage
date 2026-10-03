# 总量图表、Copilot 补充采集与官方价格参考修正

2026-10-02，用户已授权修复。先核验当前源码、锁文件及本机白名单字段；
验证记录见 [本轮记录](../../validation/desktop-usage/dashboard-repair.md)。

## 行为与兼容

2026-10-03 第二轮反馈合同：

- 应用管理的遥测根只传给 OTel 适配器；共享手工根尊重已确认的文件归属，
  同一物理文件已有有效归属时不被其他适配器再次采集。旧错误注册自动重评，
  不删除用量、修订或诊断；该目录中无用量历史的误登记不在数据源页冒充已安装 Agent。
  用户手工文件的真实未知格式仍保留诊断；不存在的历史来源变灰，权限错误仍报告读取失败。
- 模型名分为原始名称、拼写比较键和有证据的 API 参考型号。空格/下划线/连字符
  可统一比较，版本小数与未知后缀不任意删除；动态 profile 映射保留生效时间依据。
  原始来源、渠道和历史价格保持独立，按模型显示缺价/无用量原因。
- 费用曲线缺失点保持缺口，已知零金额显示零；ECharts 缺失哨兵不能参与金额格式化。
- Copilot 同范围输入输出齐全时求和；原生 turn 的末次输入与整轮输出只在展示层
  显示已观测总量下界并解释来源，不能改写为完整总 token 或参与完整总量统计。
- 设置中的开关统一视觉、焦点和键盘行为；多选项保留复选框语义和可点击标签。

2026-10-03 审查补充：费用参考卡放入今日/趋势的指标网格，短标题为“API 参考费用”，
宽容器八列、中等容器四列、窄容器两列/一列；账户额度仍用独立栏。
费用缺口区分“来源未提供可计价 token”和“没有适用价目”，部分估算说明输入拆分、
输出或价格分项缺失。未知 token 不补零，已知零用量可按适用价格记零金额。

按用户要求增加 CNY→USD 的展示折算：保留人民币原金额/原单价，旁列约合美元，
不合并不同币种，不改写历史估算。采用版本化 ECB 2026-10-02 快照（EUR/USD=1.1225、
EUR/CNY=7.5259，CNY→USD=1.1225/7.5259），显示日期和来源；该快照不自动联网更新，
未知币种或不安全数值不折算。金额和单价分别在各自整数单位上换算、四舍五入。
来源：[ECB 官方参考汇率](https://www.ecb.europa.eu/stats/policy_and_exchange_rates/euro_reference_exchange_rates/html/index.en.html)。

Codex 文件自动重评保留逐次用量、修订和对账诊断。累计快照差异保留 mismatch，
不单独降级读取健康；新版独立逐次记录不依赖累计快照，旧版 total/last 字段异常仍降级。
官方协议允许 TokenCountEvent.info 缺失/null：没有用量，不计调用、不补零。
坏行、逐次用量形状或时间异常在分批读取后保留错误状态，避免后续正常批次覆盖异常。
Kilo 新版本登记仍须逐版本脱敏证据，空会话没有 assistant 用量不能据此认证，
不按版本范围自动认证。解析器或支持版本证据更新后自动重扫旧游标。
数据源的兼容读取提示说明已经自动检查可识别用量，支持更新后自动复核；
版本证据不足与真实读取错误分别保留，不能仅为了移除提示认证新版本。
Kilo 按逐消息的 session.version 标记依据，库内最高版本不替代其他会话的证据；
混合库的兼容状态跨增量轮次保留，规则升级自动重放后重新判断。

- 总量视图每组只绘制总 token，未知保持缺口，输入输出留在对应子图与提示中。
  总量占比支持切换总量、输入、输出；缺少总量的 Agent 明示未知，不丢掉名称。
- 调用次数图按 Agent 展示 Agent 名称和调用次数，单周期也保留可悬浮的数据点。
  原生 Copilot 的用量 observation 不当成调用；模型名的已核验破折号/小数拼写统一分组。
- 一键配置的本机 Copilot file 输出自动发现。只接入已核验的 CLIENT chat span，
  支持 `chat <model>`，跳过 metrics、logs、invoke_agent 与其他非调用记录。
  上游未报告 token 的调用仍计调用；缓存桶不补零，推理为输出子集。
  总览小字提示 Copilot VS Code/Agent Host、最新版 CLI、JetBrains 需导出才能补齐
  逐调用观测；CodeBuddy CLI 已有本机会话载体，导出仅为可选补充，不误称必须。
- 没有共同调用 ID 时采用载体选择：同原始主机、用户、会话和统计本地日，
  已观测 Copilot OTel 替代原生会话贡献；其他会话/日期保留原生数据。
  这不是按时间/token 相同去重；配置开启当日可能只覆盖开启后的导出调用，明确提示。
  原生记录及修订历史保留，后续原生重扫不能重新叠加。没有会话身份不替代原生记录。
  file/HTTP 同源副本以 trace+span 联合身份和原主机/用户去重；不同 trace 的相同 span ID
  仍是不同调用。旧版 span-only 游标重放保留旧记录并撤回旧贡献，不需清库。
- 费用默认关闭。精确 provider、模型、用户配置渠道价格优先；没有精确项时，
  即使 provider/渠道缺失，也可使用同模型已核验官方供应商按量价作为参考。
  无渠道偏好时选官方 global 参考（优先 api 标签，也接受官方提供商名称的渠道标签）；
  没有该项再选有确定币种的官方渠道，
  候选渠道/币种仍有歧义则未计价。保留真实 provider、币种和 fallback_event_count。
  不给同系列的不同型号套价，订阅专属模型无按量价时仍未知。
  官方模型表确认 Kimi Code 的 k3/k3-256k 对应 K3，只用于 kimi-k3 价格参考；
  别名对裸 model_raw（`k3`/`k3-256k`，Kilo Code / oh-my-pi 在自定义 Kimi provider
  下的真实上报形态）与 provider 前缀形态都生效，并归属官方 moonshot；
  统计保留原始 profile，精确 profile 渠道价格优先。
  2026-10-03 补充：按 Kimi 官方发布说明，2026-09-11 起的 kimi-for-coding 对应
  kimi-k2.8-preview，早期记录不套用新指向。当前未核到原厂公开按量价，保留缺价；
  腾讯云等渠道的价目不伪装为 Moonshot 官方价格。hy4-preview-f 经本机官方
  CodeBuddy 模型目录确认是 Hy4 preview，按腾讯官方 CNY 价参考。
  别名与拼写规则集中维护；精确目录 ID 优先于拼写归一与上下文档位，冲突拒绝套价。
- 总览和趋势明确显示 API 按量价格参考、部分覆盖、官方回退和未计价原因；
  已知用量 observation 同样支持部分估算，计价事件数不当成调用数。
  后台价格更新不改写发生时估算。规则修复只重算仍保留明细且未封存的日期一次。
- 趋势默认调用/用量并排，费用全宽，周分布与两个占比均分一行；允许用户调整，
  不清空已有布局。网格填充空列，窄屏改为单列。
  七项范围指标与账户额度组成八个卡片，宽屏两行四列，窄屏两列/一列，不留额度旁空档。

## 官方供应商证据

以下页面正文于 2026-10-02 核验；模型系列仅用于限定候选官方供应商，
具体型号仍须在价格快照中有对应行，不根据系列虚构费率。

| 系列 | 官方供应商/目录 ID | 来源 |
| --- | --- | --- |
| GPT、OpenAI o 系列 | OpenAI / openai | [OpenAI 定价](https://developers.openai.com/api/docs/pricing) |
| Claude | Anthropic / anthropic | [Claude 定价](https://platform.claude.com/docs/en/about-claude/pricing) |
| Gemini | Google / google | [Gemini 定价](https://ai.google.dev/gemini-api/docs/pricing) |
| Kimi、Moonshot | 月之暗面 Moonshot AI / moonshot、moonshotai（含 CN） | [Kimi 定价](https://platform.kimi.ai/docs/pricing/chat) |
| GLM | 智谱 / zhipuai；国际 Z.ai / zai | [Z.ai 定价](https://docs.z.ai/guides/overview/pricing) |
| DeepSeek | DeepSeek / deepseek | [官方定价](https://api-docs.deepseek.com/quick_start/pricing)（公开正文 HTTP 200，23982 字节，保存在忽略的 build/dashboard-repair/） |

Kimi profile 对应关系来自 [Kimi Code 模型表](https://www.kimi.com/code/docs/en/kimi-code/models.html)。
日期依据 [Kimi Code 发布说明](https://www.kimi.com/code/docs/en/kimi-code/whats-new.html)。
HY4 采用 [腾讯官方价目](https://cloud.tencent.com/document/product/1823/130055)，
广州标准 API 每百万 token CNY：输入 6、输出 18、缓存命中 0.3；
[2026-10-03 补充快照](../../../desktop/src-tauri/crates/core/prices/seed-2026-10-03.json)
从核验日起生效，不把 CodeBuddy 免费入口解释为免费 API。
本轮本机明细与验证见 [第二轮反馈记录](../../validation/desktop-usage/feedback-round2.md)。
新增 [2026-10-02 补充快照](../../../desktop/src-tauri/crates/core/prices/seed-2026-10-02.json)
包含 Claude Opus 4.8 和 GPT-4o mini 的 2024-07-18 官方快照型号。
标准 USD/百万 token：Opus 输入 5、输出 25、缓存命中 0.50，5 分钟/1 小时写入 6.25/10；
GPT-4o mini 输入 0.15、缓存命中 0.075、输出 0.60。
生效起点为本轮核验日，不虚构历史生效时间，也不为不存在的缓存价格补零。

Copilot file 形状依据已存档的固定源码
[fileExporters](https://github.com/microsoft/vscode/blob/dc546cc3c9979a19adafccd439889d7b64298def/extensions/copilot/src/platform/otel/node/fileExporters.ts)
及本机 30 个 CLIENT chat span；CLI 单次/汇总差异依据
[官方 OTel 参考](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-command-reference#opentelemetry-monitoring)。
不把这次 VS Code 真实验收扩大为 CLI/JetBrains 新版本真实验收。
input 缓存包含关系与 reasoning 输出子集按 [OTel GenAI 语义](https://opentelemetry.io/docs/specs/semconv/registry/attributes/gen-ai/)
核对；SDK 数值 CLIENT=2 与 OTLP CLIENT=3 不同，接收器转换为文本枚举避免混用。

## 失败、验证与回滚

导出缺失/损坏时显示已有可观测范围，不发起模型调用补样本。
本机原始文件只读，临时验证库和脱敏输出位于根 build/dashboard-repair/。
回归覆盖旧游标、重扫、先后扫描次序、跨主机/用户隔离、当日范围、图表悬浮、
渠道未知的官方参考、型号缺失及多币种。验证命令和结果记录在本轮验证文档。
回滚只撤回本轮代码；原始来源和价格快照不删，保留原生记录可重新选择来源。
