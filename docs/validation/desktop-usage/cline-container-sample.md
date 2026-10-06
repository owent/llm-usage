# Cline VS Code SDK 容器来源验收

2026-10-06；[计划](../../../Plan.md)、[数据合同](../../design/desktop-usage/data-contract.md)、
[最新成品验收](current-acceptance.md)。本记录只认证实际运行及其原生字段，
不替旧 `ui_messages.json`、CLI、desktop sidecar 或其他供应商完成真实验收。
后续 [OpenClaw 阶段](openclaw-container-sample.md) 新包已再次通过 Cline 原始
SDK 载体旧库升级/重扫，以下制品摘要及计数保留本阶段范围。

## 官方分发与源码

[官方 v4.1.22](https://github.com/cline/cline/releases/tag/v4.1.22) 发布于
2026-09-30，固定提交 `f58bc118bdeef1bd2813cd08e00d98bdcda96475`。
原版 VSIX 9,072,144 bytes，发布 manifest 与下载 SHA256 一致：
`134b54af94e1e4cc6cd07224a61f6873c40c845d9fba1f9e6aa510dd6e2c5382`。
包 manifest：`saoudrizwan.claude-dev`、4.1.22、`dist/extension.js`、
VS Code `^1.101.0`。最新 desktop release 是另一产品面，未拿它认证扩展版本。

固定源码逐项核对：

- [原生 writer](https://github.com/cline/cline/blob/f58bc118bdeef1bd2813cd08e00d98bdcda96475/sdk/packages/core/src/services/session-data.ts)：
  schema 1，只有末条 assistant 带 metrics；缺逐 turn metrics 时可回填整个 run 用量，
  `ts` 可回退 run endedAt；不把 metrics 条数推导成底层请求数。
- [codec](https://github.com/cline/cline/blob/f58bc118bdeef1bd2813cd08e00d98bdcda96475/sdk/packages/core/src/runtime/config/agent-message-codec.ts)：
  四个 token 字段均可能用默认零；正值保留，零不能当已报告缓存缺失。
- [origin metadata](https://github.com/cline/cline/blob/f58bc118bdeef1bd2813cd08e00d98bdcda96475/sdk/packages/core/src/session/history-origin.ts)：
  版本属于可重写会话 metadata，不能认证混合历史的逐消息版本。
- [路径](https://github.com/cline/cline/blob/f58bc118bdeef1bd2813cd08e00d98bdcda96475/sdk/packages/shared/src/storage/paths.ts)：
  session 目录按 CLINE_SESSION_DATA_DIR、CLINE_DATA_DIR、CLINE_DIR/default home 解析。
- [UI translator](https://github.com/cline/cline/blob/f58bc118bdeef1bd2813cd08e00d98bdcda96475/apps/vscode/src/sdk/message-translator.ts)：
  SDK input 含缓存，转换到旧 UI 时才扣缓存；不能复用旧 UI 四互斥桶映射。
- [旧会话转换](https://github.com/cline/cline/blob/f58bc118bdeef1bd2813cd08e00d98bdcda96475/apps/vscode/src/sdk/legacy-task-handling.ts)：
  转换只复制 role/content，不复制旧 UI 用量；旧会话实际恢复仍需另验。
- [重试用量合并](https://github.com/cline/cline/blob/f58bc118bdeef1bd2813cd08e00d98bdcda96475/sdk/packages/llms/src/providers/middleware/retry-empty-response.ts)：
  被丢弃尝试的 usage 可合并到最终 finish，不能将一条 metrics 当一个 API 请求。

## 实际环境与运行

WSL Debian、rootless Podman，复用已核验的 VS Code 1.140.0 GUI 依赖镜像，
仅复制官方 Cline VSIX 到独立 `/opt/cline`，扩展目录和 HOME/配置均独立。
容器无宿主挂载、`--network=none`、非 privileged、无新增能力；GUI/模型以
acceptance 用户运行，Xvfb + 独立 D-Bus。没有登录账户或使用个人配置。
公开扩展 API 只有 startNewTask 等任务操作；测试调用 activate + startNewTask，
没有修改扩展代码、调用私有 controller 或构造 native usage。

按[官方本地模型路线](https://docs.cline.bot/running-models-locally/overview)，
只写隔离 CLINE_DIR 的公开用户配置，选择 lmstudio provider 与 loopback `/v1`。
实际服务为 llama.cpp + 官方 Qwen3.5-0.8B BF16，未声称安装了 LM Studio 服务。
模型/服务来源与摘要沿用 [Hermes 记录](hermes-container-sample.md)：
GGUF SHA256 `9a7bed4041b7975e0f71fa34670d1e9025213bc92905ac0db75d36c4fa3fa623`，
原生 context 262,144；本次实际 server context 64,000、单 slot、每次输出上限 16。
代理只透传并记录 response usage；include_usage 是客户端自身请求。

自动批准全部关闭，plan 模式、telemetry/checkpoints/auto-condense/MCP 关闭。
legacy maxRequests 已被上游移除，配置里的 1 不算有效调用上限；测试由 210 秒
外部期限和实际模型输出上限约束。观察到三条 metrics 后测试宿主正常结束，
退出 0；不声称任务成功完成、停止 API 或正式 Cline cancel 流程通过。
owned 容器已 stop/remove，保留日志与源载体，未修改宿主 IDE。

## 独立核对与读取结果

根 `build/plan-final-push/cline-real-1791283911` 保存 extension-host、模型日志、
透明 API ledger、原生源及容器 inspect；`cline-inspect`、`cline-native-source`、
`cline-fixture` 分别核对环境、来源和提取摘要。

| 记录 | API/原生总输入 | API/原生输出 | API/原生缓存读 | 统一总量 |
| --- | --- | --- | --- | --- |
| 1 | 2,915 | 16 | 0（原生默认零未知） | 2,931 |
| 2 | 2,974 | 16 | 2,911 | 2,990 |
| 3 | 3,033 | 16 | 2,970 | 3,049 |
| 合计 | 8,922 | 48 | 已知小计 5,881 | 8,970 |

原生 messages 9,732 bytes，SHA256
`c1aaa3d3170f1c33a57ea99e98de840c380313689d4be03c916cf86524e5e53f`。
manifest 累计非缓存输入 3,041/输出 48/缓存读 5,881 与 SDK 原生 input 口径不同，
它不作为第二来源。sessions.db 仅 registry metadata，也不读取 token。
源逐条 modelInfo 为 qwen3.5-0.8b-local/lmstudio，缓存写默认零；成本字段缺失。

新增 `cline-sdk-messages-v1` 专用读取，SDK origin version 保持 latest_fallback，
三条 usage_observation，调用数未知；缓存写/未缓存/推理/费用未知。
空会话、非 VS Code 或 import/subagent/未知 schema 保持隔离。
旧 UI parser 独立保留。全注册表读取同一物理文件一次，重扫幂等、源字节不变。
脱敏 fixture 只保存白名单，ID 替换；正文/system prompt/配置/秘密/DB 未提交。

## 验证与首次失败

Windows Cline SDK 合同 10 项通过：真实 API 对照/全注册表、默认/环境优先级及
五种手工根、默认零/坏字段隔离、未知面与版本、displayOnly/无用量排除、
重复 ID/真实冲突/消失历史保留、缓存矛盾、半写恢复、checkpoint 回滚/并行重试，
以及 32 MiB 读取上限拒绝且不推进游标。
旧 Cline 合同 3 项保持通过。受影响规则、矩阵、研究与计划随实际成品验收同步。

首次 cargo helper 调用漏 manifest（退出 101）、专项首次编译的并行函数参数与
借用错误，以及 7 过/1 失败的错误诊断码断言，原日志均保留。
按实际 framework/ingest 源码修正测试为 `update_conflict` + conflict flag，
未放宽业务断言。最终专项退出 0。

## 当前成品验收

以下均退出 0，日志保留在根 build/plan-final-push：

- Windows `npm run verify`：Rust 977（核心 876、应用 101，默认忽略 8）、
  前端 21、脚本 4；Markdown 195 文件，Svelte 无错误/告警，fmt、Clippy
  `-D warnings` 与前端构建通过。
- Debian `cargo test --workspace --locked`：94 组、974 通过（核心 876、应用 98，
  默认忽略 9）；release/deb/AppImage 构建通过。
- Windows NSIS 12 项（根 `1791285323343`）、无界面 11 项（`1791285322853`）、
  原生 WebView2/IPC 17 项（`1791285395704`）和接收器 8 项（`1791285577694`）
  通过。20 次小数据首屏 P95 788.5 ms，自有凭据和安装集成残留 0。
- Debian 生命周期根 `1791285259847`：9 组/47 项，实际 deb 往返、只读 FUSE
  挂载/GTK GUI/退出释放及 Orca 五页键盘、AT-SPI 名称与语音输出通过。
  普通应用 UID 1000、CapEff=0、默认 seccomp、无网络/宿主挂载；ALSA null
  不认证物理可听性，容器不认证宿主注销或完整桌面集成。

| 成品 | bytes | SHA256 |
| --- | --- | --- |
| Windows exe | 10,024,960 | `9a58a275d376f176ee5f8a7bc35920bd758a21b11af294319fce13305ce4bc72` |
| Windows NSIS | 3,955,414 | `ad7b6d5d6b92defc7a568cab3f501f7ba2371021b4c7d6c9baa669dc40e00367` |
| Linux deb | 5,486,240 | `74cb3d8b8d99ac4cdebd47b3c29afd4782c5e909b13eec5c492180fb1f151d4a` |
| Linux AppImage | 111,155,704 | `41e4a09f7c69791aacbb98f17bf05e787d142c543523cf4a0d892a032c8ea625` |

`cline-readback-1791285253` 将前阶段 deb（SHA256
`2ba79f090ef269aa48613f395278ade570fc1df6382e0437608e96554669cbb8`）
的旧库升级到上表当前 deb：旧包两次扫描无 SDK 实例/事件，新包发现一个实例、
一个物理文件、一个 checkpoint 和三条 usage observation，重扫新增零。
8922/48/8970 与已知缓存读 5881 一致，健康为 ok/latest_fallback，原始源文件
SHA256 清单不变；版本回退不降级已确认用量的来源健康。

`cline-windows-1791285320382` 用当前实际 Windows exe 双读白名单真实 fixture，
同样得到三条 observation 和上述数值/未知边界，源字节未改。这认证 Windows
应用读取，并未把 Debian 扩展运行扩大成 Windows Cline 扩展运行。

本阶段同包重新读取十个 M8 产品及 Hermes/Qwen/OpenCode 的原样来源，全部
退出 0，保持数值和原有覆盖边界；产品名前缀使同秒根仍独立：Aider/jcode 为
`1791285412`、AtomCode `1791285408`、Continue `1791285412`、Crush/Goose/Junie/Xum
为 `1791285411`、gajae-code `1791285410`、Roo `1791285414`；Hermes 为
`hermes-readback-1791285322`，OpenCode 默认/对照为 `1791285327`/`1791285330`。
Qwen 的 SDK 主/后台与原生/封存分区回读仍 2 次/16,423 token、缓存读 3。
这些复验不关闭各产品未测场景。

未跟踪 fixture 白名单审计共 54 文件、132 JSON 对象，秘密键和私人路径均零。
首次本地链接检查的 sandbox `git EPERM` 单独保留，获得执行权限后发现并修正
fixture README 的相对路径层数；真实引用检查通过。上述工具错误与产品失败
分别记录，没有用旧阶段通过替代新增 SDK 的成品验收。
