# M3/M4（部分）：Kilo Code、ZCode、Kimi Code、Kimi Work 适配器

<a id="m3m4-partial-kilo-code-zcode-kimi-code-and-kimi-work-adapters"></a>

M3 首个适配器（kilo）与 M4 三个产品（zcode/kimi-code/kimi-work）实施完成。
本机未安装的 M3 其余工具（cline/opencode/mimo/zoo/dsh/openclaw/hermes）
按用户指示另行基于文档或源码实施，真实验收后置。

<a id="run-information"></a>

## 元信息

| 项目 | 内容 |
| --- | --- |
| 日期 | 2026-09-25 |
| 执行环境 | Windows 11 x64；rustc 1.98.0；Node v24.21.0 |
| 代码 revision | 未提交工作树（M6 之后 + 本次新增） |
| 设计依据 | adapters.md 接入矩阵（A11/A12/A13/A21）；architecture.md#database 来源 SQLite 读取规则；V07/V12/V17/V30 |
| 执行方式 | 四个适配器由三个并行子代理实现（测试样本由前次中断会话预提取），主会话集成（scanner 注册、clippy/fmt 清理、全量验证） |

<a id="commands-and-results-after-integration"></a>

## 命令与结果（主会话集成后）

| # | 命令（cwd） | 退出码 | 结果摘要 |
| --- | --- | --- | --- |
| 1 | `cargo test -p llm-usage-core`（desktop/src-tauri） | 0 | **373 passed / 0 failed**（M6 基线 285 + 净增 88） |
| 2 | `cargo clippy --workspace` / `cargo fmt --check` | 0 | 干净（app crate 存量 main.rs 测试模块顺序警告为 M0 遗留） |
| 3 | `npm run verify`（仓库根） | 0 | 49 个测试二进制全绿；lint 0；vite 构建不变 |
| 4 | `APPDATA=<临时> cargo run -p llm-usage-m0 -- --headless` | 0 | **7 实例全部 succeeded，27,311 事件**；再次扫描不重复入库（仅 zcode 活文件 +3） |

<a id="适配器结论详细证据在各-real_verify-输出与-fixture-期望文档"></a>

<a id="adapter-findings-real_verify-outputs-and-sample-expectations-contain-details"></a>

<a id="适配器结论详细核验结果在各-real_verify-输出与-fixture-期望文档"></a>

## 适配器核验结果（详见 real_verify 输出及测试数据期望文档）

<a id="kilo-code-cli-m3-a11"></a>

### Kilo Code CLI（M3，A11）

- 实读 schema：`~/.local/share/kilo/kilo.db`（519MB 活库 + 30MB WAL，25 表）；
  usage 在 `message.data.tokens`（13,342/13,342 assistant 全带，五互斥口径）；
  session 行 tokens 列仅作会话对账（275/276 matched；列级语义随版本不稳定，
  绝不映射为事件）；`session.cost` 实为模型 JSON 不读；新 core 数据层
  （session_message，0 行）仅存在不读。
- SQLite 来源读取实现：只读连接 + busy/锁/CANTOPEN 时 Online Backup 暂存副本
  （2s/2GiB 上限，用后清理；真实库直读未触发暂存，busy 路径合成测试覆盖）。
- 已验证 7.4.8/7.4.9（测试样本）；库内观测 7.3.42–7.7.12 未收录全部
  latest_fallback 统计。
- 真实核对：255 会话 13,342 调用（primary 11,745 + sub_agent 1,597），
  total **1,645,598,767** 与独立 Python 求和逐字段一致；重复扫描不新增记录；对账 254/255
  matched（1 mismatch 为源端快照未同步，诊断可见）。

<a id="zcode-m4-a21"></a>

### ZCode（M4，A21）

- 双口径互斥不混算：AI SDK camelCase 五键为主（inputTokens 含缓存读）、
  anthropic snake_case 对照（input_tokens 不含）；两视图齐备时逐条校验
  （in+cr+cw==inputTokens、out==outputTokens），矛盾记 dual_caliber_mismatch。
- 版本字段 `request.headers["x-zcode-app-version"]`；已验证 3.14.3。
- db.sqlite 只读对账：活库 418/438 matched（在途/取消轮 mismatch 可见；
  M0 静止样本 16/16）。
- 真实核对：3 文件 8 事件重复扫描不新增记录；`~/.zcode/v2` 布局与 `%APPDATA%/zcode`
  桌面存储未接入（待核验）。

<a id="kimi-code-m4-a12-kimi-work-m4-a13"></a>

### Kimi Code（M4，A12）与 Kimi Work（M4，A13）

- 家族共享 `adapters/kimi_wire.rs`（metadata 探测/usage.record 提取/回声去重/
  subagent 对账/epoch 毫秒校验），两产品独立目录与注册表（已核验协议 1.5 / 1.4）。
- 实读差异：Kimi Work 布局 `conv-*|ctitle-*`（daimon 宿主）、usage.record
  无 agentId、model 裸 id；跨文件同毫秒 2 对（swarm 并行）⇒ 事件键含
  `session:agent` 身份段（修复了首版会丢 2 事件的冲突）。
- 时间全部 epoch 毫秒（无秒样本，越界记诊断跳过，不做 ×1000 猜测）。
- 真实核对（与 jq 独立求和逐字段相等，重复扫描不新增记录）：
  - Kimi Code：13 文件 965 调用（primary 586+auxiliary 5+sub_agent 374），
    total **116,428,813**；10 文件对账 matched、2 文件 echo_subset
    （打断步无回声，差异可见）、1 个 subagent.completed 快照再次核对相等。
  - Kimi Work：69 文件 1,337 调用（833+8+496），total **125,225,933**；
    69/69 matched。

<a id="integrated-source-totals-actual-headless-collection"></a>

## 集成后全源汇总（headless 真实采集）

| Agent | 调用 | total_tokens |
| --- | --- | --- |
| kilo-code | 13,342 | 1,645,598,767 |
| oh-my-pi | 8,767 | 1,026,791,695 |
| codex | 2,597 | 299,747,341 |
| kimi-work | 1,337 | 125,225,933 |
| kimi-code | 965 | 116,428,813 |
| zcode | 266+3 活文件 | 100,651,285 |
| pi | 37 | 2,864,419 |

（claude/gemini/qwen 本机无数据，not_found/no_data 维持既有结论。）

<a id="outstanding-work-at-this-stage"></a>

## 未完成项

| 项 | 状态 | 后续 |
| --- | --- | --- |
| kilo 新 core 数据层（session_message，当前 0 行） | 未接入 | kilo 切换后核验专用实现 |
| zcode v2 布局、桌面 session 存储 | 未接入 | 待核验 |
| kimi usage.record 无稳定 ID ⇒ provider/逐次延迟不入账 | 如实标注 unavailable | 上游补充关联键后接入 |
| Kimi Work 缓存写>0、subagent.completed 真实样本 | 无本机样本 | 合成已覆盖路径；样本出现后补取 |
| Kimi Work 官方默认布局/env | 未见文档 | 安装位迁移需手工加根 |
| M3 其余工具（cline/opencode/mimo/zoo/dsh/openclaw/hermes） | 未实施 | 按用户指示基于文档或源码实施，真实验收后置 |

<a id="证据文件"></a>

<a id="validation-files"></a>

## 验证产物

- 适配器：`adapters/{kilo,zcode,kimi-code,kimi-work}/`、`adapters/kimi_wire.rs`；
  产品映射下沉：kilo/zcode common.rs、kimi_wire.rs（usage_map.rs 已清理）。
- 测试：`tests/{kilo,zcode,kimi_code,kimi_work}_{contract,gaps_synthetic,incremental_v12}.rs`；
  fixtures：`tests/fixtures/{kilo,zcode,kimi-code,kimi-work}/`（真实脱敏 + 合成，附 _expectations.md）。
- examples：`real_verify_{kilo,zcode,kimi_code,kimi_work}.rs`；
  提取工具 `build/desktop-usage-validation/tools/extract-kimi.mjs`（gitignored）。
- 应用注册：`src/scanner.rs`（10 适配器）。
