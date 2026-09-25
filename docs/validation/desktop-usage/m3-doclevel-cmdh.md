# M3（部分）：Cline、DSH、Hermes、OpenClaw 文档级证据适配器

本机未安装四个产品（2026-09-25 盘点 not_found，`~/.cline` 等候选路径不存在；
real_verify 四例 discovered_roots=0 为证据）。按用户指示以固定源码/官方文档
级证据实现，合成测试通过；**真实数据验收后置**（安装后经 discover 自动发现，
real_verify 入口复验）。适配器与 fixtures 均明确标注文档级证据。

## 元信息

| 项目 | 内容 |
| --- | --- |
| 日期 | 2026-09-25 |
| 执行环境 | Windows 11 x64；rustc 1.98.0 |
| 代码 revision | 未提交工作树（M3/M4 批次之后 + 本次） |
| 依据合同 | adapters.md（A03/A08/A24/A09 固定源码锚点）；V03/V07/V12/V17/V30 |
| 执行方式 | 三个并行子代理因 API 限额中断（2026-09-25 21:33），适配器目录已完整（5 文件/个）且编译通过；主会话补齐测试、examples、内联缺陷修复并集成 |

## 实现与证据

### Cline（A03，固定源码 dcf8c3c）

- usage 载体：type="say" 且 say ∈ {api_req_started, deleted_api_reqs,
  subagent_usage} 消息的 `text` JSON（tokensIn/tokensOut/cacheWrites/
  cacheReads/cost 逐字段可选）；四桶互斥，input_total=in+cw+cr 派生；
  "compaction" 估算不入账；任务目录 `tasks/<任务 id>/`ui_messages.json（整写数组）。
- 主会话修复（子代理未跑到的内联测试暴露）：`request` 键补入已知键集合
  （A03 文档记载字段）；map_cost 补负值/非有限拒绝（与 parse 口径一致）。
- 合同测试（3）：合成 contract 全链路（3 调用 input 475/total 530 人工核算）+
  幂等；能力声明含文档级标注；缺口场景部分可用。

### DSH（A08，固定 token-meter README 46a7f68）

- 折叠规则：final 样本替换同 attempt 流式值；retry-started 结束替换范围并
  新开计费 attempt；attempt/step 边界定稿末样本；occurred_at 用观察时间
  （README 无逐事件时间）；pressure/contextBreaks 估算不计账。
- 合同测试（4）：contract（2 计费 attempt input 140/total 158，含一次
  流式→final 替换 + retry）；replacement 场景（3 attempt input 195/total 208）；
  非用量词汇不产请求；能力标注"合成假设"。
- 落盘路径与行序列化未文档化：JSONL 行形状为合成假设（能力声明如实标注）。

### Hermes（A24，固定源码 ef70b36）

- session_model_usage 是模型/路由/任务累计表（非逐请求）：映射为来源区间
  汇总（UsageObservation/区间聚合），api_call_count 不拆成 model_call，
  跨日 first_seen/last_seen 不把累计全放最后一天（V03 Hermes 行）；
  辅助 task 累计与主会话累计互斥求和。
- 合同测试（8，子代理完成、主会话修类型/断言两处）：basic 累计、跨日、
  辅助互斥、v20 回填/压缩继承、能力 evidence_level=doc-level。
- fixtures：projection.json → SQLite 重建（schema DDL 随投影携带）。

### OpenClaw（A09，官方文档）

- 文档只给出存储位置形状（`~/.openclaw/agents/<id>/agent/openclaw-agent.sqlite`
  与旧 sessions/ 归档），**未文档化表级/条目级 schema** ⇒ 运行时库与旧归档均
  unknown_format fail closed（诊断 reason 注明待真实样本），不读表、不猜字段、
  不产零值；归档按迁移输入降级（官方 doctor --fix 迁移路径）。
- 合同测试（3）：运行时库 fail-closed + 幂等；归档不入账（占位数值 1200+300
  等明确不计）；能力声明如实。layout.json → 临时目录重建（sqlite/json/jsonl）。

## 命令与结果（主会话集成后）

| # | 命令（cwd） | 退出码 | 结果摘要 |
| --- | --- | --- | --- |
| 1 | `cargo test -p llm-usage-core` | 0 | 全绿（新增 cline 3 + dsh 4 + openclaw 3 + hermes 8 + 内联若干；verify 53 个测试二进制） |
| 2 | `cargo run --example real_verify_{cline,dsh,hermes,openclaw}` | 0 | 四例 discovered_roots=0（not_found 证据；无数据目录） |
| 3 | `npm run verify` | 0 | lint 0、clippy/fmt 干净、前端构建不变 |
| 4 | `APPDATA=<临时> cargo run -p llm-usage-m0 -- --headless` | 0 | 14 适配器注册；7 实例 27,158 事件（与上批一致量级，活文件微增） |

## 过程缺陷与修复

| 处 | 问题 | 修法 |
| --- | --- | --- |
| scanner.rs | 适配器注册表两次被外部改写回旧内容（编辑器/工具回写旧缓冲；探针定位：app 零发现 vs 探针 7 根） | 重新接线并以断言式脚本验证；新增 `examples/discover_probe.rs` 诊断工具（按 app 同一上下文逐适配器打印发现根） |
| cline 内联测试 | "request" 未入已知键集合；map_cost 不拒负值 | 补键集合 + 非有限/负值拒绝（口径与 parse 一致） |
| hermes_contract | 元组类型第 4 位误标不可空；evidence_level 断言过严 | 改可空类型；前缀断言 |

## 未完成项

| 项 | 状态 | 后续 |
| --- | --- | --- |
| OpenCode / MiMo Code / Zoo Code 适配器 | 未实施（实施子代理因限额中断且未留产物） | 限额恢复后按 A17/A14/A19 固定源码实施（同文本档级证据模式） |
| 四产品真实数据验收 | 后置（本机 not_found） | 安装后 real_verify 复验 + 脱敏 fixture 补取 |

## 证据文件

- 适配器：`adapters/{cline,dsh,hermes,openclaw}/`；
- 测试：`tests/{cline,dsh,openclaw}_contract.rs`、`tests/hermes_contract.rs`；
- fixtures：`tests/fixtures/{cline,dsh,hermes,openclaw}/`（合成 + 期望文档）；
- examples：`real_verify_{cline,dsh,hermes,openclaw}.rs`、`discover_probe.rs`。
