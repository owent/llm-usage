# Qwen Code 0.25.0：关闭自动记忆的对照

与 [默认样本](../real-0.25.0-local-default/_expectations.md)使用同一官方客户端和本地模型，
只在容器内设置 memory.enableManagedAutoMemory=false 和 enableManagedAutoDream=false。
配置键与默认值先从实际安装包的 schema/配置加载器核验。JSONL 同为真实记录字段投影；
ID 匿名化、时间在同日平移，token 数值保持原样。

- 模型服务、CLI stats、原生 ChatRecord 和应用 SQLite 一致：1 次调用，prompt 8,903、
  output 2、cache read 0、total 8,905；工具调用 0。默认样本仍单独保留覆盖缺口。
- 版本/原始模型名保留；canonical/provider、缓存写入及 reasoning 仍未知。
- 二次扫描不新增、不更新，总计仍为 1 次 / 8,905。
- 不认证 Qwen 云端、其他版本、归档或缓存命中场景。
