# OpenCode v2 本地投影

0.3.3 新增 `session_v2` 与 `session_message` 读取器，与 v1 的 `session/message/part`
读取器独立。Windows 2.0.32 本地记录已对照官方 v2.0.32 的
[结构定义](https://github.com/anomalyco/opencode/blob/v2.0.32/packages/schema/src/session-message.ts)、
[用量归一化](https://github.com/anomalyco/opencode/blob/v2.0.32/packages/core/src/session/usage.ts)、
[消息更新](https://github.com/anomalyco/opencode/blob/v2.0.32/packages/core/src/session/message-updater.ts)、
[投影与分支写入](https://github.com/anomalyco/opencode/blob/v2.0.32/packages/core/src/session/projector.ts)和
[压缩累计](https://github.com/anomalyco/opencode/blob/v2.0.32/packages/core/src/session/compaction.ts)核对。
[0.3.3 记录](../../validation/desktop-usage/release-033.md)区分原生样本、合成边界测试及 Provider/API 验收。

## 归属与生命周期

核验产品根目录、SQLite 结构及所属 `session_v2` 行。每条消息保留该会话版本；已安装版本或
库中最高版本不能标识其他行。不能由会话当前模型或累计值推断历史消息。
仅从 JSON 读取用量、模型、身份和时间元数据，正文不离开 SQLite。

| 投影 | 统计贡献 |
| --- | --- |
| 已完成 assistant（`data.time.completed`） | 一次已观测模型调用，包括未返回用量的失败调用。 |
| 运行中 assistant | 暂无结算贡献；之后按稳定 ID 重读。 |
| 已完成/失败 compaction | 用量观测。上游可能将多次尝试相加写入投影，不推造逐次调用数。 |
| user/system/synthetic/skill/shell/idle 及已知切换消息 | 不产生模型用量贡献。 |
| 未知消息类型或必需元数据异常 | 提示覆盖错误；保留其他有效消息。 |

分支投影使用新 ID 和原有序号复制已结算前缀。通过 `fork_session_id` 及
`fork_boundary.messageID` 找到原始序号，排除边界之前的记录；`type=after` 同时排除边界本身。
缺少边界证据属于归属错误，不能把继承历史当作新调用。

## 字段

| 原生字段 | 映射与限制 |
| --- | --- |
| `tokens.input` | 未缓存输入。 |
| `tokens.cache.read/write` | 独立缓存桶。 |
| `tokens.output` / `tokens.reasoning` | 正文与推理输出互斥。正文输出为正证明上游减法未被截断，两者相加可恢复含推理的输出总量；正文为零时输出总量保持未知，独立保留正推理量。 |
| 零 token 分量 | 未知：上游将缺失/无效值归一为默认零，没有有效性标记证实报告零。 |
| 完整总量 | 三个输入桶均已知才派生输入总量；已知输入与恢复的含推理输出相加得到完整总量。仅缺推理拆分不使已恢复输出失效；不得将部分和作为完整总量。 |
| `model.id/providerID` | 消息自身请求身份，压缩记录可能缺失。不从模型系列或协议推断。 |
| `cost` | 正值为客户端 USD 估算；默认零未知，不是账单或免费使用证明。 |
| `time.completed/created` | 原生完成时间，否则使用压缩开始时间。正时间差提供耗时；不推断 TTFT。 |

拒绝负数、非整数及溢出的 token 结构。已确认完成的调用遇到无效用量时仍计一次调用，
token 保持未知并提示诊断。不把原生会话累计或持久事件表叠加到消息贡献。
同库两套非空 legacy/v2 布局属于尚未核验的迁移重叠，不自动合并。

## 缺失明细与可恢复输出

开源允许检查映射实现，不保证本地投影保留原始响应。v2.0.32 已核验的写入链路如下：

1. [OpenAI Chat 映射](https://github.com/anomalyco/opencode/blob/v2.0.32/packages/ai/src/protocols/openai-chat.ts#L913-L935)
   在 `Usage.providerMetadata` 中保留总量和原始 usage。结束时传递 `usage`，
   没有把其中嵌套的元数据复制到事件的 `providerMetadata`。
2. [Lifecycle.finish](https://github.com/anomalyco/opencode/blob/v2.0.32/packages/ai/src/protocols/utils/lifecycle.ts#L98-L116)
   与 [LLMEvent.stepFinish](https://github.com/anomalyco/opencode/blob/v2.0.32/packages/ai/src/schema/events.ts#L370-L374)
   继续分别传递 usage 和事件元数据。
3. [publish-llm-event](https://github.com/anomalyco/opencode/blob/v2.0.32/packages/core/src/session/runner/publish-llm-event.ts#L578-L585)
   保存五个归一化分量，仅从事件元数据提取 provider state；原始输入总量、完整总量及嵌套的
   原始 usage 没有复制到此投影。`SessionUsage.tokens` 将缺失或无效分量改为零。

只读 Windows 样本的七条 assistant 消息均没有 `providerState` 或 metadata，`event` 表为空。
七条缓存写入均为零，另有一条缓存读取和一条推理为零。这些事实解释输入及明细覆盖不完整，
不代表调用未识别或数据库损坏。供应商／模型名称和当前配置不能证明每次历史请求的协议及
原始字段是否存在。不能套用其他协议的默认值或 OpenCode 显示的分项和制造完整输入。
恢复这些明细需要来源保存总量和字段存在性证据，重扫相同记录无法补回。

实际的 [Usage getter](https://github.com/anomalyco/opencode/blob/v2.0.32/packages/ai/src/schema/events.ts#L77-L85)
计算 `max(0, (outputTokens ?? 0) - (reasoningTokens ?? 0))`。对于有效的非负用量，
正文输出为正时可以逆转该减法，包括未提供推理拆分的情况。正文为零无法排除总量缺失或截断，
因此仅有正推理量不代表完整输出。此规则不声称缺失推理是来源报告的零。
七条原生记录的正文输出均为正，可恢复 **7,888 输出 token**；输入和整体用量仍为下界。

## 重扫与健康

使用稳定的 `opencode:v2:<message-id>` 身份及 `time_updated` 源修订。
按 ID 有界分页读取投影，每次最多 50,000 行或更低的配置预算。最后一页后开始下一轮，
重新访问前面的可变 ID，不假设时间戳唯一。大数据源需要多次扫描；处理位置之前的修改
在下一轮读取。游标与统计贡献同事务提交。未完成整轮时保留既有错误，所有行均重访后
才替换错误状态；成功的空页不能清除错误。逐行校验 SQLite 列类型，ID、所有者、消息类型、
序号或修订时间的类型错误产生行级诊断，不中止其他有效消息，也不让分页停住。非空坏 ID
使用保留类型的稳定游标越过；NULL ID 排除并显示诊断。之后的完整有效扫描才可替换这些错误。
非有限 ID 先产生诊断，随后跳过。保存的整轮完成状态在有界页面仅含这种 ID 时仍保留排除状态，
不会反复重置首页；新的完整轮次会再次检查它。此状态写入前的旧上下文保持原来的首轮行为。

适配器解析器升级通过正常发现路径重评原来未识别的登记，包括手工父目录根。
v1 及其他产品的读取器继续独立。有效兼容记录保留内部版本依据，不显示来源警告。
真实坏记录、归属未知及不支持布局仍会提示，无须清空数据库。

解析器 `opencode-session-messages-2` 及 dispatch 4 会重读已处理位置。同修订输出修正需
重建 parser-1 的完整事件摘要，只允许已核验的输出／总量覆盖及解析依据变化。
保留首次观测时间、全部诊断历史及真实冲突，包括同批次较早产生的冲突。
修正、汇总重算和处理位置在失败时一同回滚。
