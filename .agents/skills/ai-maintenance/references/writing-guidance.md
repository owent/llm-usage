# 写作指导：去除 AI 腔

Use when drafting or editing repository prose: replies, comments, documentation,
examples, agent instructions, commit messages, and PR or release notes.

按读者需要先说明事实、结果或动作，再解释必要原因。去除空话不等于猜测作者身份，
也不意味着用口语梗、营销比喻或省略技术条件来装自然。

## 内容与结构

- 写当前可验证的行为；未知、计划和已完成分别表达，不把“未验证”改成肯定句。
  文档只保留最新规则和状态，直接合并修改，不追加历史修订日期或逐次更新说明。
  变更过程由版本控制或备份保存；仍需兼容的旧行为、版本号、配置键和 API 路径继续说明。
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
| 夹具 | 指测试直接读取的静态数据文件时写测试数据；泛指 xUnit 固定测试前提时保留英文 fixture |
| 秘密、机密 | 按实际对象写密钥、凭据、口令或敏感信息，不把 secret 直译为“秘密” |
| 预算 | 若谈执行限制，写重试次数上限、剩余次数、超时时间或数量上限 |
| 合同（用于开发文档或计划） | 按所指内容写设计规范、接口约定、字段规则、平台协议或验收标准；法律合同、引文和代码标识符保留原词 |
| 缺证、待证、未证 | 写明缺少什么或尚未完成什么：缺少真实用量样本、本地格式尚未核验、版本兼容性待验证；不把“未验证”写成“不支持” |
| 证据（用于研发说明） | 按对象写源码依据、文档依据、样本、日志、测试结果、测量结果或验证记录；说明它能支持哪项结论，避免只写“有证据”或“缺少证据” |
| 门槛 | 按含义写前置条件、采用条件、版本要求、验收标准、判定阈值或限制；有明确数值时直接写比较条件，保留必须满足还是仅供参考的区别 |
| 兜底、护栏、门禁、钳制 | 写触发条件及实际动作：返回默认值、拒绝、重试、限制数值或阻止合入 |
| 全面、深度、系统性、全方位 | 写实际检查的文件、场景和方法，避免暗示未做的工作 |
| 强大、无缝、优雅、高效、稳健、轻松 | 写使用前提和已验证行为，删除无依据的评价 |

直接陈述事实，减少“值得注意的是”“需要指出的是”“显然”“毋庸置疑”。
删除无事实作用的“本质上”“从某种意义上说”和“对……进行……处理”。
少用“不是 A，而是 B”“并非 A，而是 B”“不在于 A，而在于 B”等先否定、再肯定的句式。
否定项只为烘托重点时，直接写结论；纠正具体误解、区分易混概念或说明排除范围时可以保留。
不要凭空树立反面观点，也不要把可以同时成立的两项写成互相排斥；需要突出重点时写清依据和优先次序。
按常见汉语复句分类，“不是……而是……”属于并列关系中的对举，否定前项、肯定后项。
“不仅……而且……”“不只是……更……”表达递进，前项仍然成立；简化时保留两项事实。
“但”“然而”“不过”按真实的转折或让步关系使用，不因这条规则减少正常转折。
检查句式是否增加信息，不设词频、比例或固定次数限制；保留必要的否定、条件和技术限制。
“从 X 到 Y”只用于同类且确实覆盖的范围，不为凑节奏强行列三项。

描述验证情况时，尽量写清检查对象、方法、结果和适用范围。
“尚未验证”表示检查未完成；“缺少真实样本”表示缺少特定输入；
“验证未通过”表示已经检查且结果不符合要求，三者不能互换。
源码或文档可作为实现依据，合成测试可验证解析逻辑，真实样本可核对本地格式，
这些结果不能相互替代，也不能扩大到未检查的版本或环境。
条件类表述需保留约束强度、适用版本、数值及比较方向，不统一改成含糊的“要求”。

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
Avoid unnecessary “not X but Y” reframing. State Y directly when rejecting X adds no information.
Keep genuine corrections, distinctions, and exclusions; do not frame compatible claims as mutually exclusive.
Preserve both claims when simplifying “not only X but also Y”. Use but, however, and yet according to meaning.
Review the information a sentence adds, without imposing a word ban or frequency quota.

## 保留语义的例子

| 原句 | 改写 |
| --- | --- |
| 全面打通 Skill 加载链路，形成验证闭环。 | 从仓库根和子目录检查 Skill 发现结果，并记录失败项。 |
| 耗尽重试预算后触发兜底。 | 重试次数达到上限后返回错误。仅当代码确实如此时使用这句。 |
| 缺证 IDE 后移 F1。 | 本地用量格式尚未核验的 IDE 列入 F1 后续计划。 |
| 取得证据后更新验收状态。 | 完成对应检查并记录结果后更新验收状态。 |
| 源码证据不能代替桌面证据。 | 源码核验不能代替原生桌面验收。 |
| MCP 采用门槛。 | MCP 采用条件。 |
| 三次试验不是产品硬门槛。 | 三次试验仅为建议次数，产品未规定必须执行三次。 |
| 本节不是罗列功能，而是介绍配置步骤。 | 若无需澄清章节范围，直接写“本节介绍配置步骤”。 |
| 本工具不仅能扫描日志，更能生成报表。 | 若无需强调递进，写“本工具能扫描日志并生成报表”，保留两项能力。 |
| 通过本节，你可以轻松了解项目的强大规则体系。 | 删除引导句，直接写规则与适用条件。 |
| This robust workflow seamlessly ensures documentation quality. | Run npm run lint:docs to check Markdown formatting. |

改写不得改变首次尝试是否计数、何时重置、范围是否含端点等行为。
重试、返回默认值、拒绝输入和截断数值是不同操作，不能一律改成“处理异常”。

## 例外与检查

保留原样：标识符、配置键、API/协议文本、错误信息、引文和专有名称。
内存对齐、张量维度、网络链路、链路追踪、算法收敛、闭环控制、
统计显著性、财务预算和 SRE 错误预算等术语按准确含义使用。
“证据”在法律、取证或专门讨论证明材料的语境中可保留；“门槛”用于实际进入难度时也可保留。
解释某个词时可以引用它。历史验证记录可以调整说明用词，保留当时的日期、版本、
命令、结果和未执行项；原始输入、引文及程序实际输出保留原样。
来源核验时间、样本覆盖时间、价格生效日期和模型别名适用日期有技术用途，按需保留，
不要把这些日期当作文档修订历史删除。注释按同一规范改写，保留标识符与可执行内容。
真实纠正可以保留，例如在澄清计数单位时写“这里统计的不是请求次数，而是消息条数”。
正常转折也可以保留，例如“调用成功，但日志未完整写入。”它说明调用结果与日志状态的差异。

回读时检查重复、长定语、被动句、读者不熟悉的缩写、无依据判断及机械开头结尾。
检查先否定、再肯定的句子：否定项是否增加信息，两项是否确实排斥，有没有为强调而制造错误观点。
改为直接陈述后，核对必要的排除范围和前项事实是否仍然清楚；重复使用同一种句式时调整组织方式。
搜索命中只用于定位，不能仅凭词语判 lint 失败或声称文本由 AI 生成。
确认术语、事实、授权边界、条件和未验证项没有因缩短句子而丢失，再检查格式与链接。

## 参考与取舍

参考兄弟仓库 AICodeReviewer、atsf4g-co 的
`.agents/skills/ai-agent-maintenance/references/writing-guidance.md`，
按本仓库任务整理；来源见 [来源索引](source-index.md#本地写作参考)。
采用具体用词、段落结构、语义例外和人工审阅方法；
不引入其他项目的站点禁词脚本、业务路径或中文词语识别 AI 作者的结论。
本文件包含执行所需内容，不要求客户端访问兄弟仓库。
