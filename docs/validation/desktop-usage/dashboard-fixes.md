# 看板用户反馈修复与未提交代码复审（2026-10-03）

复审 2026-10-02 工作树的 8 项反馈修复，重新核对源码、测试、官方协议与本机数据。
原记录的“无需换汇”、658 构成和 Kilo 全库就绪结论已修正。正式库/配置只读，
只写隔离验证副本；本轮没有提交、推送或部署。

## 元信息

| 项目 | 内容 |
| --- | --- |
| 日期 | 2026-10-03 |
| 执行环境 | Windows 11 x64；Node 24.21.0；Rust 1.98.1；Edge headless；desktop 依赖已按锁文件恢复：Tauri API/CLI 2.12.0、Svelte 5.57.1、TypeScript 6.0.3、Vite 8.3.1 |
| 代码 revision | 工作树改动（未提交）：前端图表/设置/费用卡片 + 核心计价/适配器/健康度 |
| 依据合同 | [dashboard-polish](../../design/desktop-usage/dashboard-polish.md)、[dashboard-repair](../../design/desktop-usage/dashboard-repair.md)、[pricing](../../design/desktop-usage/pricing.md)、[data-contract](../../design/desktop-usage/data-contract.md)、[adapters](../../design/desktop-usage/adapters.md)、[architecture](../../design/desktop-usage/architecture.md) |

## 问题与修复

| # | 反馈 | 根因 | 修复 |
| --- | --- | --- | --- |
| 1 | 今日图表变柱状 | `TodayHourly.renderTotal()` 调用次数 series 为 `type:'bar'` | 调用和 token 均为平滑曲线；单小时按 Agent 的分类比较保留柱形 |
| 2 | 缓存有效期（天）显示 0 | `PricingSettings` 派生 `Default` 令 `online_cache_ttl_days=0`，`0 ?? 3` 仍为 0 | 手写 `Default=3`（DEFAULT_TTL_DAYS）；`load_settings` 将 0/越界规整回 3 |
| 3 | API 价格参考是「仅文字」 | 原修复仍在指标网格之外 | 纳入今日/趋势第八张带边框卡；十语言短标题“API 参考费用”，完整说明可悬浮查看。容器八/四/二/一列，额度独立。1920/1280/1024/760/480 无横向溢出、同列等宽、标题单行 |
| 4 | 币种是否都为美元/需否换算 | models.dev 与原生人民币价不能混为一谈 | models.dev 为 USD/百万 token；原生快照区分 USD/CNY。价格以 1/100 分/百万 token 存储，1M 输入+1M 输出回归分别得 USD 18、CNY 120。CNY 原金额/单价旁列约合美元，记录 ECB 日期/来源，不合并币种、不改历史 |
| 5 | k3-256k「暂无适用单价」 | 真实 `provider=kimi-code-owent`、`model_raw=k3-256k`（裸）；`reference_model_key` 别名键是带前缀 `kimi-code/k3-256k`，永不命中 | `reference_model_key` 与 `official_providers` 对裸 `k3`/`k3-256k` 也映射到 `kimi-k3`/moonshot → 按官方 USD 参考价计价 |
| 6 | 658 未计价 / 9 部分估算 | 原记录混淆当前参考与发生时价，已知零用量也误报未计价 | 区分无 token、缺价与部分覆盖，零用量按适用价记零。原快照修复后 622 未计价、9 部分，准确构成见下 |
| 7 | 数据源「需核对 38 个文件」 | 累计对照不相等被当成读取失败；合法 null info 也被当作字段异常 | `codex-rollout-3` 自动重放旧游标；保留 mismatch/regression，不单独降级读取健康。新版独立逐次和旧版 total/last 必需字段异常分别处理；坏行/时间错误跨批次保留。隔离副本中原 38 个文件全部重评为 active |
| 8 | kilo「兼容读取 1 个文件」 | 7.8.1 未登记；全库最高版本错误覆盖逐消息依据 | 34 调用真实 fixture 登记 7.8.1，旧版本表/游标自动重评。`kilo-message-tokens-2` 按消息所属 session.version 标记，混合库兼容状态跨增量保留；7.8.3 仅空会话不能认证，兼容提示说明自动检查/复核 |

## 658 与 9 的准确解释

口径是原应用快照中的**当前 API 参考**，不是发生时价格历史。

- 564 条 `codex-auto-review`：路由名不是已知真实型号，不套 GPT 系列价格。
- 34 条 `k3-256k`：裸 profile 别名缺失，已修复；全部按 USD 官方参考计价。
- 58 条 Copilot round 标记：没有 token 用量，保留调用，不填零、不制造金额。
- 2 条 GLM 已知零用量：旧判断把零金额误当“没有可计价分量”，已修复。

只读复查该路由的真实文件：turn_context 仅报告 codex-auto-review，逐次载体只有
调用身份与 usage 字段，没有真实模型字段；不能从 token 数量或额度模型猜底层型号。

同一快照修复后为 622 未计价（564+58）。9 条部分估算是 5 条 VS Code 用量
observation 加 4 条 Visual Studio 调用：输出/部分缓存已知，未提供完整未缓存输入拆分；
不能把整轮 input_total 直接套未缓存价。未知字段仍未知，重新采集的新记录会改变数字。

## 官方来源与自动核对边界

以下来源均于 2026-10-03 重读：

| 来源 | 采用的事实 |
| --- | --- |
| [Kimi Code 模型表](https://www.kimi.com/code/docs/en/kimi-code/models.html)与[全球价格](https://platform.kimi.ai/docs/pricing/chat) | k3/k3-256k 是 K3 profile；每百万 token 输入 USD 3、缓存读 0.30、输出 15，写 5m/1h 为 3/6，不含税；会员额度不等于 API 单价 |
| [models.dev 维护者说明](https://github.com/anomalyco/models.dev) | cost 本身是 USD/百万 token，不能因提供商带 CN 名称再次换汇 |
| [ECB 日 XML](https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml) | 有效日期 2026-10-02，同日 EUR/USD=1.1225、EUR/CNY=7.5259；CNY→USD=1.1225/7.5259 |
| [Codex 0.144 官方协议](https://github.com/openai/codex/blob/rust-v0.144.0/codex-rs/protocol/src/protocol.rs)与[额度事件问题](https://github.com/openai/codex/issues/14489) | TokenCountEvent.info 为 Option；本机 0.144.0-alpha.4 的 info:null、rate_limits:object 合法；token_count 可随额度刷新重新发出，不能逐条当调用 |

汇率是随代码版本记录的参考快照，不自动联网；金额/单价在各自整数单位上做同日
交叉汇率计算与四舍五入，显示原值、约数、日期和来源，不改写历史或原币种小计。
历史计价匹配规则标记更新为 official-reference-3，只修复仍保留且未封存的日期。

原 38 个 Codex 文件重评后全部 active；对账仍保留 mismatch，读取正常不等于证明
源记录覆盖完整。新增 0.159.0-alpha.12.1 文件仍按未验证版本兼容读取，不凭同形字段
认证。5 个专门回归覆盖已消费且字节未变、单调修订/历史、预算分批、null info 和坏记录。

Kilo 7.8.1 fixture 的 34 调用与独立合计 3,189,308 token 对账 matched，自动重评不双计。
实时全库已出现没有 assistant 用量的空 7.8.3 会话；全量核对 256 个有消息会话仍有
1 个历史累计不匹配，保留诊断和 degraded。不能写成“整个当前 Kilo 库已就绪”。
空/混合版本回归证明数据库最高版本不认证其他会话，也不降级已验证消息的依据。

## 命令与结果

| # | 命令（cwd=仓库根，除注明） | 退出码 | 结果摘要 |
| --- | --- | --- | --- |
| 1 | `npm --prefix desktop ci --ignore-scripts` | 0 | 按锁文件恢复依赖，没有修改锁文件 |
| 2 | `npm run verify` | 0 | 文档、资源、脚本、UI、Svelte、fmt、clippy、Rust、前端构建完整链通过 |
| 3 | `npm run test:rust`（verify 内） | 0 | 71 个测试二进制；811 passed、0 failed、3 ignored；含 dashboard_repair 14、Codex 自动重评 5、Kilo 8、费用设置 5 |
| 4 | `npm run check`（svelte-check） | 0 | 0 errors / 0 warnings |
| 5 | `npm run build:web` | 0 | 前端构建成功 |
| 6 | `npm run test:ui` | 0 | 16 passed；十语言、汇率单位/舍入、缺口原因 |
| 7 | `npm run test:browser`（Edge） | 0 | 曲线、费用卡/分辨率、TTL=3、CNY 金额/单价、原因说明、兼容提示与原有回归 |
| 8 | `npm run fmt:check` / `npm run clippy` | 0 / 0 | 格式与 lint 通过 |
| 9 | `npm run lint:md` | 0 | 171 文件 0 issue |
| 10 | `python -X utf8 build/review-feedback/run_live_probe.py` | 0 | 真实只读来源→隔离库；38 文件重评，K3 34 调用计价，版本/历史诊断保留 |
| 11 | `python -X utf8 build/review-feedback/audit_summary.py` | 0 | 脱敏字段白名单通过，34 条真实 backup 与 fixture 的五桶独立合计一致 |
| 12 | `git diff --check` | 0 | 最终工作树检查；未跟踪代码/fixture 另查 |

## 失败与未执行项

| 项 | 状态 | 原因 | 后续条件 |
| --- | --- | --- | --- |
| codex-auto-review 定价 | 未执行 | 伪模型名无公开价，真实底层模型未知 | 用户提供映射或配置后可计价；禁止猜测 |
| 正式库 degraded/compat 文件重评 | 隔离副本已验证；正式库未写回 | 原配置/数据库只读 | 用户更新后首次采集自动重放，无需清库 |
| 新 GUI 实机验收 | 未执行 | 本轮未启动新 GUI | 桌面验收随 build:desktop |

初次资源检查因安装的 Tauri 2.11.5 与锁文件不符失败；恢复依赖后 67 个派生资源、
82 个文件检查通过，未重生成或修改图标。Node 检查的 sandbox 子进程 EPERM 经
获准执行重跑；fmt 曾指出未加 --all 的遗漏，clippy 曾指出新语法不兼容仓库 MSRV，
均已修正。最终成功日志在 build/review-feedback/verify-final.log 与 browser.log。
未执行 release 打包、默认忽略的远端 models.dev smoke 与遥测实机配置写入检查。

<a id="证据文件"></a>

## 验证产物

- 本轮只读一致快照、探测脚本、隔离核对库和日志：`build/review-feedback/`（gitignore）；
  SQLite 用 mode=ro + Online Backup，不用 immutable 跳过 WAL。
- 新增 fixture：`desktop/src-tauri/crates/core/tests/fixtures/kilo/session-7.8.1-k3.sanitized.json`
  （脱敏：ids/paths/正文去除，仅保留 tokens/time/finish/modelID/providerID 结构化字段）
  与 `session-7.8.1-k3._expectations.md`（独立核算）。
