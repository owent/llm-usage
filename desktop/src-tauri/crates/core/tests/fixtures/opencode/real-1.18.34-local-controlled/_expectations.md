# OpenCode 1.18.34 明确标题对照

2026-10-05 官方 CLI、固定源码 aec0b9a6d8898f68f923aaf08b7306d931fd9d76，独立
离线 rootless Podman、本地 CPU 模型与新 HOME。相对于默认样本，仅使用实际
`run --title` 显式设标题；[官方 ensureTitle](https://github.com/anomalyco/opencode/blob/aec0b9a6d8898f68f923aaf08b7306d931fd9d76/packages/opencode/src/session/prompt.ts)
对非默认标题跳过标题生成，实际只观测一条 API 用量。

API input 298、output 1、total 299、cache read 0；CLI / part / assistant message /
session 均为 input 298、output 1、reasoning 0、cache read/write 0、total 299。
应用一条事件、input_total 298、total 299；重扫事件/修订/汇总不变。
三表 DDL 原样保留，用量字段未改，正文删除，ID/路径/标题匿名化及同日平移时间。

这项对照不取消默认标题覆盖缺口，不认证云端/其他版本、非零费用、缓存写或推理桶。
版本仍为 latest_fallback，原因及整个采集流程见
[容器记录](../../../../../../../../docs/validation/desktop-usage/container-sources.md)。
