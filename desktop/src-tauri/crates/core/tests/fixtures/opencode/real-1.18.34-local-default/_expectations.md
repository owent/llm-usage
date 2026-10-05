# OpenCode 1.18.34 默认标题场景

2026-10-05 官方 CLI 在独立 rootless Podman 中调用真实 CPU 本地模型，网络关闭、
新 HOME；固定源码 aec0b9a6d8898f68f923aaf08b7306d931fd9d76。并非合成 token，
过程与来源见 [容器记录](../../../../../../../../docs/validation/desktop-usage/container-sources.md)。
仅提取三张表的原始 DDL 与用量白名单；正文删除，ID/路径/标题匿名化，时间按同一
偏移移到 2026-10-05 UTC，token 不改。message 与 session 累计只用于对照，不叠加。

| 依据 | 调用 / 用量 |
| --- | --- |
| 真实主循环 API usage | input 298（含缓存读 3）、output 1、total 299 |
| CLI / part / assistant message / session | 未缓存 input 295、cache read 3、write 0、output 1、reasoning 0、total 299 |
| 应用 | 1 条、input_total 298、output_total 1、total 299；重扫事件/修订/汇总不变 |
| 默认标题 API usage | 另 1 条：input 539、output 10、total 549；未进入 step-finish 与会话累计 |

默认两次 API 合计 848，不能从主循环和累计对账 matched 声称全客户端覆盖。
不从差额补造标题事件，不由本地模型名推断云端模型或价目。费用 0 为客户端所写，
没有真实账单或非零费用依据。版本仍为 latest_fallback；逐记录版本及旧游标升级
验收未完成，不能以本样本认证同库其他会话。
