# Cline VS Code 4.1.22 真实 SDK 样本

<a id="cline-vs-code-4122-native-sdk-sample"></a>

官方原版 VSIX 在隔离 VS Code GUI 中经公开 `startNewTask` 调用本地模型。
原生 JSON 仅提取 envelope、id、role、ts、modelInfo、metrics 和已知 metadata；
会话/消息 ID 脱敏，正文与配置没有保存。三个原生 metrics 与独立 API 逐条一致：
输入 8,922、输出 48、总量 8,970；正缓存读合计 5,881。
默认零缓存写/首条缓存读未知，不派生未缓存输入；metrics 可合并 run/重试，
应用记三个 observation，调用数未知。会话版本 metadata 不能据此确认历史消息，
仍 latest_fallback。任务完成和旧 UI 格式真实验收不由本样本确认。

原文件/官方分发物/模型摘要、环境和边界见 provenance.json；独立响应见
api-usage.json，详细执行记录见
[Cline 记录](../../../../../../../../docs/validation/desktop-usage/cline-container-sample.md)。
