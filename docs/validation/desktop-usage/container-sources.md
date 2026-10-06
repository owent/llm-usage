# 容器内官方客户端与真实本地模型样本

2026-10-06：Qwen 0.25.0 SDK 连续 JSON、逐次 span 与原生分区择一已实施，
真实合计 2 次/16,423 token；OpenCode 1.18.34 已按实际记录版本注册并验收旧处理位置升级。
真实首记录的 Qwen 手工根路由也已修正。最新实现/包回读与剩余边界见
[来源规则升级](source-policy-upgrades.md)；八个 M8 客户端的后续真实样本及成品复验见
[M8 容器样本](m8-container-samples.md)；MiMo/Zoo/DSH 后续原始载体和最新包结果见
[三源验收](m3-container-samples.md)。下文保留 2026-10-05 原始核验与当时旧包结果。

日期 2026-10-05；cwd 为 WSL Debian 的本任务独立仓库；准备边界见
[真实数据验证](../../design/desktop-usage/implementation-readiness.md)。
用户允许 Podman 测试环境；按本地模型方式执行，未登录个人账户、读取个人密钥
或发起云端付费请求。安装软件与模型实际返回 token 分别记录。

## 已核验环境

rootless Podman 5.4.2，派生自 [安装验收](installation-lifecycle.md)中补 CJK 字体前的
Debian GUI 镜像 311d072b80b15c3a57a9afd8135deed530931aec6e4afac81361f35f6f19793e；
模型/客户端运行用户 UID 1000，新 HOME、无宿主挂载，`--network=none`。
llama-server 仅监听容器 127.0.0.1:8080，CPU 推理；该服务只用于产生本地真实记录，
未接入任何远端用量/账单 API。镜像构建与公共包/模型下载先联网，运行阶段离线。

- 官方 npm Qwen Code 0.25.0，Node 24.21.0；registry manifest 要求 Node >=22，
  dist.integrity 为
  `sha512-XlqtxN7UKEkXLpuKioorDTBoIR/rWMdNHMtO/aPuQb7HDsTswMPqkEMYsVY56wuF3xSaI9PKgh9Q2NU/CSTkAQ==`。
  以 [npm registry 当前条目](https://registry.npmjs.org/@qwen-code%2fqwen-code/0.25.0)
  与实际 --version 为准；搜索摘要中的旧版不作为安装依据。
- [Qwen 官方模型提供商合同](https://qwenlm.github.io/qwen-code-docs/en/users/configuration/model-providers/)
  已核验 OpenAI 兼容本地 provider。实际 CLI --help 确认 auth/base-url/model/plan/
  max-session-turns/output-format；容器使用新配置和无账户含义的本地鉴权占位值。
- 官方 [Qwen2.5-0.5B-Instruct-GGUF](https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/tree/9217f5db79a29953eb74d5343926648285ec7e67)，
  q4_k_m 文件 SHA-256
  `74a4da8c9fdbcd15bd1f6d01d621410d31c6fc00986f5eb687824e7b93d7a9db`。
- 官方 llama.cpp server 镜像固定 amd64 manifest
  `ghcr.io/ggml-org/llama.cpp@sha256:559ac229adefe0f7e2d4e32f5222f26927b6bb8ba44b9db08e2544afebf41984`；
  实际版本 0.5.0-dev build 11382 / commit 11fe02151。
  派生镜像 ID `c66a5d2978b14379e7c29493fd48f12e3c4ef9c13ebb7e3cbbcf61d20e1d9060`。

Qwen 客户端以 plan 模式、1 turn、JSON 输出运行，实际工具调用 0。应用使用当前实际
0.2.1 deb 的 --scan-once，独立数据目录与容器来源环境；不手写模型响应或 token。

## 默认场景及对照

| 场景 | CLI 直报调用与总 token | ChatRecord / 应用 | 独立核对 |
| --- | --- | --- | --- |
| 默认自动记忆 | 2 次 / 16,082；bySource.main=10,228，managed-auto-memory-extractor=5,854 | 仅主循环 1 条：input 10,226、output 2、cache read 0、total 10,228 | 主循环与模型服务 timing 一致；后台调用无会话逐次记录，覆盖缺口保留 |
| 关闭后台记忆 | 1 次 / 8,905；仅 main | 1 条：input 8,903、output 2、cache read 0、total 8,905 | CLI、模型服务、原生载体和实际 SQLite 全部一致；4 项核对退出 0 |

对照配置键从已安装 0.25.0 chunks 源码核验：schema 声明默认 true，配置加载器
读取 `settings.memory?.enableManagedAutoMemory` 与 enableManagedAutoDream，
forked agent 使用 managed-auto-memory-extractor 名称。仅新容器内设二者为 false。
不能凭主循环验证声称默认所有请求均已采集，也不将 CLI 总量减会话量补造事件。
后台 CLI 明确直报 prompt=5,639、cached=3、total=5,854；服务 timing 的 prompt=5,636、
total=5,851，与 API usage 计数不同，不能把 timing 直接替代 API 总量；此部分没有进入应用事件。

实际应用每次均发现 1 个 qwen-code 来源、health=ok；首次添加 1 条、再次添加 0 条，
diagnostics=0。事件保留 schema_version=0.25.0、原始本地模型 qwen2.5-0.5b-local，
canonical/provider 未知，不推断云端计费。完整已知输入/输出不表示全部客户端调用被覆盖。

正式对照脚本退出 0。默认脚本的最初离线审计用了错误 SQL 列名而退出 1，之后按
实际 schema 只读补核对，模型/客户端和两次采集均已成功；默认原始载体没有成功收集到
容器外，保留已抽取的白名单与 SQLite/CLI 统计，不声称保留了完整原始默认会话。
对照原始载体成功留在 WSL 专属 build/；Windows 只复制白名单及配置代码依据。
结果位于 build/install-lifecycle/qwen-evidence/ 和 WSL 的 real-qwen-default-results/
real-qwen-controlled-results/；只有脱敏字段投影进入 tests/fixtures/qwen/real-0.25.0-local-*。
投影删去正文/项目路径、匿名化 ID、同日平移时间，原 token 不变；合同测试检查
两个样本的字段、未知值、调用数和重扫时修订不变。
Windows 与 Debian 的 Qwen 合同各 3 项退出 0。最终能力说明已替换“无真实样本”，
明确默认后台记忆覆盖限制；最终 deb 在新的无网络容器重新读取保存的真实对照载体，
首次 1 条、二次 0 条、8,905 token、health=ok，SQLite 中能力说明同步，退出 0。
该回读不再发起模型请求，结果见 build/install-lifecycle/final-qwen-readback.json。

## Qwen 0.25.0 真实 file 遥测

继续用独立无网络容器、新 HOME、本地模型与默认后台记忆，启用 telemetry.outfile，
logPrompts/includeSensitiveSpanAttributes 均为 false，usageStatisticsEnabled=false。
先核对 [v0.25.0 固定 SDK 实现](https://github.com/QwenLM/qwen-code/blob/6788c035698a0ada471c958d1e789e96c6cddd9b/packages/core/src/telemetry/sdk-impl.ts)：
outfile 抑制 OTLP exporter；[file-exporters](https://github.com/QwenLM/qwen-code/blob/6788c035698a0ada471c958d1e789e96c6cddd9b/packages/core/src/telemetry/file-exporters.ts)
直接序列化 SDK 对象为连续多行 JSON，并非 JSONL 或 OTLP JSON envelope。

实际导出 25 个对象，包含两个 qwen-code.api_response 日志及两个
qwen-code.llm_request span。后者 kind=0（SDK INTERNAL）、身份为私有 _spanContext，
resource._rawAttributes 的 service.name/version 为 qwen-code/0.25.0。
主循环有 parentSpanContext，后台记忆 span 无父 span，不能据同一 session 将它
假定为主 span 的子调用；日志/span/HTTP span/metrics 都不能相互叠加。

| 来源 | API response 日志 / LLM span / CLI bySource 独立核对 | 原生会话 / 应用 |
| --- | --- | --- |
| main | input 10,226、output 2、cache read 0、total 10,228 | 1 条 / 10,228 token |
| managed-auto-memory-extractor | input 5,639、output 556、cache read 3、total 6,195 | 该调用未进入原生会话，未补造事件 |
| 合计 | CLI 2 请求、16,423 token、工具调用 0；字段与两条独立日志一致 | 应用重扫仍 1 条、health=ok、diagnostics=0 |

五项独立核对退出 0。后台缓存读 3 是真实 API/遥测直报；主循环缓存命中及其他
provider/版本仍待验证。真实 file 已找到后台逐次载体，但现有 OTel JSONL/Copilot
读取合同不覆盖该形状，不能仅改文件后缀或 span 名套用；尚未实施此载体解析及与
原生会话的权威分区选择，应用默认后台覆盖缺口仍保留。
原始测试 file/会话/SQLite 留在 WSL build/install-lifecycle/real-qwen-telemetry-1791206781/，
Windows 仅保存 qwen-telemetry-whitelist.json、qwen-export-audit.json 与固定源码。
原始 file SHA-256 为 5d0abf2ac630aaf64c13a7f870eb7c264d5ebc91e5b3dc534de7ba10925a4ef5。
此轮没有改变解析器或将两种载体合并统计。

## OpenCode 1.18.34 主循环与默认标题

同一离线 rootless 环境使用官方
[opencode-linux-x64 1.18.34 manifest](https://registry.npmjs.org/opencode-linux-x64/1.18.34)，
固定源码 [aec0b9a6d8898f68f923aaf08b7306d931fd9d76](https://github.com/anomalyco/opencode/tree/aec0b9a6d8898f68f923aaf08b7306d931fd9d76)。
官方 dist.integrity：
`sha512-RTAMjCve4euxP2QKLuvRmdoW5J5DQK1DiZqt+7slfixyjAEi79QC2Df2oYKogibaAI4IEU8uzenoJeEl3k+UEw==`。
官方直连发生超时后，用 npmmirror 的同名/同版本 tarball；执行前与上述官方
SHA-512 一致校验，未通过镜像包自行声明的摘要认证。实际包 60,309,530 字节，
SHA-256 b83e8ac66d752d05ead4b6a439d3a2cfa32bcd9817c708825a389b5d5cba4f19；
实际 --version 为 1.18.34。仅解包官方二进制，没有执行 npm 生命周期脚本。

[官方 llama.cpp provider 合同](https://opencode.ai/docs/providers/#llama-cpp)与实际 run
--help 已核验。新 HOME 中自建 provider 和 primary agent（steps=1、权限全部拒绝），
run --pure 禁用外部插件，关闭 models fetch/自动更新/分享；工具调用 0。
本地只保存 API response 的 usage、模型与消息角色计数；代理仅监听容器 loopback，
不保存请求正文、模型输出或认证头，不接入远端用量/账单 API。

| 场景 | 真实 API usage | CLI / 原生载体 / 应用 |
| --- | --- | --- |
| 默认标题 | 2 次：标题 input 539、output 10、total 549；主循环 input 298（含缓存读 3）、output 1、total 299；合计 848 | 仅主循环 1 条：未缓存 input 295、cache read 3、write 0、output 1、reasoning 0、total 299 |
| run --title 对照 | 1 次：input 298、cache read 0、output 1、total 299 | 同一主循环 1 条：input 298、cache read/write 0、output 1、reasoning 0、total 299 |

[固定 ensureTitle](https://github.com/anomalyco/opencode/blob/aec0b9a6d8898f68f923aaf08b7306d931fd9d76/packages/opencode/src/session/prompt.ts)
对默认标题发起独立 LLM 流，只取文本设置标题；明确标题则跳过。
[主循环 processor](https://github.com/anomalyco/opencode/blob/aec0b9a6d8898f68f923aaf08b7306d931fd9d76/packages/opencode/src/session/processor.ts)
将 step usage 写 part 与 assistant message，session 累计与 part 一致，但不含标题。
因此 matched 不证明全客户端覆盖；不能从差额补造标题事件，也不能将三个原生载体相加。
最初服务 timing 有两项推理任务但无可用 HTTP 请求计数；后续用真实 SSE usage 核对，
timing 不替代 API token 数量。

每轮五项独立核对退出 0；应用实际 --scan-once 两次后仍为 1 条/299 token，
health=ok、diagnostics=0。第二次扫描报告 events=1 是重叠窗的解析量，SQLite 未新增事件。
保留原始本地模型 qwen2.5-0.5b-local 与 provider llama.cpp，不认证云端价目。
真实库 session/part/message 行数 1/4/2；新 core session_message 为 0 行，
空表不作为新载体依据。无网络不代表已验收联网时全进程出站行为。

WSL 的 build/install-lifecycle/real-opencode-default-1791208377/ 与
real-opencode-controlled-1791208422/ 保存原生载体、API 白名单及应用库；Windows
保存 opencode-default-audit.json、opencode-controlled-audit.json、匿名投影和固定源码。
仅三表 DDL/用量字段及匿名必要字段进入
[默认 fixture](../../../desktop/src-tauri/crates/core/tests/fixtures/opencode/real-1.18.34-local-default/_expectations.md)与
[标题对照 fixture](../../../desktop/src-tauri/crates/core/tests/fixtures/opencode/real-1.18.34-local-controlled/_expectations.md)。
Windows / Debian 的 OpenCode 合同各 8 项退出 0，逐字段、累计对账与重扫修订不变通过。
当前数据仍标 latest_fallback：文件层使用库内最高版本选择依据，逐记录认证与混合
版本、空会话、未变化旧游标升级尚未完成；不能直接注册一条真实版本后认证整个库。
应用能力说明已更新真实范围、标题缺口与升级条件；未修改读取/计算或版本分派规则。
包含新说明的 deb 已重建，再用新容器回读保存的两份原生库：两次扫描后各仍为
1 条/299 token、缓存读分别 3/0、health=ok；SQLite 能力说明为 real-local，版本
注册表仍空，退出 0。Qwen 已保存对照载体也在新 deb 保持 1 条/8,905 token。
结果在 build/install-lifecycle/fuse-continuation-opencode-{default,controlled}-result.json
及 fuse-continuation-qwen-readback.json；均未再次请求模型。

## 仍待验证

此次证明 0.25.0 本地兼容 provider 的主循环，不能认证 Qwen 云端、其他版本、
真实归档或主循环缓存命中；SDK 后台载体已在 2026-10-06 接入，无需为已有合法记录降级来源健康。
Gemini 官方 [认证合同](https://geminicli.com/docs/get-started/authentication/)
仍列 Google 登录/API key/Vertex 路线，本轮未取得官方离线 provider 的已核验合同，
没有现成非空载体；不拿第三方 fork、模拟 Gemini API 或 Qwen 样本代替其真实版本认证。
OpenCode 其他版本、Windows 布局、非零费用、缓存写、reasoning 与标题独立载体及
跨版本载体仍待验收；1.18.34 的逐记录版本与旧游标升级已通过。
Windows Zed 本机库仍为空；其他产品的真实样本条件保留在 Plan.md。
