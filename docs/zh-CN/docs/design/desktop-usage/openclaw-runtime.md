# OpenClaw schema 24 本地读取说明

<a id="reading-local-openclaw-schema-24"></a>

依据为 npm openclaw@2026.9.8 的实装代码、官方固定 schema 与本轮真实本地模型
非空样本。公开 npm provenance 的 aa6008ad 提交、CLI 显示 fc23bc8 及实际编译
代码分别核对；provenance 提交新增的 contextUsage 在当前实装归一代码中不存在，
不得据此填造历史字段。源码与运行记录集中到本轮验收记录，状态见 [Plan.md](../../../Plan.md)。

每个 agents/Agent/agent/openclaw-agent.sqlite 为独立实例，排除全局 state 库。
探测同时核对 user_version=24、schema_meta 主行的 role=agent/schema_version/agent_id
及所需表列；schema_meta.app_version 为整库可变元数据，不能据此核验历史记录客户端版本。
最新兼容读取仍 latest_fallback，未知 schema fail closed。旧 sessions JSON/JSONL
为迁移输入，继续拒绝入账；不改变源文件或运行维护/迁移命令。

逐条读取 transcript_events 的 TEXT 或 zstd 正文；压缩只读解码，原始/解码正文
上限 4 MiB，必须匹配 event_utf8_bytes。导航索引不是用量正文，超限/坏行留诊断
并继续其他有效行。读取、schema/归属及归档检查处于同一只读 SQLite 快照。
每轮最多 50,000 行，以 session_id/seq 分页，末页从头复查可变历史，WAL 不用
字节偏移短路；事件与续读位置仍随统一批次同事务提交，回滚不推进。

仅已保存 session_entry_provenance=1、acp_owned=0 且无 plugin_owner_id、
hook_external_content_source 的本地 OpenClaw 会话纳入；缺失/外部来源保持隔离。
逐条 assistant message 的 api 必须是已核验的 openai-completions；其他协议不套用
相同桶语义。身份用 session_id+条目 id，其他消息/系统/自定义/快照不重复入账。
模型只取记录自身 provider/model，不沿用会话最新模型。调用类别及底层调用数在
未完成所有尝试归属核验前保持未知，记录为 usage_observation。

当前实装 input 为扣除缓存读/写后的非缓存输入，output 为总输出，cacheRead/
cacheWrite 缺字段默认零。仅保留有效正桶，零/缺失保持未知；不从计算所得
totalTokens 或缺省缓存推导完整输入/总量，不使用默认 cost=0 推断账单。
坏类型、负值、超限和矛盾保持可见；有效其他字段可继续保存。时间按实际字段
写入依据处理，不用数据库写入时刻补造请求完成时间。

活动 transcript 与归档不相加。当前未取得非空归档真实样本，存在归档时显式
提示覆盖缺口并保留已经保存的历史；不将空活动表核验为完整历史。Gateway、辅助/
嵌套/导入/旧版本及其他 transport 另验，不从本地 CLI 的两次调用扩大能力。
