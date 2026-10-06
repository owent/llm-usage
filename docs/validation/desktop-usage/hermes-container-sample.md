# Hermes 真实容器样本与统计修正

2026-10-06 在 WSL Debian 的独立 rootless Podman 中运行官方 Hermes Agent
0.21.5，公开 CLI 与 `--resume latest` 两次调用真实本地模型，均退出 0。
原生数据库、CLI 报告与模型服务的独立用量核对确认：`input_tokens` 为未缓存输入，
缺字段/初始化零不能当作已报告零。解析器及旧库重放已修正，本阶段成品验收见文末。
后续 [OpenClaw 阶段](openclaw-container-sample.md) 新包已再次通过 Hermes 原库升级/重扫，
下述摘要和计数保留本阶段范围。

## 固定来源与运行环境

- [官方发布 v2026.9.24](https://github.com/NousResearch/hermes-agent/releases/tag/v2026.9.24)，
  实际版本 0.21.5，commit `f97608f178d1ffeca59860195ab7da295f7c8e5f`。
- 官方 `docker.io/nousresearch/hermes-agent` amd64 摘要
  `sha256:2fd023efbb8d3d2b0ce1a73d028b07370cff34f567cfe0e999553e8c327ea283`；
  OCI revision、镜像内 provenance 与实际版本命令一致，官方客户端未改动。
- [自定义供应商](https://hermes-agent.nousresearch.com/docs/integrations/providers/) 与
  [本地模型](https://hermes-agent.nousresearch.com/docs/user-guide/local-models/) 的公开
  custom/base_url 路线；没有提供账户凭据。CLI SDK 内部 keyless 占位不作为真实凭据。
- [Qwen 原模型](https://huggingface.co/Qwen/Qwen3.5-0.8B)，固定 revision
  `2fc06364715b967f1860aea9cf38778875588b17`，原生上下文 262144；
  [ggml-org GGUF](https://huggingface.co/ggml-org/Qwen3.5-0.8B-GGUF/tree/8fea620810c4afa23dd6443f999a48574c1611a3)
  的 BF16 文件 1,557,662,496 字节，SHA256
  `9a7bed4041b7975e0f71fa34670d1e9025213bc92905ac0db75d36c4fa3fa623`，与官方 LFS 摘要一致。
  llama-server 实际日志 `n_slots=1, n_ctx_slot=64000`，输出有界；使用真实模型推理，不构造响应或用量。
- 模型容器 `network=none`；客户端仅共享该容器 loopback。无宿主挂载、无个人配置，
  CLI 用户 UID 10000、CapEff=0、默认 seccomp=2；空独立工作目录，CLI toolsets 为空，
  memory/compression/title generation 关闭。每次外部命令期限 180 秒；仅清理自有容器。
- 透明代理只记录白名单模型/状态/流选项/usage，请求与响应原样转发；正文不入脱敏 fixture。

## 三方用量核对

| 场景 | API 总输入 | API 缓存读 | CLI 未缓存输入 | 输出 | API/CLI 总量 | 来源调用数 |
| --- | --- | --- | --- | --- | --- | --- |
| 第一次 CLI | 816 | 0 | 816 | 2 | 818 | 1 |
| 公开 resume | 845 | 812 | 33 | 2 | 847 | 1 |
| 合计 | 1661 | 812 | 849 | 4 | 1665 | 2 |

原生 `session_model_usage` 只有一条主任务累计行：未缓存输入 849、缓存读 812、
输出 4、api_call_count=2；缓存写/推理均零。first_seen=1791281502.5680463、
last_seen=1791281505.302139 是聚合写入边界，不能拆成逐次调用时间。
完整原库 270,336 字节，SHA256
`cfc28012eb3ff398b865a6337d5eadb814f0d5f321cd0a5fd7b6dafd9ae8d32d`。

[脱敏 fixture](../../../desktop/src-tauri/crates/core/tests/fixtures/hermes/real-0.21.5/provenance.json)
保存原生 DDL、白名单时间/计数/路由字段；会话身份替换，排除消息、FTS、正文、配置、路径和凭据。
只读连接提取前后原库摘要一致。projection、API 与 CLI 的字节数/摘要分别记录于 provenance。

## 修正合同及真实范围

[固定归一代码](https://github.com/NousResearch/hermes-agent/blob/f97608f178d1ffeca59860195ab7da295f7c8e5f/agent/usage_pricing.py)
与原 A24 commit `ef70b3661cbfcf57e583008ad91dd04d8ba46070` 一致：
OpenAI prompt 扣缓存读/写后保存未缓存桶；reasoning 为 output 子集。缺字段及初始化
默认归零，存储没有字段有效性标记。因此正数桶 reported、零未知；全部必需桶已知
才派生输入/完整总量，checked_add 溢出保持未知。零调用数亦未知。

真实应用应保留未缓存 849、缓存读 812、输出 4、来源调用汇总 2；输入总量、
完整总量、缓存写和推理保持未知。API 完整数值只用于独立对照，不能补入原生来源。
仍是 interval aggregate，日汇总及事件数不生成；当前数据层没有模型/费用维度。

解析器 `hermes-session-model-usage-2` 沿真实发现路径重放旧处理位置。只有重构完整旧摘要
匹配才能修正旧 input_total 归属与默认零；稳定身份、首次入库时间、来源修订保留，
正数、质量、区间、覆盖或调用数变化仍仲裁。事件/聚合/游标同事务，失败可重放。
坏类型行独立诊断，其他有效累计行仍可兼容入库。

实际库 schema=30；[在线存储文档](https://hermes-agent.nousresearch.com/docs/developer-guide/session-storage/)
已演进到其他 schema，不能套用到固定版本。整库迁移版本没有逐行客户端版本，
注册表继续为空/latest_fallback，不认证其他历史会话。gateway、辅助、压缩、子 Agent、
混合模型/版本、v20 历史回填与绝对写入仍未真实验收，合成回归边界保留。

## 验证及首次失败

- 初次 32K 模型在客户端最低 64000 上下文检查处退出 1，没有 HTTP 调用、没有用量行；
  `hermes-real-1791281141` 保留证据。随后更换原生支持足够上下文的真实模型，未绕过检查。
- 单调用 `hermes-real-1791281409` 退出 0；最终双调用根
  `hermes-real-1791281489` 的两条公开 CLI 命令均退出 0。
- 首轮合同测试 13 通过/1 失败，原因是新测试误用了不存在的应用数据库列
  `detection_json`；依据实际 `format_status` 修正后，Hermes 合同 14 项全部通过。
  其中新增 5 项覆盖真实全注册表/重扫、旧处理位置/并行/事务回滚、受保护字段冲突、
  坏类型/负值行隔离及默认零。另增映射默认零/溢出单元回归。
- 首轮完整验证在 Clippy 编译单元测试处退出 101：TokenQuality 没有 PartialEq，
  新增测试的整对象相等断言不可编译；改为逐字段验证 unknown，不改变领域类型。
- 后续完整回归发现仅检查“有汇总”放宽了 Codex 无逐次载体的拒绝行为。
  收窄为存在经过校验的 exclusive 累计行；duplicate/overlap_unknown 对账快照仍不能
  认证兼容。保留 Codex 原有拒绝回归与 Hermes 有效/坏行混合场景回归。
- 完整验证退出 0：Windows Rust 967（核心 866、应用 101；忽略 8）、前端 21、
  脚本 4、Markdown 193；Svelte 无错误/告警、fmt/Clippy -D warnings/构建通过。
  Debian Rust 964（核心 866、应用 98；忽略 9）及 deb/AppImage 构建退出 0。
- `hermes-readback-1791283098`：新包安装真实未改原库；旧包已消费位置改为超过
  样本时间后升级，新包自动全量重放。未缓存 849、缓存读 812、输出 4、来源调用 2，
  未知分项保持未知；旧稳定身份/区间/修订/首次入库保留，规则更新 1、真实冲突 0、
  旧审计历史 1、游标 1，二次扫描幂等、原库字节不变、health=ok、latest_fallback。
- 当前 Windows NSIS 12 项退出 0（根 `1791283166963`）、无界面 11 项/3 事件/75 token
  退出 0（`1791283234990`）、接收 8 项/自有凭据 0 退出 0（`1791283236566`）；
  原生桌面 17 项、20 次首屏 P95 745.6 ms 退出 0（`1791283243856`）。
- 当前 Linux deb 生命周期/FUSE/GTK/WebKit/Orca 9 组、47 项退出 0
  （`1791283121749`），五页导航/六个焦点名称/实际语音及 FUSE 退出释放保留证据；
  与前阶段同样的 rootless、普通 GUI 用户、默认 seccomp、无外网/宿主挂载边界。
- 当前 deb 回读十个 M8 原始载体/重扫及 Continue/gajae-code/AtomCode/Junie/Xum/Roo
  旧库升级均退出 0，Qwen 原生/SDK/封存副本分区与 OpenCode 默认/对照也通过。
  原数值/覆盖缺口不变，不扩展采样产品场景。回读根分别为 Continue 1791283134、
  AtomCode 1791283136、gajae-code/Roo 1791283138、Junie 1791283179、Goose/Crush 1791283180、
  Xum 1791283182、jcode 1791283211、Aider 1791283212；OpenCode 默认/对照为
  1791283211/1791283217，Qwen 命令日志独立保留。
- 脱敏审计 50 文件/129 JSON 对象，凭据键与私人路径 0；受影响 Markdown 本地链接
  校验通过。Skill 静态校验默认 GBK 首次失败，使用 Python UTF-8 模式后通过，
  description 未改变，不认证模型路由评估。全部临时命令、日志、原始库及首轮失败
  保留于根 `build/plan-final-push/`。

## 本阶段受测包

| 成品 | 字节 | SHA256 |
| --- | --- | --- |
| Windows release | 10008064 | 0496e86a003823df11da72def5e63b8041baf35f5c7b9d7d1c9c646ccea7275f |
| Windows NSIS 0.2.1 | 3951219 | f96e564e2128df12fae896baa48b01540c37715eed382e96a57912fae83358b5 |
| Debian 0.2.1 | 5477020 | 2ba79f090ef269aa48613f395278ade570fc1df6382e0437608e96554669cbb8 |
| AppImage 0.2.1 | 111151608 | a75bf3bc27d9518c750c79e005de9e7befdd745f24297d827111c68a3b9d80ba |

旧 Hermes 对照为前阶段 deb `a7a607943723547e10c8e2e05cf42b176f38e55e76ab35e9ae0fc39d2d5e76e6`；
Windows/Linux 生命周期各用已保存的 0.2.0 包。工作树未提交，受测包不等于已发布制品或远端 CI。
