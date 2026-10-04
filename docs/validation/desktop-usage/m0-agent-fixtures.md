# M0：本机 Agent 版本记录与脱敏 fixture

## 元信息

| 项目 | 内容 |
| --- | --- |
| 日期 | 2026-09-24 |
| 执行环境 | Windows 11 Pro 26200 x64；Node v24.21.0；只读提取，未修改任何 Agent 原始数据 |
| 代码 revision | 工作树未提交改动 |
| 依据合同 | implementation-readiness.md「实施阶段的真实数据验证」；execution.md M0 第 5 项 |

## 范围与方法

- 有界发现：仅已知候选目录，不全盘扫描；版本从本地包/安装元数据读取，未启动 Agent 入口。
- 白名单字段：事件类型、schema/版本迹象、匿名 ID（anon-N 稳定映射）、时间、模型、token 数值及包含关系；
  正文以常量替换。SQLite 在暂存副本上只读查询，副本已清理。
- 产物在 gitignored 的 `build/desktop-usage-validation/`：`fixtures/<source>/`（每源附 `_expectations.md`
  人工可核算期望值）、`agent-inventory.md`、`tools/`（提取与泄漏扫描脚本）。
- 泄漏核查：`tools/leak-check.py` 扫描全部 42 个产物文件，0 泄漏（UUID/路径/邮箱/正文/用户名）；
  过程中发现并修复两处脱敏缺陷后重生成全部 fixture。

## 本机产品矩阵（实测）

| 产品 | 版本（核验来源） | 本地格式状态 | 样本 |
| --- | --- | --- | --- |
| Codex（VS Code 扩展宿主） | cli 0.155.0-alpha.16.3（rollout session_meta）；openai.chatgpt 扩展 26.917.62051 | rollout JSONL：token_usage_record 逐次 + token_count 累计 + turn_context；另有 6 个 sqlite | extracted（3 个会话，最多 49 调用/1457 行） |
| 新版 Kimi Code | desktop 1.0.3；wire protocol_version=1.5 | `sessions/<wd>/session_*/agents/*/wire.jsonl`；camelCase usage.record{inputOther,output,inputCacheRead,inputCacheCreation}，epoch 毫秒 | extracted（主线 + 子代理对账样本） |
| ZCode | 3.14.3（exe 3.14.3.7762）；协议客户端 0.16.9/schemaVersion 1 | model-io JSONL 双口径（AI SDK camelCase + anthropic snake_case）；db.sqlite 有 model_usage/turn_usage | extracted（8 请求尾部、日日志、两表样本） |
| Copilot CLI | 1.0.73（events.jsonl copilotVersion） | events.jsonl 事件流无逐次 token；session-store.db 的 assistant_usage_events 逐 turn 全字段 | extracted |
| Kilo Code CLI | 7.4.21（pnpm 包元数据）；session.version 观测 7.4.15–7.4.20 | `~/.local/share/kilo/kilo.db`（1.6 GB；**不在** ~/.config/kilo）；opencode 派生 SQLite，usage 在 message.data.tokens | extracted（schema + 脱敏样本） |
| VS Code | 1.139.0（code --version） | copilot-chat session-store.db 无 token 列（M5 再证）；扩展 kilocode.kilo-code 7.7.9 | schema_recorded |
| pi | 0.87.1（scoop） | sessions 目录为空 | no_data |
| WorkBuddy | 37.10.3-24（version 文件） | 三处目录均空/仅 version 文件 | no_data（残留） |
| Claude Code | 未找到 CLI 安装元数据 | ~/.claude 无 projects/，sessions/backups 空 | no_data |
| Gemini CLI | 未找到安装元数据 | ~/.gemini 无 chats/ | no_data |
| oh-my-pi | 18.2.11（scoop manifest） | ~/.omp/agent/{agent.db,history.db} 存在 | exists_only（合同限存在性检查） |
| qwen/opencode/mimocode/openclaw/hermes/zoo/dsh | — | 候选路径不存在 | not_found |

## 字段口径结论（实读核验，供 M1/M2 解析器合同）

- codex：total=input+output，cached⊆input，reasoning⊆output（98/98）；Σ逐次==最终累计快照（无压缩会话）；
  **compaction 会重置累计快照**（Σ32.06M≠快照 31.59M）；usage 无 model 字段，须按 turn_context 归属。
- kimi wire：四字段互斥无 total；`event.usage` 是 usage.record 的回声（防双计二选一）；
  subagent.completed.usage == 子代理 wire Σ逐次（逐字段相等）。
- zcode：同产品两种口径相反——response.usage.inputTokens **含**缓存读，anthropic.usage.input_tokens **不含**；
  turn_usage==Σmodel_usage（16 请求轮相等）。
- copilot：input=未缓存+read+write；request_multiplier=27.0 为付费倍率。
- kilo：total=input+output+reasoning+cache.read+cache.write **全互斥**（与上述源相反）；
  codex threads.tokens_used 与 rollout 逐次合计跨存储一致。

## 失败与未执行项

| 项 | 状态 | 原因/后续 |
| --- | --- | --- |
| 缓存写>0 场景 | 仅 copilot 覆盖 | codex/kimi/zcode 样本全 0；M2/M3 用合成样本补充并标识 |
| 模型中途变更 | 仅 codex 覆盖 | 其余源样本未观测到 |
| reasoning 字段 | kimi/zcode 样本无值 | 不补零，记 unknown |
| kimi wire 活文件 | 时间点快照 | 采样期间 394→515 行；期望值已注明快照时点 |
| .omp 数据库内容、WSL/容器实例、F1 产品 | 未执行 | 合同限制或不在本阶段范围 |

<a id="证据文件"></a>

## 验证产物

`build/desktop-usage-validation/`（gitignored）：agent-inventory.md、fixtures/（codex、kimi-code、zcode、
copilot-cli、kilo 共 5 源）、tools/（extract-jsonl.mjs、sqlite-probe.py、kilo-sample.py、leak-check.py）。
入库前需按合同复核脱敏 fixture；CI 只使用已审阅 fixture。
