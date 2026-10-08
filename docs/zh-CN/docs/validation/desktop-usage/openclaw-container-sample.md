# OpenClaw 真实容器样本与运行库读取

<a id="openclaw-native-container-sample-and-runtime-database"></a>

日期 2026-10-06；0.2.1 未提交工作树。官方 OpenClaw 2026.9.8 CLI 在独立
rootless Podman 内通过公开 `agent --local` 接口调用真实本地模型，两次退出 0。
schema 24 hot transcript 只读适配及全注册表发现已实施，原库升级、Windows
成品双读和包生命周期通过。其他协议、冷归档和 gateway 路径仍单独保留。
行为见 [运行库规则](../../design/desktop-usage/openclaw-runtime.md)。

<a id="distribution-and-references"></a>

## 分发物与依据

- 官方 [npm 2026.9.8 元数据](https://registry.npmjs.org/openclaw/2026.9.8)
  与 [tarball](https://registry.npmjs.org/openclaw/-/openclaw-2026.9.8.tgz)；
  dist SHA1 `7246c389c7134d9c15622082772912c7082f68af`，13,320 文件、
  unpacked 388,955,824 bytes。npm integrity 为
  `sha512-G+JkNUhtpDE3cXR4AEi2NyyG9fqI/T2WUSl8ZnR8AATH8Dh1kC3qYFL7wwPoZtgHiP/cszA86PEiE0PDysxb9Q==`。
- [GitHub v2026.9.8](https://github.com/openclaw/openclaw/releases/tag/v2026.9.8)
  于 2026-10-03 发布，tag commit 为
  `fc23bc864e4553c2d215e479eeec47b67a0bf943`；实际 CLI 显示
  `OpenClaw 2026.9.8 (fc23bc8)`。
- [npm 公开 provenance](https://registry.npmjs.org/-/npm/v1/attestations/openclaw@2026.9.8)
  声明的构建 commit 为 `aa6008ad198ef99c43f9d89dbd01694708712974`，
  workflow 为 `.github/workflows/openclaw-npm-release.yml`，
  [构建 run](https://github.com/openclaw/openclaw/actions/runs/37087759921/attempts/1)。
  已解码声明并核对 subject SHA512 与 dist integrity；未独立验证签名或透明日志信任链。
- 实际分发物 `dist/package-update-activation-recovery.mjs` SHA256
  `e08dfc1fb3ba7962f9e01d7a6770117c6cd77406b3fa4f7ba94788a670030ba0`。
  其中 OpenAI normalizer 与 fc23 语义一致，没有 aa6008 的 `contextUsage`。
  因而依据实际安装代码、schema 与原始响应读取，不能把声明 commit 的新字段
  套到分发物；库内可变 app_version 也不能识别历史行的版本。

WSL Debian、Node 24.21.0；官方 engines 要求 `>=24.16.0 <25 || >=26.1.0`。
镜像 `localhost/llm-usage-openclaw:2026.9.8`，image ID
`4ba2f82099166a8ac7bcf60b4c60c1488786bf1961b063f86957ec04e84b9bcb`。
首次 `--omit=optional` 安装因 Koffi/CMake 退出 1；保留日志，安装 cmake/
build-essential 并恢复 optional 后安装退出 0。npm 的未批准安装脚本提示保留，
没有通过测试绕过批准。客户端源代码和分发物未修改。

<a id="real-runs-and-independent-comparison"></a>

## 真实运行与独立核对

成功根为 WSL `build/plan-final-push/openclaw-real-1791288304`。普通 UID 1000，
CapEff=0、默认 seccomp、无网络/宿主挂载；自有容器已回收。
自定义 OpenAI-compatible provider 仅连容器内回环模型服务，公开 local marker
`ollama-local` 不含个人凭据。配置校验先退出 0；tools 全禁用、thinking off、
telemetry 关闭，无 gateway/delivery 或宿主 IDE 修改。

两次使用同一自有 session 的公开命令：

```sh
openclaw agent --local --agent main --session-id <owned-session-id> \
  --model local-llama/qwen3.5-0.8b-local \
  --message 'Reply with the word OK. Do not call tools or read or change files.' \
  --thinking off --json --timeout 120
```

模型沿用 [Hermes 实测模型](hermes-container-sample.md)：官方 Qwen3.5-0.8B BF16
GGUF，SHA256 `9a7bed4041b7975e0f71fa34670d1e9025213bc92905ac0db75d36c4fa3fa623`；
llama-server 单 slot、context 64,000、8 线程、最大输出 128。透明代理只记录真正
SSE usage 并原样转发；客户端本身请求 include_usage，没有构造响应或用量。

| 轮次 | API 完整 prompt | API completion | API cache read | 原生 input | 原生 output | 原生 cacheRead |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 6,105 | 47 | 0 | 6,105 | 47 | 0 |
| 2 | 6,153 | 107 | 6,101 | 52 | 107 | 6,101 |
| 合计 | 12,258 | 154 | 6,101 | 6,157 | 154 | 已知小计 6,101 |

原生 `input` 是减去缓存后的桶；cacheWrite 默认零，totalTokens 是客户端计算值，
cost 初始化零。应用保留两条 usage observation、未缓存输入 6,157、输出 154、
已知缓存读 6,101；完整输入/总量/来源总量、缓存写、推理、费用和底层调用数未知。
API 确实有两次请求，仍不从两条 observation 推断一般调用语义。
发生时间来自 message.timestamp 的请求开始时间，不能用 DB 写入时间替代。

原生库 `agents/main/agent/openclaw-agent.sqlite` 为 733,184 bytes，SHA256
`446b41b0eb2bbb105e553a5b0670c757f0d9ce8f86fa70af5e25afe94070b42e`，
user_version 24、primary role agent、owner main。10 条 transcript events 中
seq 5/8 的 assistant 有用量；session_entry_provenance=1、acp_owned=0，
plugin/hook 为空、harness openclaw、所属 session_key 与 agent 一致。
global store schema 19、quarantine schema 2 和辅助文件不作为用量来源。
本样本没有冷归档，未核验冷归档读取。

首次根 `1791288181` 使用最大输出 16，HTTP 200 且产生 6,107/16/6,123 的真实
用量，但 CLI 因 length/incomplete_turn 退出 1。原始错误保留；增大输出上限，
换独立容器后两轮均退出 0。没有把失败的任务当作零用量或成功任务。

<a id="implementation-and-regressions"></a>

## 实施与回归

修复真实路径 `agents/<agent>/agent` 发现；环境优先级与六种手工根均经过完整
注册表核对。同一物理文件只登记一次，活 WAL 下重新检查旧 seq，不依赖文件
字节指纹认定没有变化。只读事务同时核对 schema/provenance、冷归档清单和
hot rows；有界分页/取消、TEXT/zstd 4 MiB 上限及坏行隔离，游标/事件同事务。
其他 schema、外部 CLI 镜像、迁移来源、plugin/hook/acp、其他协议继续隔离。
冷归档缺口可见，消失 hot rows 不删除已采历史。

`openclaw_runtime_contract` 11 项及旧格式测试 3 项通过：限定字段的真实记录/API、
发现与环境优先级、未知 schema/owner、来源隔离、坏桶/计算矛盾、zstd/坏类型/
有界读取、分页/WAL/可变库版本、并行和事务回滚、冷归档缺口/历史保留、默认
五桶零未知、重复身份与真实冲突。zstd/cold 等边界样本明确为合成，不能扩大
真实来源的验收范围。首次专项误用并行 API 的编译失败、完整检查发现 MSRV 不支持
`Option.is_none_or` 均保留；按真实接口及 MSRV 1.77.2 修正后全部通过。

脱敏测试数据位于 `core/tests/fixtures/openclaw/real-2026.9.8`；仅限定字段，
四条原生 user/assistant 记录、API 用量与来源说明，ID 替换。正文、凭据、
配置和原库不提交。projection SHA256
`68d4db4b60b40b817fd09f853d148ee4299a1506e1c01e8644ec1396e0492d69`，
API SHA256 `8edf09c8e6b699385d1466a796ead81511766702d5b5490497826dec236f4a6a`。

<a id="current-packaged-build-checks"></a>

## 当前成品验收

以下均退出 0，日志/失败记录在仓库根 build/plan-final-push：

- Windows `npm run verify`：Rust 988（核心 887、应用 101，默认忽略 8）、前端
  21、脚本 4；当时 Markdown 197 文件，Svelte 无错误/告警，fmt、Clippy
  `-D warnings`、前端构建通过。
- Debian workspace：95 组、985 通过（核心 887、应用 98，默认忽略 9）；
  Clippy、release/deb/AppImage 构建通过。
- Windows NSIS 12 项（`1791289684773`）、无界面 11 项（`1791289665186`）、
  原生 WebView2/IPC 17 项（`1791289782343`）、接收器 8 项（`1791290098384`）；
  20 次首屏 P95 752.4 ms，自有凭据及安装集成残留 0。
- Linux 根 `1791289583346`：9 组/47 项，deb 往返、只读 FUSE 挂载/GTK GUI/
  退出释放、[Orca 十语言五页](orca-multilang.md)的当次焦点/语音均通过。
  ALSA null 未核验物理可听性，容器未核验宿主登录/注销和完整系统集成。

| 成品 | bytes | SHA256 |
| --- | --- | --- |
| Windows exe | 10,046,976 | `58c7c7cf1484f55645f11c2ff0cc780bb50fb340ae06115ec87442e6732362d3` |
| Windows NSIS | 3,961,563 | `f9b43e75ecf67ff92374410b8413aafa38b8a7cceb9a9951395ef512e1d43a73` |
| Linux deb | 5,497,396 | `099676397117791d99c44be53057ded1be44f5808822c7df8dc579a3b415c357` |
| Linux AppImage | 111,163,896 | `c10013873e31457a6984fc1366b039794359d3838b648660156e7d2679412b5a` |

`openclaw-readback-1791289585` 将前阶段 Cline deb（SHA256
`74cb3d8b8d99ac4cdebd47b3c29afd4782c5e909b13eec5c492180fb1f151d4a`）的旧库
升级到当前 deb；旧包两次读取无事件/有格式诊断，新包得到两条 observation、
一个实例/物理文件/checkpoint，健康 ok/latest_fallback，重扫不重复计数且数值如上。
主库及持久源文件 SHA256 不变；SQLite 只读也可能更新 SHM bookkeeping，
因此未把共享内存 sidecar 纳入“不变”断言。
`openclaw-windows-1791289662154` 用实际 Windows exe 双读限定字段重建的 SQLite，
得到相同值和未知边界；它验证 Windows 应用读取，未验证 Windows OpenClaw CLI。
完整测试数据字段审计 57 文件/135 JSON 对象，凭据键与私人路径均零。

同包回读 M8 十源及 Hermes/Cline/Qwen/OpenCode 全部退出 0，数值/未知边界和
原有覆盖缺口保持：Aider/jcode/Continue 根 `1791290118`，AtomCode `1791290116`，
gajae-code `1791290117`，Crush/Hermes/Xum `1791290119`，Goose/Roo/Cline
`1791290120`，Junie `1791290121`；产品前缀保证同秒根互不混用。
OpenCode 默认/对照根 `1791290118`/`1791290135`；Qwen SDK 主/后台、原生先读
与封存副本择一仍为 2 次/16,423 token，缓存读 3。复验未扩大其他产品能力。
