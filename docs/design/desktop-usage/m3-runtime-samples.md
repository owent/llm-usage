# MiMo、Zoo 与 DSH 真实载体合同

2026-10-06 已用官方分发物在独立 rootless Podman 中调用真实本地模型。
本文件约定读取行为；完成状态以 [Plan.md](../../../Plan.md) 和
[三源验收记录](../../validation/desktop-usage/m3-container-samples.md)为准。
容器、字段白名单和个人账户边界沿用 [准备合同](implementation-readiness.md)。

## Zoo Code 3.86.0

[官方发布](https://github.com/Zoo-Code-Org/Zoo-Code/releases/tag/v3.86.0)
对应提交 `6aa9d0174a9ecae155c6c5db9134bead4b67197d`，VSIX 摘要与发布资产一致。
真实 VS Code 扩展通过公开 API 启动任务，原生 `ui_messages.json` 包含普通文本、
推理、恢复提示及一个内联更新的 `api_req_started`；API、扩展回调和文件均为
输入 6,118、输出 53。该文件没有逐请求模型或客户端版本，不从全局配置补造。

按此提交 `packages/types/src/message.ts` 的完整 ask/say 枚举接受非用量消息并跳过；
未知类型仍整文件拒绝、保持游标。保留旧 finished 的 LIFO 合并与压缩辅助费用。
`tokensIn` 是含缓存输入，缓存是子集，不重复相加。实际 OpenAI-compatible writer
忽略嵌套 cached_tokens，四桶和费用有默认零，因此零保持未知；正输入/输出可派生
总 token。缓存未齐不拆未缓存输入。实际格式锚点不能认证未采样版本。

parser 升级须重新评估未变化的旧游标。只在重建旧规则后的完整事件摘要相符时，
修正默认零及解析依据；正值、质量、模型、身份、归属和修订改变仍进入冲突仲裁。
旧摘要两种版本、并行、事务回滚及同批冲突均须回归，保留观察时间和诊断历史。

## MiMo Code 0.1.15

[官方发布](https://github.com/XiaomiMiMo/MiMo-Code/releases/tag/v0.1.15)
对应提交 `14dfe68a1c121f859544ba810b3c308e8501bfb2`，Linux 资产完整性已核对。
真实 CLI 与续会话在 `mimocode.db` 保存八条 step-finish，与八条模型 API 用量逐条
对应。CLI 退出 0，但八次 finish reason 均为 length，不作为任务完成验收。

以该版本 session getUsage 的 SDK 归一代码核对输入、输出、reasoning、cache 和 total；
默认零不能认证缓存/推理不存在。输入 SDK 总量减缓存得到原生 input，输出 SDK 总量
减 reasoning 得到原生 output；将同一份缓存/reasoning 加回可恢复正 SDK 总量，
这项代数还原不把默认零子桶认证为报告零。源 total 保留独立对照；零费用未知。
MiMo 的新映射与 OpenCode 保持产品独立，不用血统
推断版本。只读 part，不叠加 message/session 的重复用量；所属会话版本逐记录保存，
最高库版本不认证其他行。旧处理位置和完整旧摘要升级须保护真实值、归属及冲突。
MIMOCODE_HOME 必须非空绝对路径，优先于 XDG；其 data 为基础目录。
MIMOCODE_DB 可为绝对/相对数据库路径，`:memory:` 没有落盘载体。
未知或混合版本继续逐记录 latest_fallback，不将一次 0.1.15 样本注册为全历史认证。

## DSH 0.2.0-rc.2

实际分发为 [官方 npm 包](https://www.npmjs.com/package/@deepseek-ai/dsh)，安装锁与
编译后的 persistence、LLM codec、token-meter 模块是该样本的字段依据。GitHub
当前提交与 rc.2 分发不是同一版本，不将当前源码当分发认证。

两轮 headless/续会话成功。持久化为 `session.v4.jsonl.zstd`：v4 session header，
随后带 seq/time/data 的事件。主循环两条 assistant/message settlement 保存用量，
内嵌 stream usage 是同一份用量，不能叠加。标题 API 另有一次调用，但只有请求标记、
没有持久化结果用量；保留覆盖缺口，不以累计差值补造事件。

新 v4 读取器独立于旧文档级 session log：检查头部、生成版本和本机来源证据；
有界解压和逐行检查，半写、未知格式、坏类型、重复身份与回滚分别回归。
用量与模型只取该次 settlement 的已验证字段，导入/种子历史不认证本机调用。
默认根、手工根和全注册表物理文件去重须经过真实发现路径验收。

实际 rc.2 token-meter 读取 `assistant/message` 的 data.usage，缺失时及
`assistant/attempt` 读取内嵌 stream 的末次 usage；同 turn/step 在重试前只保留末次
settlement，retry-started 才开新身份。stream 副本与上下文压力不另计。
seeded 头部须有 inherited end-seed 标记，继承前缀不入账；普通续会话的空 end-seed
不是继承标记。seq 必须连续，未知 required 类型拒绝，明确 ignorable 的未来类型跳过。

实装 pi-ai 0.87.1 的 openai-completions codec 将 input 归一为
max(0, prompt − cacheRead − cacheWrite)，output 已含 reasoning，total 为自算和。
仅在记录自身 replayState.response 明确此 API/version 2 且正 input 未触发零钳制时，
将原生缓存加回恢复正总输入；其他协议不移植。零分项未知，自算 total 不作独立源总量。
模型来自该次 message.source；缺失时不从可变全局路由补造。
文件与解压结果各 64 MiB，zstd window 64 MiB，单行 4 MiB、50,000 行上限；
完整有效快照才推进字节游标。超限或半写保持游标并明确诊断，不静默吞掉尾部。
默认 DSH_HOME/手工根提升到 sessions；同目录选最高 canonical 生成版本，
未来版本拒绝，不能回退旧文件掩盖升级。恢复全注册表的旧错误归属须有完整原生头部
且旧来源无明细/日/期间/来源汇总历史，文件与旧 checkpoint 转移同事务；只在没有
其他文件时退役旧空实例，保留诊断和用户其他坏文件。快照删除既有尝试时保留旧值并
提示 snapshot_regressed，不猜测删除意图或生成抵销事件。
