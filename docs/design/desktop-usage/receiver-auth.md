# 本机 HTTP 接收鉴权

实施范围为 Windows 11 当前用户、用户明确应用配置的 Claude Code/Codex 和
有同一路径 manifest 依据的 CodeBuddy CLI 2.98.0 本机实例。
接收器仍默认关闭，只绑定 127.0.0.1；file 导出不需要此令牌。
CodeBuddy CLI 2.98.0 官方发布记录已确认 OTLP 认证头，按该版本 manifest 限定接入；
旧版、新版、无 manifest 或非标准安装仍为手工核对。其他平台的原生密钥存储另验。

每次应用配置生成独立的 128 位来源标识和 256 位随机秘密；Windows 使用
BCrypt 系统随机源，Generic Credential 按当前用户保存，持久化限定本机。
凭据同时保存应用数据目录、客户端、配置路径。请求按来源标识读取对应凭据，
常数工作量比较秘密，并检查应用目录与允许的 HTTP 路径；重启不放宽认证。
没有凭据、存储不可用、认证头缺失/重复/无效一律拒绝，先鉴权再读正文及解压。
不接受浏览器 Origin 或声明转发的请求；这不能证明持有令牌的进程身份，
也不能防止同一用户故意转发，仍仅支持用户确认的本机 Agent。

Claude 使用 logs 专用 headers，Codex 使用 exporter headers；两个来源只允许
`/v1/logs`；CodeBuddy 2.98.0 使用发布记录明确的 generic headers（Bearer 空格
按官方例子编码 `%20`），只允许 `/v1/traces/supplemental` 并保持隔离。
rolling 文档新增的 traces-specific headers 不用于认证旧版支持，已有该键时手工核对。
现有远端或其他输出目标保留，不能向它们注入本应用的秘密。
用户配置需要 exporter 可读取的令牌，系统凭据库保存应用的认证副本；
因此不能称为秘密只存在凭据库。配置、条件回滚备份和撤销资料仅留本机。
预览只显示占位符，检查不生成/写入凭据；IPC、用量库、导出和诊断不返回令牌。

先核对预览版本，真实绑定接收器，再建立凭据并写用户配置。
配置失败回收本次凭据、恢复本次启用状态；撤销先吊销该凭据，随后仅恢复仍属于
本次配置的键。吊销失败须报告，不把取消失败报成成功。既有配置令牌失效时，
只有仍指向本应用的目标才提供重新确认配置，不自动改用户文件。
撤销句柄仍是当前进程内合同；重启后的持久撤销与完整安装生命周期另验。

依据核验日期 2026-10-04：

- [Claude Code monitoring](https://code.claude.com/docs/en/monitoring-usage)：logs headers、用户 env 与认证。
- [Codex sample configuration](https://developers.openai.com/codex/config-sample/)：otlp-http headers 表。
- [OTLP exporter 规范](https://opentelemetry.io/docs/specs/otel/protocol/exporter/)：按 signal 的 headers 与键值格式。
- [CodeBuddy 2.98.0](https://www.codebuddy.ai/docs/cli/release-notes/v2.98.0)、[环境变量](https://www.codebuddy.ai/docs/cli/env-vars)、[安装说明](https://www.codebuddy.ai/docs/cli/installation)：官网索引正文核验了版本/headers/包名；直接请求失败，不称为实时下载成功，未核验版本不自动套用。
- [BCryptGenRandom](https://learn.microsoft.com/en-us/windows/win32/api/bcrypt/nf-bcrypt-bcryptgenrandom)：系统随机源。
- [CredWriteW](https://learn.microsoft.com/en-us/windows/win32/api/wincred/nf-wincred-credwritew)、[CredReadW](https://learn.microsoft.com/en-us/windows/win32/api/wincred/nf-wincred-credreadw)、[CredDeleteW](https://learn.microsoft.com/en-us/windows/win32/api/wincred/nf-wincred-creddeletew)：用户范围、持久化和精确回收。

合成 HTTP、失败恢复与真实 Windows 凭据库测试分别记录，不能替代产品实际导出验收。
