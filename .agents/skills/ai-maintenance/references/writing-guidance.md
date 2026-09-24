# 写作指导：去除 AI 腔

Use when drafting or editing repository prose: replies, comments, documentation,
examples, agent instructions, commit messages, and PR or release notes.

按读者需要先说明事实、结果或动作，再解释必要原因。去除空话不等于猜测作者身份，
也不意味着用口语梗、营销比喻或省略技术条件来装自然。

## 内容与结构

- 写当前可验证的行为；未知、计划和已完成分别表达，不把“未验证”改成肯定句。
  旧行为、迁移过程和变更历史放对应记录；保留仍是合同的版本号、配置键和 API 路径。
- 一段说明一件事，用具体对象和动词。一个概念使用一个名称，避免为了变化而换同义词。
- 删除不增加信息的开场、章节预告、段末总结和重复结论。顺序重要时保留步骤编号；
  字段、选项或并行检查适合列表/表格，其余优先用连贯段落。
- 少用形容词称赞方案；写实际文件、条件、数量、时间或验证结果。没有测量就删掉性能赞语，
  不能为了替换“高效”而编造数字。
- 技术说明先交代变化和影响，再给依据。PR 说明面向未读聊天的审阅者，
  包含实际问题、最终行为及必要验证，不复述讨论过程或无关替代方案。

## 中文表达

下表是人工审阅提示，不是禁词表。先核对代码或上下文，再选择准确表述。

| 需要检查的表达 | 按实际含义改写 |
| --- | --- |
| 赋能、加持、助力、打造 | 谁提供什么功能、执行什么动作 |
| 抓手、底座、打通 | 具体工具、公共库、服务或接口，以及连接关系 |
| 沉淀、落地、闭环 | 记录结论、保存数据、实现、部署、修复并验证 |
| 对齐、拉齐、收敛 | 保持哪项一致、合并什么、统一什么状态或缩小什么范围 |
| 口径、维度、颗粒度 | 计算规则、时间来源、检查条件、指标或处理单位 |
| 链路、投影、水位 | 实际调用顺序、缓存/副本/派生视图、最大值或已处理序号 |
| 预算 | 若谈执行限制，写重试次数上限、剩余次数、超时时间或数量上限 |
| 兜底、护栏、门禁、钳制 | 写触发条件及实际动作：返回默认值、拒绝、重试、限制数值或阻止合入 |
| 全面、深度、系统性、全方位 | 写实际检查的文件、场景和方法，避免暗示未做的工作 |
| 强大、无缝、优雅、高效、稳健、轻松 | 写使用前提和已验证行为，删除无依据的评价 |

直接陈述事实，减少“值得注意的是”“需要指出的是”“显然”“毋庸置疑”。
删除无事实作用的“本质上”“从某种意义上说”和“对……进行……处理”。
“不是 A，而是 B”“不仅……更是……”仅在真实区别有助理解时使用；
不要凭空引入读者没有提出的反面选项。
“从 X 到 Y”只用于同类且确实覆盖的范围，不为凑节奏强行列三项。

## English wording

Prefer concrete verbs and familiar nouns. Review words such as delve, leverage,
seamless, robust, streamline, unlock, elevate, empower, cutting-edge, game-changer,
foster, facilitate, and utilize when they replace a specific action or evidence.
Keep the established technical meaning when it is the accurate term.

Remove staged openings such as “Let's dive in” and “In today's world”, unnecessary
Additionally/Moreover/Furthermore, vague attribution, stacked hedges, and unsupported praise.
Prefer is/are/has over inflated uses of serves as, boasts, and features.
Cut trailing highlighting/underscoring/ensuring clauses if they add no fact.
Keep one name per concept; avoid forced triads, false ranges, and decorative bold labels.
Use sentence-case headings. Dashes and real contrasts are useful only when the meaning needs them.

## 保留语义的例子

| 原句 | 改写 |
| --- | --- |
| 全面打通 Skill 加载链路，形成验证闭环。 | 从仓库根和子目录检查 Skill 发现结果，并记录失败项。 |
| 耗尽重试预算后触发兜底。 | 重试次数达到上限后返回错误。仅当代码确实如此时使用这句。 |
| 通过本节，你可以轻松了解项目的强大规则体系。 | 删除引导句，直接写规则与适用条件。 |
| This robust workflow seamlessly ensures documentation quality. | Run npm run lint:docs to check Markdown formatting. |

改写不得改变首次尝试是否计数、何时重置、范围是否含端点等行为。
重试、返回默认值、拒绝输入和截断数值是不同操作，不能一律改成“处理异常”。

## 例外与检查

保留原样：标识符、配置键、API/协议文本、错误信息、引文和专有名称。
内存对齐、张量维度、网络链路、链路追踪、算法收敛、闭环控制、
统计显著性、财务预算和 SRE 错误预算等术语按准确含义使用。
解释某个词时可以引用它。历史验证记录保留当时事实，不为统一文风改写历史结果。

回读时检查重复、长定语、被动句、读者不熟悉的缩写、无证据判断及机械开头结尾。
搜索命中只用于定位，不能仅凭词语判 lint 失败或声称文本由 AI 生成。
确认术语、事实、授权边界、条件和未验证项没有因缩短句子而丢失，再检查格式与链接。

## 参考与取舍

2026-09-24 读取兄弟仓库 AICodeReviewer、atsf4g-co 的
`.agents/skills/ai-agent-maintenance/references/writing-guidance.md` 后，
按本仓库任务改写；来源记录见 [来源索引](source-index.md#本地写作参考)。
采用具体用词、段落结构、语义例外和人工审阅方法；
不引入其他项目的站点禁词脚本、业务路径或中文词语识别 AI 作者的结论。
本文件包含执行所需内容，不要求客户端访问兄弟仓库。
