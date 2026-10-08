# M8 容器真实来源样本

<a id="m8-real-container-source-samples"></a>

日期：2026-10-06；0.2.1 未提交工作树。临时产物位于仓库根
build/plan-final-push；WSL 工作副本为 /home/owent/llm-usage-platform-auth-20261005。
按 [准备要求](../../design/desktop-usage/implementation-readiness.md) 只保留选定的
字段。容器客户端调用真实本地模型；没有使用个人账号或付费云端请求。

最新受测包已在后续 OpenClaw 阶段重建：十源原始记录/旧库升级及 Hermes/Cline/Qwen/OpenCode
回读全部保持原数值和覆盖边界，Windows 12/17/8 项及 Linux 47 项再次通过；
最新摘要/命令根见 [OpenClaw 阶段](openclaw-container-sample.md)，以下前阶段摘要保留历史。
2026-10-07 后续取得 Zed 1.22.0 / DbThread 0.3.0 指定外部 Provider 的原生样本，
见 [后续记录](plan-20261007.md)。这项更新取代下表当时仅核验 hosted 路线的限制，
不改变本文十源阶段的历史结果。

## Aider 0.86.2

官方 [Docker 安装](https://aider.chat/docs/install/docker.html) 指向 paulgauthier/aider；
按 [兼容 API 配置](https://aider.chat/docs/llms/openai-compat.html) 使用本地端点。
带版本的 0.86.2 镜像标签不存在，首次拉取退出 125；随后拉取文档所述镜像，
实际 aider --version 为 0.86.2，固定为以下清单后执行：

- 镜像：docker.io/paulgauthier/aider@sha256:764924922f1f9a47e1185ebaa72e6435027af657deaff3febf0f20a176403c1e。
- image ID：4596c7c132b3d8f9594e81324c46cbc588b95847929325478d60b37d6d63b647。
- 原始 analytics.jsonl SHA-256：93fa009a0c7e92544d54cb15a5488245c69c398018c24fa007d6e6c4c3e2924c。
- 样本：core/tests/fixtures/aider/real-0.86.2，含脱敏 analytics、独立 API 用量与来源说明。

客户端和模型使用 rootless Podman 普通 UID 1000，外部网络关闭、不挂载宿主目录；
共享隔离网络中的回环端点。模型为真实 llama.cpp/Qwen 2.5 0.5B 推理，
模型身份保持 qwen2.5-0.5b-local。ask 模式、非流式、禁用 Git/自动提交/更新检查/
网络 analytics，显式 --analytics-log。本地模型元数据费率为 0，不能作为云端价格依据。

实际安装包 analytics.py 的守卫和写入文件路径确认：设置 logfile 即使未开启网络
analytics 仍写本地事件；base_coder.py 在发送结束后记录 message_send 的 token/cost。
真实六行包含 launched、no-repo、auto_commits、message_send_starting、message_send、exit；
仅 message_send 有用量。日志没有产品版本字段；0.86.2 只作采样来源，
注册表仍为 aider-analytics-doc-1 格式参考版本，不能核验其他记录版本。

独立结果一致：模型响应 prompt/completion/total 为 95/3/98，CLI 显示
“Tokens: 95 sent, 3 received.”；应用为 1 次、95 输入、3 输出、98 总 token。
模型 API 的 cached_tokens=0 未进入 analytics 记录，应用缓存与推理分项保持未知。
本地 cost=0 仍为 Estimated，total_cost 累计值不叠加。

首次采样 root aider-1791265417 没有用量事件：测试代理错误要求必须显式
stream:false，实际客户端省略该可选字段；代理 AssertionError 导致重试。
按 [API 文档](https://developers.openai.com/api/reference/resources/chat) 修正非流式默认值，
未伪造响应。首次文件保存还缺少目标目录，清理同时删除共享网络容器的次序有误；
保留日志后创建目录、按客户端再模型顺序回收，最终成功 root aider-1791265647。

已验收包在独立无网络容器读取原始日志，完整扫描后重扫为 1 次/98 token；
aider health=ok。直接手工文件也触发 Zed 的格式诊断，保留为其他来源 degraded，
不能把它归为 Aider 解析失败。结果 root aider-readback-1791265745。
新增 aider_contract 经整个注册表验证相同结果、未知分项、Estimated 成本与重扫，退出 0。

验收仅覆盖该版本一次成功 ask 调用；云端、编辑循环、多发送、失败/重试用量、
分支、缓存和其他版本仍未验收。新版能力说明已更新，后续包复验另记。

## Goose 1.53.0

从 [官方发布](https://github.com/aaif-goose/goose/releases/tag/v1.53.0) 的 API 读取
goose-x86_64-unknown-linux-gnu.tar.gz；下载 SHA-256 与官方 digest 一致：
deb2191a6b75acc0a20232fc5c52655ea2f9cc8fa2f5dffc8622e8d378a915dc。
实际二进制 --version 为 1.53.0。固定版本的 cli.rs、openai.rs 与
session_manager.rs 确认 --no-profile/--max-turns、自定义无鉴权非流式端点、
响应后写 usage_ledger；按 [官方配置](https://github.com/aaif-goose/goose/blob/v1.53.0/documentation/docs/getting-started/providers.md)
在容器内配置 custom_local，不改宿主配置或凭据。

无外网/宿主挂载的 rootless Podman 普通用户调用同一真实本地模型，
禁用扩展、限定一轮、显式会话名。API 和逐请求库均为 1 次、320 输入、
2 输出、322 总 token。cache_read_tokens=0，cache_write_tokens/cost/cost_source
为 NULL；session 累计的 cache_write=0 来自产品 unwrap_or(0)，不能替代 ledger 的未知。
--stats 输出未找到 token 数字，不声称完成 CLI 统计的三方核对。

原始 sessions.db SHA-256：f1c1d714b195aa1795c50ed2db1795613d868a07340cbf0b1a563384fdfebdcf。
成功 root goose-real-1791266205；新 deb 默认发现并重扫为 1 次/322 token，
仅 goose health=ok、无诊断，root goose-readback-1791266290。
真实 schema 与选定字段的记录位于 core/tests/fixtures/goose/real-1.53.0。
新增 goose_contract 验证整注册表发现、ledger 与累计互斥、未知成本/缓存写/
未缓存输入、原始库字节不变和重扫，退出 0。客户端版本只作来源说明，
注册表仍保留 goose-usage-ledger-1 格式参考版本。

初次解包守卫预期只有 goose，实际官方包含 ./ 和 ./goose，退出 1；
保存首失败后严格接受这两个实际条目，未跳过校验或重复下载已验证归档。
真实验收尚不覆盖 desktop、旧库、重绕/分叉、子 Agent、压缩及其他版本。

## Continue CLI 1.5.47

按 [官方 CLI 配置](https://docs.continue.dev/cli/configuration) 的本地 YAML 和
[兼容端点](https://docs.continue.dev/customize/model-providers/top-level/openai) 接入真实
本地模型。官方 npm 包 @continuedev/cli@1.5.47 与安装锁的完整性一致：
sha512-gtpewV3RoIOD9dyTtKIBi1SY0VOHRu3Ehe7C/mmnswm+j34MPyrcQhQaWj/m+jdfGO4fNIKdrgGIlLso1ULDFw==。
安装后隔离镜像 ID ec002ed485dbcc28879a695c09da7f33ef3a334dc97bcdfb01aefd13897e4dde。
准备镜像期间下载依赖，实际客户端执行时无外网/宿主挂载，普通 acceptance 用户；
只读模式、独立 CONTINUE_GLOBAL_DIR，模型名保持真实本地名称。

真实 SSE 原样转发，客户端自行传入 include_usage=true；代理只保存模型、状态与
最后 usage，不保存请求/响应正文。API 为 1 次、1,471 输入、2 输出、1,473 总 token；
CLI 回答 OK，session usage 累计输入/输出为 1,471/2。客户端只写入文件会话累计，
没有原始总量/逐请求身份或模型、没有区间起点；应用不补造总 token、调用数，
不把 mtime 端点伪装为完整日归属，日汇总仍未知，保留独立会话汇总。
totalCost=0.001475 未作为供应商费用入库。

实际包 session.ts:74–81/130–137 初始化缓存两桶为 0，149–177 仅非零响应值才累加。
本次 API cached_tokens=0、未报告 cache write，写入文件却有缓存读/写均为 0：
记录不能证明零值来自 API。解析器 continue-session-usage-2 将缓存零保留 unknown，
正数保持原生累计；不改变输入/输出，不从混合 provider 关系派生完整总量。
旧未变化游标自动重读；完整旧聚合摘要仅允许两桶 0/reported→NULL/unknown，
保持源修订。token、其他质量、覆盖或更高旧修订仍由原冲突处理保留；
聚合/指纹/游标同事务，保留旧诊断和新增规则升级诊断，不修改源文件或封存历史。

原始会话 SHA-256 eb93d5dba3db44af18952e9e2a5dadf80d223a53ddec3e9575bc4c01e7fa023f；
mtime=1791266584000。成功推理 root continue-real-1791266577。
首次提取把同目录 sessions.json 索引数组当会话对象，退出 1；已保存完整文件，
区分对象后只读提取，未重复模型请求。脱敏样本位于
core/tests/fixtures/continue/real-1.5.47。

continue_contract 三项退出 0：真实累计与未知分项、未变化旧游标/完整旧摘要修复/
并行包装/事务回滚、真实 token/质量/覆盖冲突与更高旧修订保护。
最初测试接线不符合实际并行 API，编译退出 101；随后日期断言错误期待未知起点
汇总进入当天，断言失败；按既有区间规则改验独立汇总及日未知，两者不是产品缺陷。
M8 19、并行 3、汇总 4 项共同通过。跨云端/版本、多模型、继续/分叉、GUI 仍未验收。

实际 deb 旧/新包往返 root continue-readback-1791267437 退出 0：旧库已消费且
字节/mtime 未变化，升级后无需清库将缓存 0/0 改为 NULL/NULL，输入/输出仍为
1,471/2、源修订 1791266584000 不变；parser 为 continue-session-usage-2、health=ok。
旧完整 hash 190f4216e8675ec3 被规则纠正，新增升级诊断 1、真实冲突诊断 0；
二次扫描不再新增诊断，调用/完整总量未知。旧包从此前回读容器的已保存归档恢复，
摘要确认仍为 69abe4b2463459d2969bf582d874d1e82796b14e77e5823f4ea5f38dc1adbd46。

## jcode 0.91.0

[官方发布](https://github.com/1jehuang/jcode/releases/tag/v0.91.0) 的
jcode-linux-x86_64.tar.gz SHA-256 与官方 digest 一致：
74e9426b0a8e26c5fcff7803d6a5716f8294d1158fa63feb1b429450ac3f705e。
归档实际为 jcode-linux-x86_64 启动脚本和同目录 .bin，版本输出
jcode v0.91.0 (439a243bb)。首次准备错误预期单一 jcode 文件，StopIteration
退出 1；保存后严格核对两个实际普通文件和启动脚本，按原布局安装。

固定版本 [OAUTH.md](https://github.com/1jehuang/jcode/blob/v0.91.0/OAUTH.md) 与实际
CLI 确认 provider add 的本地端点/--no-api-key/--context-window；调用时
--provider-profile local-model、--tool-profile none、--no-update/--no-selfdev，
先 telemetry disable。真实客户端和模型在同一无外网、无宿主挂载的 rootless
Podman 普通用户容器内，SSE 原样转发，未伪造响应或用量。

API、CLI 和 session 快照的 input/output/cache-read 一致为 460/2/0，API 总量 462。
自定义 profile 的 provider_key=local-model；应用不推断为 openai，不按缓存读零猜
缓存写或推理零，完整 total、uncached、cache-write/cost/reasoning 保持未知。
快照 env_snapshots 的创建时版本为 v0.91.0 (439a243bb)，但它不能核验混合会话的
每条消息；注册表仍用 jcode-session-1 格式参考版本，未扩展其他产品版本支持。
prompt_tokens 数字保留在脱敏记录，不叠加到 input_tokens。

原始快照 SHA-256：
9b80f14e29be44cb94cd2b5355bfabaaa5acc2c15b40e80e4a08a945581de8c7。
成功 root jcode-real-1791268010；脱敏 session、CLI/API 选定字段及来源说明位于
core/tests/fixtures/jcode/real-0.91.0。
jcode_contract 1、M8 19、M8 修正 27 项退出 0；整注册表 JCODE_HOME 显式隔离、
仅 1 个有效来源/调用、460 输入/2 输出、未知桶、原始字节和重扫不新增用量通过。
最初测试设置 home_dir 触发 Kimi Work 的既有本机默认候选，导致来源数量断言失败
退出 101；改用显式 JCODE_HOME 且不给默认 home，没有据此修改产品发现规则。
本次只有成功单轮快照；journal、云端、分支/重绕、多模型、子 Agent、失败/重试、
缓存命中和其他版本均未验收，成品包回读另记。

## gajae-code 0.18.7

[官方发布](https://github.com/Yeachan-Heo/gajae-code/releases/tag/v0.18.7) 的 gjc-linux-x64
为 161,092,808 bytes；SHA-256 与官方 digest 一致：
c7d745845ac975a500a836c2e362e74791e6f430cffd760c0d186bc5e1cb8486。
实际 --version 为 gjc/0.18.7。初次下载超出 240 秒退出 1，保留 85,983,232 bytes；
后续仅在服务端返回精确 Range 的 206/Content-Range 后续传，完整校验后才执行。
未将不完整文件当作安装成功。

固定版本 [models.md](https://github.com/Yeachan-Heo/gajae-code/blob/v0.18.7/docs/models.md)
确认 local-model 的 baseUrl/auth:none/api:openai-completions 和显式真实模型配置。
GJC_CODING_AGENT_DIR 独立，--print/--mode json，禁用 tools/LSP/MCP/rules/title，
thinking off，系统提示为纯测试文本。普通用户、无外网/宿主挂载的 rootless Podman
调用同一真实本地模型；客户端自行请求 SSE usage，代理原样转发。

API、CLI message_end 和 session v5 assistant usage 一致为 412 输入、2 输出、414 总量。
文件 10 行，新增 configured_model_chain；
固定 [session-manager.ts](https://github.com/Yeachan-Heo/gajae-code/blob/v0.18.7/packages/coding-agent/src/session/session-manager.ts)
确认它只是配置回退链的持久化/回放状态，不是用量。仅增加该非用量类型，其他
未知类型仍整文件停读并保持游标，不能以新版本名跳过未知正文。

固定 [parseChunkUsage](https://github.com/Yeachan-Heo/gajae-code/blob/v0.18.7/packages/ai/src/providers/openai-completions.ts)
1991–2033 行中，缓存/输入/输出缺字段回退为 0；输入为 prompt-cacheRead-cacheWrite，
总量取报告值与组件和的较大者。缓存零无法区分 API 报告零与缺字段，初始化零同理。
新 gjc-session-2 对该 API 的零桶保留 unknown；无法确认两缓存桶时 uncached 未知，
总输入按归一反变换恢复为 412，输出 2、完整总量 414，源总量独立保留，成本 Estimated 0。
没有将 API 独立 cached_tokens=0 注入记录字段依据，也不套用于其他 API。
全零控制记录仍计已观测调用，token 保持未知。

openai-completions.ts:609–614 在连接前建立输出；
[createInitialResponsesAssistantMessage](https://github.com/Yeachan-Heo/gajae-code/blob/v0.18.7/packages/ai/src/providers/openai-responses-shared.ts)
1166–1183 将 timestamp=Date.now()。本次 message.timestamp=1791268562127，
比条目写入文件时间早 288 ms；现在标 source_start，发生时间数字不改，未伪装完成时间。

原始 JSONL SHA-256：
be9f3e72bc71e3d58a81ef23fd7cf68e0c41fe30d0fb5a1c812ed1dfb002affb。
真实执行 root gajae-real-1791268555；脱敏 10 行及 CLI/API、来源说明位于
core/tests/fixtures/gajae-code/real-0.18.7；产品版本与 session 格式版本 5 分开。
gajae_contract 5 项退出 0，包含全注册表隔离发现/重扫、完整旧 canonical/legacy
摘要与已消费游标、并行包装、检查点失败回滚、其他 token/质量/模型/成本/修订
冲突、同批次真实冲突保留、旧全零改 unknown，以及未知类型停读。
显式旧规则纠正只放行上述字段，观察时间、修订、原冲突标记与诊断保留，未封存汇总
与事件/检查点同事务。旧规则对照数据只在首次扫描前去除新非用量行，用量字段不变；
它不冒充未经改动的产品日志。

首次专项退出 101：测试错误期待“旧修订有值、新修订缺失”不标冲突；
既有冲突处理要求完整可比修订，原摘要已正确保留，修改测试断言后通过，未放宽产品冲突处理。
首失败日志 gajae-target.log 和最终 gajae-target-final/gajae-zero-final.log 分别保留。
真实验收仅覆盖这一成功本地调用；云端、其他 API/版本、失败/重试、缓存命中、
多模型、子 Agent、分支/重绕仍未验收。实际包旧/新往返结果另记。

<a id="package-rechecks-after-five-clients"></a>

## 五个客户端完成后的成品复验

Windows 与 Debian 构建均退出 0；统一检查 Rust 938（核心 837、应用 101，
默认忽略 8）、前端 21、脚本 4，Markdown 192 文件、Svelte 0 错误/告警、
fmt、Clippy -D warnings 与前端构建通过。Debian 实际 workspace 日志为
935 项通过（核心 837、应用 98，默认忽略 9）；此前独立凭据 6 项仍按原阶段记录。

| 受测制品 | 字节 | SHA-256 |
| --- | ---: | --- |
| Windows exe | 9,994,752 | a8bdaff6fb9a9d4bd5db31d1224d5eba6019fd050dbc4822069135503c7cde13 |
| Windows NSIS | 3,944,393 | a451f70d1b6b1e04bdbddca05d78cc83059c153feeaf11d94fb396ed7fe1a2b1 |
| Debian deb | 5,470,798 | e901252da13179ab3cbeca98906f5e93d72149ea97635ea2d1f28f42a256733e |
| Linux AppImage | 111,143,416 | 85d839e6db240c1bd7071f312dca30aa81f52ba69a519c899d68dce60731b8bf |

以下均使用上表实际制品；不重复模型推理，只读原始记录并保持来源字节不变。
Windows 命令 cwd 为仓库根，Linux 命令 cwd 为 WSL 独立副本，均退出 0：

| 验收 | 结果与日志目录 |
| --- | --- |
| test:install:windows | 12 项；build/install-lifecycle/windows/1791269838899；真实 NSIS 升级/回滚/卸载/重装与写者失败中止，自有集成残留 0 |
| test:install:linux --screen-reader --appimage-mode fuse | 9 组、47 项；WSL build/install-lifecycle/linux/1791269789954；实际 deb 往返、普通 UID 1000/CapEff=0/Seccomp=2 的 GTK 与只读 FUSE、退出释放；Orca 五页导航/语音及品牌名称；专属容器已回收 |
| test:headless | 11 项，3 事件/75 合成 token；build/plan-completion/headless/1791269785308 |
| test:receiver | 8 项，自有凭据残留 0；build/plan-continuation/native-receiver/1791269789134；真实 IPC/HTTP，不声称真实 exporter |
| test:desktop | 17 项、20 次小数据首屏 P95 736.4 ms；build/plan-completion/native/1791270091144；真实 WebView2/IPC |
| Aider 原始 analytics 回读/重扫 | 1 次/95 输入/3 输出/98 总量，health=ok；aider-readback-1791269832；其他手工格式的 Zed 诊断保留 |
| Goose 原始 sessions.db 默认发现/重扫 | 1 次/320 输入/2 输出/322 总量，health=ok、诊断 0；goose-readback-1791269839；cache-write/cost 未知 |
| Continue 旧库→新包/重扫 | 输入 1,471/输出 2，缓存 0/0→NULL/NULL，源修订不变，完整 total/calls 未知、health=ok；continue-readback-1791270088；规则升级诊断 1、真实冲突 0 |
| jcode 原始快照默认发现/重扫 | 1 次/460 输入/2 输出，cache-read=0、完整 total 未知、health=ok、诊断 0；jcode-readback-1791269789 |
| gajae-code 原始 JSONL 旧库→新包 | 旧包因 configured_model_chain 停读 0 条；新包 1 次/412 输入/2 输出/414 总量，缓存/uncached 未知、source_start、health=ok；gajae-readback-1791269788；旧格式诊断保留 |
| gajae-code 旧规则控制库→新包/重扫 | 首次前仅去除新非用量条目，用量不变；旧完整摘要 fnv1a64:40c2834cd0e94759 与已消费游标自动重评，缓存 0/0、uncached 412→未知，completion→start；数字时间/修订不变，规则升级诊断 1、真实冲突 0，同目录上述结果中单列 |

Continue 旧包 SHA-256 为 69abe4b2463459d2969bf582d874d1e82796b14e77e5823f4ea5f38dc1adbd46；
gajae-code 旧包为 6faa4f4a93fca930b197000e5db32b99feb38a5ad1c69485747c7cf52fc1bcf6。
后者旧规则对照数据 SHA-256 为 31f066bc087dfb6c7df12a2f18baba8a436aa759582d4962fe173599d03ce40d，
与未经修改的真实 JSONL 分开报告。实际 GUI、合成生命周期及真实记录回读各有独立核验结果，
不能用同一包版本号替代制品摘要，不能用来源客户端安装核验其他场景。

## AtomCode 5.2.1

[官方安装](https://atomcode.atomgit.com/docs/en/getting-started.html) 指定 npm
@atomgit.com/atomcode；平台包 5.2.1-linux-x64 的 SHA-512 与 registry dist.integrity 一致：
ncC/rqORQeDLMPH39NpjchZEoFCBmC4uosaWaKe8spbRv3Ek51/HvRQuJMMaC6amPrkRdfjSRZYRxP8ktz2oTg==。
归档只有 package/bin/atomcode 与 package/package.json，二进制实际输出
atomcode 5.2.1 (unknown)。GitHub 镜像 releases/latest 返回 404，未猜下载地址；
随后读取官方安装脚本与 npm 元数据，不执行宿主安装脚本。固定镜像源码
45e05cb14775f070f3867539e1f21c9849b3484b 的 Cargo 版本同为 5.2.1，但 binary build id
未知，不声称证明二进制等同该提交；保留原格式参考版本 atomcode-meta-turns-1。

按 [官方配置](https://atomcode.atomgit.com/docs/en/configuration.html) 和实际 CLI
使用本地 OpenAI 端点，headless --no-tools/--dev/--no-telemetry/--output-format jsonl，
独立 ATOMCODE_HOME、无账户含义的本地鉴权占位值，禁止外网/宿主挂载、普通 UID 1000。
官方网页的旧 --max-turns/--disable-tools 选项不作为本次参数，实际帮助与固定源码
确认 --no-tools 及 coding.max_rounds。模型和 SSE 沿用真实本地服务，响应未伪造。

真实 root atomcode-real-1791270565：API、CLI usage/turn.completed 和 `.meta` v1
输入 6,176、输出 2、总量 6,178、round_count=1、tool_call_count=0 一致。
created_at=1791270569650、updated_at=1791270575895；TurnStat 没有发生时间，
保留会话区间/来源报告调用汇总，不补造逐次事件。末次请求 total_tokens 不另叠加。
原始 .meta SHA-256：3e5db1220bad2e44993054bb4257ea70c6580cfdec99f3aedf8d674c58e2f0d7。
脱敏 .meta、CLI/API 选定字段及 provenance 位于 core/tests/fixtures/atomcode/real-5.2.1；
删去名称、owner/origin/import/fork、正文和工作路径，数值与时间保持不变。

固定 openai_compat.rs:1785–1803/manager.rs:680–715 确认 API 缺字段和
TokenBreakdown 反序列化都回退 0；缓存累计零不能确认 API 报告，cache-read/uncached
保持未知。有效归一桶恢复正总输入 6,176，输出 2、总量 6,178 保留；全零控制仍保留
reported round=1，token 未知。旧代码还把缺少/坏桶按零累计；现在坏桶/溢出仅保留
未知与受限诊断，其他有效模型继续入库。parser 为 atomcode-meta-turns-2。
同修订升级仅在完整旧聚合摘要可由默认零与原派生桶重构时放行；其他 token、质量、
调用、区间、覆盖或较高旧修订仍保留原冲突处理，聚合/检查点同事务。

真实目录另有 `.ui.json` v1（entries）与 `.rewind.json` version 2（points）。
固定 manager.rs:1069–1077/3253–3308 确认是辅助状态；仅完整已核验形状、合法
同名 `.meta`/身份配对后有界排除（64 KiB），其他辅助格式或未知版本仍诊断。
手工目录中坏文件/真实 usage JSON 不按名字隐藏；当前手工入口规则为目录根。
最初控制错误使用 rewind version 1；回查原始辅助文件数值后改为实际 version 2，
并确认未核验的 version 99 仍保留格式诊断，最终六项退出 0。
最初测试错误以手工文件根调用只枚举目录的 discover，退出 101；按实际入口规则改为
目录后通过，没有为该测试扩大产品入口。

六项 AtomCode 规则测试覆盖全注册表真实记录/重扫、已消费游标/完整旧 hash、
并行包装、检查点失败回滚、其他字段/较高修订保护、全零、缺桶/坏类型/溢出和
辅助状态/手工错误。初次命令误用了不存在的 parallel_scan 目标，退出 101；
按实际 parallel_scans 修正，日志分开保留。统一检查首次因新增诊断未格式化退出 1，
随后因 is_none_or 超过项目 MSRV 1.77.2 退出 101；改为兼容写法，不提高 MSRV。
只有该次成功本地单轮；云端、其他版本、继续/分支、子 Agent/后台与缓存命中未核验。

<a id="package-rechecks-after-six-clients"></a>

## 六个客户端完成后的成品复验

Windows 统一检查退出 0：Rust 944（核心 843、应用 101，默认忽略 8）、前端 21、
脚本 4，Svelte 无错误/告警；最终 rewind version 2 修正经六项 AtomCode 规则测试确认，
冻结后的完整统一检查再次退出 0，数量相同。
Debian 实际 workspace 941（核心 843、应用 98，默认忽略 9）退出 0。
随后分别构建当前 Windows release/NSIS、Debian deb/AppImage；源代码在构建与
成品复验间保持不变。此前五客户端的制品表保留其历史验收范围。

| 当前受测制品 | 字节 | SHA-256 |
| --- | ---: | --- |
| Windows exe | 10,000,896 | fc68aa7b60c8a0a65435663566712d70d177949f38b0335207eef537198fa953 |
| Windows NSIS | 3,945,809 | 397b2425c76a397c22e88eaa010a056b435f7e142d1530dbddc950851d0d0c03 |
| Debian deb | 5,472,434 | 07970ce598fc6fbe10e08af769d3a544c902acb85e470d8ed653ba279cda8ec6 |
| Linux AppImage | 111,143,416 | 50f3b106c478b01b8e93679d8cc7189edcc93d834dc502ad70e1c98a563deb1f |

以下命令均退出 0；Windows cwd 为仓库根，Linux cwd 为 WSL 独立副本。
六源只读既有真实记录，未重复推理，扫描前后来源文件摘要不变。

| 验收 | 结果与日志目录 |
| --- | --- |
| test:install:windows | 12 项；build/install-lifecycle/windows/1791271903027；NSIS 升级/回滚/卸载/重装及失败中止，自有集成残留 0 |
| test:install:linux --screen-reader --appimage-mode fuse | 9 组、47 项；WSL build/install-lifecycle/linux/1791272024234；实际 deb 往返、GTK/IPC、FUSE 只读挂载及退出释放、Orca 五页名称/键盘/语音；普通 UID 1000、CapEff=0、Seccomp=2，无网络/宿主挂载，容器已回收 |
| test:headless | 11 项，3 事件/75 合成 token；build/plan-completion/headless/1791272017297 |
| test:receiver | 8 项，自有凭据残留 0；build/plan-continuation/native-receiver/1791272022051；真实 IPC/HTTP，exporter 仍为合成配置 |
| test:desktop | 17 项、20 次小数据首屏 P95 766.9 ms；build/plan-completion/native/1791272022393 |
| Aider 原始 analytics/重扫 | 1 次/95 输入/3 输出/98 总量、health=ok；aider-readback-1791272167；其他手工格式的诊断保留 |
| Goose 原始 sessions.db/重扫 | 1 次/320 输入/2 输出/322 总量、health=ok、诊断 0；goose-readback-1791272171；cache-write/cost 未知 |
| Continue 旧库→当前包/重扫 | 输入 1,471/输出 2，缓存 0/0→未知，修订不变、health=ok；continue-readback-1791272170；规则升级 1、真实冲突 0、total/calls 未知 |
| jcode 原始快照/重扫 | 1 次/460 输入/2 输出，cache-read=0、完整 total 未知、health=ok、诊断 0；jcode-readback-1791272171 |
| gajae-code 原始/旧规则控制库→当前包 | 原始新非用量行获识别，控制库完整旧 hash 自动纠正缓存/uncached 与时间依据，数值时间/修订不变、health=ok；gajae-readback-1791272171；1 次/412/2/414、规则升级 1、真实冲突 0 |
| AtomCode 原始完整目录旧库→当前包/重扫 | atomcode-readback-1791271970；1 次/6,176/2/6,178、修订 1791270575895 和区间不变，缓存/uncached 默认零→未知；health=degraded→ok、规则升级 1、真实冲突 0、逐次事件 0、重扫不新增用量 |

AtomCode 旧包即上阶段 e901252d…；原始目录包含未修改的 `.meta`、UI v1 和 rewind v2。
旧完整摘要 fnv1a64:e3f4f4038bc2d9f0 与已消费检查点自动重评，新摘要
fnv1a64:4a44cb7e34542f28。旧 unknown_format 4 条保留，新包不再添加两种已核验
辅助文件的格式误报。首次成品回读 root atomcode-readback-1791271915 中产品结果
已正确，但脚本错误要求全部诊断计数不变，新增 scan_completed 导致退出 1；
改为历史单调保留、unknown_format 严格不新增后在独立新库复验通过，未清库掩盖产品问题。

<a id="command-code-1743-preparation-and-authentication-limits"></a>

## Command Code 1.74.3 准备与认证限制

[官方 BYOK](https://commandcode.ai/docs/byok) 文档支持 providers.json 的本地无密钥
OpenAI 端点、--local-only/CMD_LOCAL_ONLY。官方 npm command-code@1.74.3 的 dist.integrity
经 SHA-512 验证：K0eLCJtkWDvdR7JegxANKe5CUdsujO7+VtGeH/SLyjTWRnXLPoR5poBKiedgoRdn0NNtEQ/+/tE/3VqjAsmtQA==。
实际 --version 为 1.74.3；离线准备镜像
f1ff5b83dcc302e1f1d6e7fa62c8a370d90ca12c81370274a3bf75a1b98dca7f。
完整依赖仅在准备容器安装，客户端在无网络、无宿主挂载、UID 1000 和新 HOME 运行。
真实模型已启动，但没有收到 API 请求。

root commandcode-real-1791272678：实际 --local-only、--no-auto-update、--no-skills、
--skip-onboarding、--permission-mode plan、--model local-model/qwen2.5-0.5b-local、
--max-turns 1 与 -p/--output-format json，退出码 3：Not authenticated，要求 cmd login。
未生成 projects 会话文件。输出中的四桶 0 是失败结果初始化，不能证明存在用量。
该分发物 resolvePrintAuthentication 要求 isAuthenticated=true；isAuthenticated 读取
Command 账户 key，未在该入口按 local-only 放行。没有注入测试环境变量、伪造凭据、
修改客户端或登录个人账户。当前只完成安装/真实启动限制核验，仍不计入六个真实用量样本。

## Crush 0.97.1

[官方发布 v0.97.1](https://github.com/charmbracelet/crush/releases/tag/v0.97.1) 的
Linux x86_64 tar 为 26,726,787 bytes，SHA-256 与官方 release digest 一致：
1b7cbe0600a3797538a74dc00dd8c4bac54ac4b8f4455ba5ff2312b29c7bd598。
实际输出 crush version v0.97.1。同标签源码核验了新 crushrc 配置与全部 29 个
工具名；逐项 permissions deny，不使用通配符猜测。关闭 provider 自动更新与 metrics，
模型和客户端仍在普通 UID 1000、无网络/宿主挂载的 rootless Podman 内。

root crush-real-1791273563：主循环及自动标题共 2 次真实 API 请求，输出 OK。
API 输入 5,059、输出 4、总量 5,063、缓存读 4；原生根会话/CLI 统计页却为
上下文快照 4,899 输入/2 输出/4,901 总量，不能沿用其 SUM 为累计用量。
[agent.go:2036–2086](https://github.com/charmbracelet/crush/blob/v0.97.1/internal/agent/agent.go)
确认成本累加而 token SET 覆盖；子会话成本仍回卷父会话。应用保持 token、调用和
模型未知，只采根会话 Estimated 成本。格式参考版本 crush-sessions-cost-1 不扩大为产品版本核验。

本次明确设置人工验收费率：每百万输入/输出各 USD 1，缓存读/写各 0；它不是
本地模型价格或实际账单。独立 API 按 prompt-cached+output 计算 5,059 micro-USD，
原生与 CLI HTML 内嵌 total_cost 均为 0.005059。原始数据库 SHA-256：
443b94f91fa1995f70af99123807c660d3b4088113d5f8d3f4519c4dd4ccc911，完整性检查通过。
core/tests/fixtures/crush/real-0.97.1 仅保留真实 schemas、数值/时间、脱敏身份及
CLI/API 选定字段；正文/项目路径省略。原始 parts 类型只有 text/finish，各 2 个，无工具调用。
crush_contract 1 项退出 0，全注册表
发现、单一 cost-only observation、未知桶、只读来源及重扫不新增用量通过；当前成品回读待记录。

首次模型调用成功，但后续统计收集误用 stats --json 导致整体退出 1；源码/帮助
确认 stats 生成 HTML，改为只回读原保存库，未重复模型推理。第一次误把输出路径
通知按 JSON 解析，另一次复制目标尚未创建退出 125，原日志分别保留；最终读取
生成的 HTML 内嵌统计退出 0。未运行统计页浏览器，云端/压缩/子会话/其他版本未验收。

<a id="junie-26922-341929"></a>

## Junie 26.9.22（3419.29）

[官方自定义模型](https://junie.jetbrains.com/docs/custom-llm-models.html) 与实际帮助
确认自定义完整 Chat Completions URL、无 API key 及独立 JUNIE_HOME。官网安装脚本
下载返回 403，随后读取官方 GitHub 固定提交 472d75becf96aa4a37733797f37309ca2a42f980；
不执行宿主安装脚本。官方 release 清单的 3419.29 Linux amd64 ZIP 为
337,552,368 bytes，完整 SHA-256 一致：
7ac5d675d90305c65207f9ddaf4219a1bf78c34630b8e39423833d2716ce7b5e。
慢速首下载仅停止本任务精确匹配子进程，保留 59,768,832 bytes；余段逐段核对
206/Content-Range、完整大小及整包摘要后才解包，未把半包当作已安装。
实际版本 Junie 26.9.22 (3419.29)，离线镜像
ae1b5f9b69f8e03bf7c634776f29630081c5da78f61de65bc38e1302f801414f。

首次显式 chat 模式退出 1，实际稳定版要求 Nightly，未发 API 请求。
随后按稳定版 classic 模式执行，root junie-real-1791273837：产生七次真实本地调用，
原生 events.jsonl 43 行、7 个 LlmResponseMetadataEvent，最终因小模型响应格式不符
退出 1。API 合计 prompt 64,074、缓存读 53,102、输出 98；CLI 与记录的 inputTokens
为去缓存后的 10,972。任务失败不消除已发生用量，也不能核验成功任务场景。

发行包中的 LlmMetadata.onLlmEvent 从 AIAnswer.usage 构造原生 ModelUsage；
UsageTokens 构造及反序列化均将缺数字段默认零。OpenAIChatRequest 的输入为
prompt_tokens−cached_tokens，缓存写默认零；Responses/Anthropic/Google 的转换
也使用独立归一桶。事件未保存 API 类型/provider/产品版本，不能从模型名推造
完整总输入/总 token；输入正值保存为 input_uncached，正缓存读/输出独立保留。
缺失或零分项保持 unknown。AIAnswer.time 的默认零不能确认耗时。
calcTokenCost 使用 ModelCapabilities 的每百万价目估算四桶及 Web Search；
本次自定义模型无价目而 cost=0，保持 unknown；正费用为 Estimated，USD 单位
沿用原文档依据，未核验付费渠道/账单。

真实 events.jsonl SHA-256：529330c708ec839b3c840e54503ba6cc7bf158c0f6150e762eed45d9df061580。
fixtures/junie/real-3419.29 保留全部 43 行的位置、类型及七个用量事件/时间戳，
剔除非用量 payload、环境/路径/正文；API 和 CLI 最后 result.errorCode 数组是
原生用量字段。合计 10,972 非缓存输入、53,102 缓存读、98 输出与独立 API 一致；
API 全输入/总量 64,074/64,172 仅用于对账，不填补来源记录未知桶。

junie-events-doc1→2 保留原始桶/费用组成的事件键与修订，自动重读旧游标；
最多 128 个默认零/缺失候选仅重构允许的旧差异并比较完整 canonical/legacy 摘要。
输入位置、费用质量及零耗时纠正不允许模型/正数量/其他质量/修订等变化混入。
观察时间、真实冲突标记与历史保留，事件/汇总/游标同事务。JUNIE_HOME 真实发现
入口已补。junie_contract 五项通过，覆盖真实/API/CLI/全注册表只读与重复读取不新增用量、
旧游标/两摘要/并行/检查点回滚、其他内容及同批冲突、全零/缺字段、坏条目与正费用质量。
首次测试命令缺 manifest；随后测试用错私有辅助函数/ScanTarget/列名而编译失败，
按实际定义修正后退出 0，初始日志保留。完整验证及当前 deb 原始来源回读/旧游标迁移通过，
结果见下述最新阶段。

静态工具第一次尝试发行包内 javap 退出 127，jdk.jdeps 不存在退出 1；按用户已授权
在 WSL Debian 安装官方 openjdk-21-jdk-headless 21.0.12.1+1-1~deb13u1 后 javap 读取通过，
未修改官方 JAR。其他版本/API 的运行、流式/付费渠道、完整隐藏调用和 IDE 未验收。

<a id="eight-source-package-rechecks"></a>

## 八源阶段成品复验

冻结业务源码后，根 npm run verify 退出 0：Rust 950（核心 849、应用 101；默认
忽略 8）、前端逻辑 21、脚本 4、Markdown 192、Svelte 0 错误/0 警告、fmt、
Clippy -D warnings 与前端构建通过。Debian workspace 947（核心 849、应用 98；
默认忽略 9）退出 0。八产品专项合计 23 项；此前阶段结果不改写为本阶段结果。

| 当前受测包 | bytes | SHA-256 |
| --- | --- | --- |
| Windows LLMUsage.exe | 10,002,944 | 07521361b500982a9cadb485fb64acbf3eb2f03a14a01bf07731ffe8f617c603 |
| Windows NSIS 0.2.1 | 3,947,611 | 969b39b97f7511a34d55464dde9a1b935d8f721b333090b019606a0d53d065d5 |
| Debian 0.2.1 | 5,472,992 | c50e4bb168eee805b295fd6141184e8a6a14bd601766b1f337cd0cedf1523e78 |
| AppImage 0.2.1 | 111,147,512 | 806cfe2dace748ad83de4d1742058039b4874386e7fa276ed31ae30d1082bd73 |

Windows 安装 root build/install-lifecycle/windows/1791275512662 的实际 NSIS 往返
12 项退出 0，自有系统集成残留 0；无界面 root build/plan-completion/headless/1791275588209
11 项（3 合成事件/75 token）；接收 root build/plan-continuation/native-receiver/1791275585960
8 项，自有凭据残留 0。原生 WebView2 root build/plan-completion/native/1791275584776
17 项、20 次首屏 P95 776.3 ms，退出 0。

Debian root build/install-lifecycle/linux/1791275588714：9 组/47 项退出 0，实际
旧/新 deb 安装、升级、回滚、卸载、重装/清除，AppImage 只读 FUSE/GUI/退出释放，
Orca 五页原生 Tab/Enter、AT-SPI 名称和语音输出通过。仍用先前固定 Orca 镜像，
普通 UID 1000、CapEff=0、默认 seccomp、network none、无宿主挂载，只有自有
rootless 容器显式 SYS_ADMIN。宿主注销、其他 DPI/发行版/辅助技术及物理音频不扩大。

以下均用本阶段 c50e4bb... deb 回读保存的真实原始来源，重复读取且来源字节未变化；
路径相对 WSL build/plan-final-push（OpenCode 列相对 build/install-lifecycle）。

| 来源 | 成品结果 | 结果根 |
| --- | --- | --- |
| Aider | 1 次，95/3/98；缓存未知；目标 health=ok | aider-readback-1791275681 |
| Goose | 1 次，320/2/322、缓存读 0；其他未知 | goose-readback-1791275676 |
| Continue | 输入 1,471/输出 2；旧缓存 0→NULL，同修订/未知 total/calls | continue-readback-1791275679 |
| jcode | 1 次，460/2；缓存读 0，总量未知 | jcode-readback-1791275678 |
| gajae-code | 1 次，412/2/414；原样配置链恢复，旧规则对照数据完整摘要修正/历史保留 | gajae-readback-1791275679 |
| AtomCode | 6,176/2/6,178、原报告调用 1；旧缓存/未缓存→NULL、同修订，旧 unknown_format 历史 4 保留 | atomcode-readback-1791275679 |
| Crush | 1 个 cost-only observation，Estimated 5,059 micro-USD；token/model/calls 未知 | crush-readback-1791275588 |
| Junie | 原七条：非缓存 10,972/缓存读 53,102/输出 98；输入/总量/默认零费用及耗时未知 | junie-readback-1791275627 |
| OpenCode | 默认/对照各 1 次，298/1/299，缓存读 3/0，known_version/覆盖提示不变 | fuse-continuation-opencode-default-1791275761 / controlled-1791275765 |

Qwen 原生先入、SDK/副本择一复验退出 0：有效 2 次，15,865 输入/558 输出/16,423 总量，
缓存读 3；原生排除 1、SDK 副本排除 2，目标 Qwen/OTel 健康，其他手工格式诊断保留。
Junie 初次旧包回读只设置 JUNIE_HOME，旧版本不支持该入口而发现 0 项，退出 1；
按新旧共同支持的隔离 HOME/.junie 布局重验，不改源事件。旧 deb 07970ce... 已归档，
原七条旧 input_total 为 10,972，缓存/成本/耗时默认零；升级后原键/首次观察不变，
7 个 parser_policy_updated、无冲突。旧缓存包含关系诊断各 5 条保留，二次扫描不新增。
首次失败与最终通过日志分别保留。所有自有验收容器均停止并移除，缓存镜像保留。

## Xum 0.30.0

官方 [npm 元数据](https://registry.npmjs.org/@coder/xum/0.30.0) 的 gitHead 为
81b0b744db6e27a4416f3596d70bf88529171caf；32,673,885 字节 tarball 已核对
SHA-512 integrity：
`dPdpIxj8o0gZw+Kt5xWe93QMMIZhKC3DKDzqW7JDLEUs/n67lGuNnkexNFQwjz9xejsBhzuGyt/ZexwyhleSRQ==`。
官方包实际 --version 是 v0.30.0-dirty (81b0b744d)，保留原样，不将 dirty 标记归因于
本轮修改。只在自有 Podman 安装依赖（ignore-scripts），镜像
c97123cc6297913ea9aacc8269b49a54ae5c0c3a67e2b41425172a31d1a534ac。

[CLI](https://xum.coder.com/reference/cli)、[Providers](https://xum.coder.com/config/providers)
与固定源码支持无 key 的 openai-compatible 自定义 provider。真实推理在 network none、
普通 UID 1000、无宿主挂载容器执行，XUM_DISABLE_TELEMETRY=1，--no-mcp-config、
thinking off，max_output_tokens=16；不使用账户或 fake key。官方 runSessionRoot.ts
支持 XUM_RUN_SESSION_ROOT/MUX_RUN_SESSION_ROOT 保留原生 sessions，默认临时配置退出删除。
paths.ts 另确认 XUM_ROOT/MUX_ROOT 与新旧 .xum/.mux 根。两次原生 chat 均仅有两个 text
part，CLI 无工具事件；模型实际回答未经修改。

首次 xum-real-1791276725 使用默认网关/Plan 模式：实际 API 调用 1 次、HTTP 200，
但客户端未请求 stream_options，模型响应没有 usage；官方 SDK 默认 includeUsage=false，
Xum providerModelFactory.ts:1574–1580 没有传该设置，SDK 会覆盖同名请求参数。
原生 byModel 与 lastRequest 的五桶全为 0，chat.metadata.usage={}。
Plan 模式因未提出计划退出 1，不能核验成功任务或报告零；原始记录 788 字节，SHA-256
bbbbe31cfa7d2216f441a9dc74d7bf31a0b03f0974d68a30ade4fcd4eaf69af6。
首次 SDK 检查误用了容器不存在的 rg，日志保留；改用已有 Python 阅读原分发物，
没有安装或修改 SDK。

独立对照 xum-real-1791276921 使用 Exec 模式，官方客户端及 provider 不改。
本地网关只给上游实际 llama.cpp 请求增加 stream_options.include_usage=true，
每个实际 SSE 响应字节原样转发，不创建或覆盖 usage。API 一次调用 13,721 输入、
2 输出、13,723 总 token、cached_tokens=0；CLI run-complete 与 chat.metadata.usage
一致，任务退出 0。原生 byModel.input=13,721、output=2，其他桶 0，未写 cost_usd；
CLI cost_usd=0 不能确认费用。该设置是明确的网关对照，不作为客户端默认覆盖能力。
原生文件 796 字节，SHA-256
b5adf0dac5bf304a2852c0f13b5817b2502825e534e7f91ff8538fc234a094f1。

固定 displayUsage.ts:117–147 将输入扣去缓存读/写、输出扣去推理，缺字段归零，
历史值还有下限限制；sessionUsageService.ts:173–195 按模型累加，lastRequest 只是末次。
解析器改为 xum-session-usage-2：正 input 映射 input_uncached；零五桶未知；
正文本输出加已知推理、质量 derived，推理未知时保留文本下界及覆盖提示，健康不降级；
完整输入、total、调用和费用仍未知。完整旧摘要最多 32 候选，仅恢复已明确的旧桶/默认零，
保护修订、区间、身份、质量、覆盖及其他正值。默认缺 usage 场景的旧五零也会自动修正，
保留一条全未知的原生累计范围，不能确认调用或完整用量。

脱敏测试文件 位于 core/tests/fixtures/xum/real-0.30.0，原生 usage 文件本身只含模型/
数值/时间，无需改写；API 只保留选定字段。六项专项通过：两真实场景/全注册表/重扫、
输入与已知推理分项、坏桶与溢出、未变化旧游标/并行/回滚、其他字段与较新修订保护、
四个官方环境入口与未知 schema 的 latest_fallback。首次测试两次类型/结构编译失败、
一次未知版本预期错误均保留；按实际定义及既有兼容规则纠正后通过。

成品回读 xum-readback-1791277675 用已验收旧 deb c50e4bb... 与当前 4dde217...：
原始两种记录格式及其辅助文件 SHA-256 全部不变，两个旧游标仍在，原键/修订/created_at 保留；
原五零改为全 unknown，正 input_total 13,721 改为 input_uncached，正输出下界 2 保留。
2 个 aggregate_parser_policy_upgrade、0 个新冲突、1 个不降级健康的输出覆盖提示；
重扫不新增，来源健康 ok。未验收其他版本、云端、带推理分项的模型/缓存命中、子代理/
重建/跨日回卷，不从 schema v1 推广到所有产品版本。

<a id="nine-source-package-rechecks"></a>

## 九源阶段成品复验

包含 Xum 修正的 Windows 统一验证 Rust 956（核心 855、应用 101）、前端 21、
脚本 4；Debian Rust 953（核心 855、应用 98，忽略 9）退出 0。九源专项共 29 项通过。
成品来自同一冻结源码，历史八源阶段包摘要与首次失败保留：

| 成品 | 字节 | SHA-256 |
| --- | ---: | --- |
| Windows release exe | 10,006,528 | d185adde19096b6207ed14233bba9f7211a9474d83ebd19a6b85f0a03e46a73b |
| Windows NSIS | 3,949,453 | ac25b4a1bbfd36eba38be965941fce953bc91fdab0a2395b862acfba9da4c8a4 |
| Debian | 5,475,376 | 4dde217007fd7dc24b99ea6074babd2ae6f07f4a0bc72c2e50c883dca79fa4a9 |
| AppImage | 111,147,512 | 7e67071f7e357c714198876c57b69b6c41205b18505eef572bcef0c9c164f06e |

Windows NSIS 12 项（1791277785564）、无界面 11 项（1791277904563）、真实接收器
8 项（1791277901389）退出 0，自有凭据残留 0。当前 Linux 9 组/47 项（1791277727107）
退出 0，实际 deb 往返、AppImage FUSE、GTK/WebKit 与 Orca 五页导航/语音均通过。
Windows 原生 WebView2 root build/plan-completion/native/1791277903313，
17 项、20 次首屏 P95 729.7 ms，退出 0。

当前 deb 回读九源并重扫退出 0，既有八源数值与升级保护保持：

| 来源 | 结果及根（WSL build/plan-final-push） |
| --- | --- |
| Aider | 1 次/95 输入/3 输出/98；aider-readback-1791277771；无关手工 Zed 格式诊断保留 |
| Goose | 1 次/320/2/322；goose-readback-1791277765；写缓存/成本未知 |
| Continue CLI | 1,471/2，默认缓存零改 unknown，修订保留；continue-readback-1791277766 |
| jcode | 1 次/460/2，完整 total 未知；jcode-readback-1791277768 |
| gajae-code | 1 次/412/2/414，原始/对照完整旧摘要保护；gajae-readback-1791277768 |
| AtomCode | 6,176/2/6,178，默认缓存/未缓存未知、调用汇总 1、旧诊断保留；atomcode-readback-1791277767 |
| Crush | cost-only Estimated 5,059 micro-USD，原始库/注册表不变；crush-readback-1791277772 |
| Junie | 七条非缓存 10,972/缓存读 53,102/输出 98，旧七键/诊断/观察时间不变；junie-readback-1791277767 |
| Xum | 默认全未知与对照非缓存 13,721/文本下界 2，两个旧摘要升级；xum-readback-1791277675 |

测试数据 39 文件/118 JSON 对象检查通过，秘密键/个人路径 0。所有任务产物仍仅在忽略的
build 根；文档/Skill 格式及引用随最后同步复查，不将阶段构建当成已发布/远端 CI 结果。

<a id="remaining-local-sampling-routes"></a>

## 剩余本地采样路线核查

Droid [公开 BYOK](https://docs.factory.com/model-independence/byok) 支持本地模型，
但 [无账户 Airgap 路线](https://docs.factory.com/enterprise/airgapped-deployment)
是未公开分发的企业包；公开 headless 安装步骤仍要求 Factory API key。
目前只核验这项文档/发行边界，未运行真实调用，不把公开 BYOK 支持扩展为免账户能力。
Roo 官方文档已公告 2026-05-15 关闭扩展；下节已单独核验其官方历史 VSIX 与本地调用，
未以归档源码或 ZooCode 分支代替 Roo 真实样本。

其余路线已按 2026-10-06 官方文档复核，安装或支持 BYOK 本身不完成真实记录格式验收：

| 产品 | 官方依据与当前限制 |
| --- | --- |
| Amp | [Model Routing](https://ampcode.com/docs/customize/model-routing) 已支持 Custom URL 的多种协议，个人/工作区连接由账户维护；不能沿用“没有 BYOK”的旧假设，免账户公开本地路线仍未核验 |
| Qoder | [认证](https://docs.qoder.com/cli/authentication) 要求 Qoder 登录或 PAT；[Custom Models](https://docs.qoder.com/cli/custom-models) 的 Individual BYOK 通过账户可见目录/向导，明确禁止手工 settings.json 配置；未核验其用量记录 |
| Antigravity | [实际 CLI 安装/认证页](https://antigravity.google/docs/cli/install/) 说明 Google 登录或 Gemini API key，自定义端点须 Gemini 兼容；[SDK 本地模型](https://antigravity.google/docs/sdk/local-models/) 另有免账户 OpenAI 路线，但不能据此核验当前 IDE protobuf 格式，需逐面单独核验 |
| Zed | [官方模型提供商](https://zed.dev/docs/ai/llm-providers) 的本地模型与当时只核验 hosted zed.dev 的格式不同；当时的空库不作样本，本地 BYOK 也不能核验 hosted 行为。后续 2026-10-07 的指定外部 Provider 原生验收见 [后续记录](plan-20261007.md) |
| Kiro | [实际认证页](https://kiro.dev/docs/getting-started/authentication/) 的公开账户/企业路线仍需相应凭据；未找到已核验的免账户本地规则，不构造账户状态 |
| Grok Build | x.ai/grok-build 当前读取失败，仅能记录该失败；尚未核验官方分发物和本地规则，不用同名第三方 CLI 替代 |

上述只有 Command Code 有实际无账户 headless 被拒绝的结果；其他限制的文档资料与实际运行结果分开。

## Roo 3.54.0

[官方发布](https://github.com/RooCodeInc/Roo-Code/releases/tag/v3.54.0) 对应 commit
27001b2b5aa47b65e8a6ba1914e0f4216be0ebb0，发布于 2026-05-15T17:52:24Z。
roo-cline-3.54.0.vsix 为 30,837,353 字节，SHA-256 与发布 asset digest 一致：
615b7e30ab456c51fe2e1f413a6e05fe4bd370f2350e14de574ec92e25366b4b。
实际 manifest 为 RooVeterinaryInc.roo-cline、3.54.0、./dist/extension.js、VS Code ^1.84.0。
[官方公告](https://roocodeinc.github.io/Roo-Code/) 的关闭日期不作为历史 BYOK 无法运行的依据。

Microsoft 官方 Linux VS Code 1.140.0（07f806f999227108933c2e30515b26eecc1fda74）
tar 为 348,920,118 字节，SHA-256 d32031e9e213d59532af3cf32fcb8b357a1cdd10417967b4f5b5ba30436dc0dc，
与官方版本元数据一致。两份归档均先检查成员数量、解包总量、路径及链接目标边界。
独立镜像 ID 6ed04799198831354cad3bde113717d908f3f93c3420b0ff89f1bba9f6377ddd，
仅补实际 ldd 缺失的 libnspr4/libnss3；执行阶段无网络、无宿主挂载，普通用户、
独立 Xvfb/D-Bus、原生 Electron GUI。--no-sandbox/--disable-gpu 是该验收环境标志。

依照固定版本 src/extension/api.ts 和官方 vscode-e2e，激活未修改扩展，等待 isReady，
调用公开 setConfiguration/startNewTask/cancelCurrentTask，并观察 taskTokenUsageUpdated。
provider=openai、base URL 为容器 loopback、本地模型 qwen2.5-0.5b-local，最多输出 16 token；
关闭 MCP/工具自动批准、checkpoint/自动压缩，独立用户数据/扩展目录，VS Code 遥测及更新关闭。
未配置 API key；官方 handler 为 SDK 自行提供 not-provided 默认占位，验收没有写入账户凭据。
扩展自行请求 include_usage=true，代理不改请求或真实 SSE，仅保存选定的用量字段。

| 场景 | 真实模型 API | 原生记录与扩展公开统计 |
| --- | --- | --- |
| 默认请求上限（roo-real-1791279141） | 三次：6,772/2/6,774、7,095/2/7,097、7,418/2/7,420；合计 21,285/6/21,291，嵌套缓存读合计 13,869 | 两个写入文件 api_req_started；13,867/4/13,871；第三次在公开取消时移除，覆盖缺口保留 |
| 请求上限 2（roo-real-1791279262） | 一次：6,772/2/6,774、缓存读 0 | 一次 6,772/2/6,774，一致；下一占位请求受公开上限阻止，取消后移除 |

模型回复 OK，但未发 completion 工具，两项均使用公开 API 取消，不能核验任务成功完成。
默认记录 767 字节，SHA-256 b9c8ca855050f224281ec50038efa5f5aa4674e0726cce9a98b8c09204862238；
上限对照 556 字节，SHA-256 f5fa87fcc6bd6bccdcc529aec41a737d35f5ae81252aff08d81c124eedb5e2cc。
两个原始记录及 API/扩展统计均已按准备要求脱敏保留于 fixtures/roo/real-3.54.0，
保留真实时间、桶值、记录顺序和类型，移除正文、请求文本、任务标识及个人路径。

固定 Task.ts/cost.ts 将缓存桶及费用初始化为零；OpenAiHandler.processUsageMetrics 只读取
顶层 cache_read_input_tokens/cache_creation_input_tokens，忽略本次实际正数的
prompt_tokens_details.cached_tokens。因此原生零不能证明实际零缓存或免费调用。
roo-ui-messages-doc2 保留正桶及已报告输入/输出，零缓存/费用为未知；不补未缓存输入，
仅已知输入总量与输出总量派生完整 total。空占位不计调用，有显式零用量标记则保留调用，
model/provider 未写入文件仍未知，condense 正费用仍为 Estimated。

旧库升级仅接受 roo-ui-messages-doc1 的完整旧 canonical/legacy 摘要匹配，最多 32 种
旧零桶/零费用候选，原旧派生未缓存/总量按旧算法恢复后比对；其他 token、质量、模型、
归属、时间、修订、身份变化仍冲突处理。保留事件键、首次观察时间和诊断/真实冲突历史，
事件/游标/指纹与未封存汇总同事务；APPDATA 默认发现可独立于 HOME，验收不读取本机其他来源。

roo_contract 五项及 M8 19 项退出 0：真实整注册表发现/API 对照/重复读取、旧完整与
legacy 摘要/已消费未变游标/并行/事务回滚、受保护字段及同批真实冲突、零标记与纯占位、
坏行与其他有效记录/正估算费用。旧包到当前 deb 真实往返 roo-readback-1791280179 退出 0：
两个源共三个写入文件调用，输入 20,639/输出 6/总量 20,645；服务四次的缺口不补。
三个规则升级、冲突 0、checkpoint 2、health=ok，来源字节、事件键、原观察时间与修订不变，
一条仍在保留期内的历史诊断保持；重扫不新增诊断。

首失败均保留：VS Code 下载上界小于官方实际大小，核验元数据后提高明确上界；
缺 NSS 库及官方 WSL 启动提示先解决；启动包装脚本提前退出没有产生 API/记录，
改用官方测试使用的原生 Electron 并强制核对结果；请求上限 1 因官方计数包含已发占位
而没有实际请求，不作为样本。初次测试数据检查触及其他默认来源和误解既有质量分区，
随后分别修正来源隔离和规则断言，未修改质量分区产品行为。回读首次脚本语法、来源
format 过滤错误和超保留期的历史 marker 断言均单独保留，最终使用 agent 与真实保留期核对。
官方扩展的 ripgrep 定位告警、离线模型目录下载失败保留；未测试工具、其他版本或云端。

<a id="ten-source-package-rechecks"></a>

## 十源阶段成品复验

本节为 Roo 修改后的当前成品；以上五/六/八/九源包和日志继续保留历史范围。
npm run verify 退出 0：Rust 961（核心 860、应用 101）、前端 21、脚本 4、Markdown 192，
Svelte 无错误/告警、fmt、Clippy -D warnings、构建通过。Debian workspace Rust 958
（核心 860、应用 98，默认忽略 9），M8 真实来源专项共 34 项。

| 当前成品 | 字节 | SHA-256 |
| --- | --- | --- |
| Windows release | 10,008,064 | d56c2ba2580dcd870ca05f5e190285bbd090c76a7588276bef1c7c87d8753e49 |
| Windows NSIS | 3,949,669 | 16de6498284e3830a29b99552f9a8b80fec7589a603dd0aa4f0eb099d5e3ecf1 |
| Debian deb | 5,476,298 | a7a607943723547e10c8e2e05cf42b176f38e55e76ab35e9ae0fc39d2d5e76e6 |
| AppImage | 111,151,608 | b9decd976bb69068e3d2cff539b25252443895501bff7f592f5e52382a90eb11 |

NSIS 12 项 root 1791280147976、headless 11 项 root 1791280229639、receiver 8 项
root 1791280228175（自有凭据 0）、Windows 原生 17 项 root 1791280276230 全部退出 0；
20 次首屏 P95 753.6 ms。Linux 9 组/47 项 root 1791280082989 退出 0，实际 deb 往返、
AppImage 只读 FUSE/退出释放、GTK/IPC/Orca 五页与品牌语音通过；普通用户 CapEff=0、
默认 seccomp、无网络/宿主挂载，FUSE 仅增加自有容器 SYS_ADMIN，未核验物理音频。

| 当前 deb 原始记录回读/重复扫描 | 实际 root |
| --- | --- |
| Aider 1 次/95 输入/3 输出/98 总量 | aider-readback-1791280325 |
| Goose 1 次/320/2/322，缓存读 0，写/费用未知 | goose-readback-1791280328 |
| Continue 累计 1,471/2，旧缓存零改未知、修订不变 | continue-readback-1791280273 |
| jcode 1 次/460/2，完整总量未知 | jcode-readback-1791280329 |
| gajae-code 1 次/412/2/414，旧完整摘要修正 | gajae-readback-1791280275 |
| AtomCode 6,176/2/6,178、报告调用 1，区间不确定，旧源健康恢复 | atomcode-readback-1791280273 |
| Crush 仅 Estimated 5,059 微美元，人工费率不能核验账单 | crush-readback-1791280327 |
| Junie 七条非缓存 10,972/缓存读 53,102/输出 98，旧键/诊断/观察时间不变 | junie-readback-1791280229 |
| Xum 默认全未知与对照非缓存 13,721/文本下界 2，两个旧摘要升级 | xum-readback-1791280229 |
| Roo 三个写入文件调用/20,645 总量，缓存/费用未知，三个旧摘要升级 | roo-readback-1791280179 |

测试数据 46 文件/125 JSON 对象检查通过，秘密键与个人路径 0；原始记录字节均不变，
旧诊断/冲突不被规则升级清除。当前结果为本地未发布成品，未核验远端 CI 或正式发布。
