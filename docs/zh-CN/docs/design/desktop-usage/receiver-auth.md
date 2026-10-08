# 本机 HTTP 接收鉴权

<a id="local-http-receiver-authentication"></a>

实施范围为当前用户、用户明确应用配置的 Claude Code/Codex 和
有同一路径 manifest 依据的 CodeBuddy CLI 2.98.0 本机实例。
接收器仍默认关闭，只绑定 127.0.0.1；file 导出不需要此令牌。
CodeBuddy CLI 2.98.0 官方发布记录已确认 OTLP 认证头，按该版本 manifest 限定接入；
旧版、新版、无 manifest 或非标准安装仍为手工核对。

跨平台存储沿用相同来源标识、绑定及撤销规则。Linux 使用当前用户的
Secret Service 默认持久集合与 DH 加密会话；不回退到 session 集合、明文文件或
内存凭据。默认别名也不能指向 session；检查不创建集合、不解锁。
服务缺失、默认集合缺失/锁定、匹配项重复
均拒绝接入。每次存储操作限定 3 秒，后台读取不显示解锁提示。
macOS 使用当前用户 Keychain generic password，明确禁用 iCloud 同步，
逐次查询禁止认证 UI；锁定、拒绝访问或无可用存储均拒绝接入。
两平台的随机令牌使用系统随机源，错误只返回稳定代码，不格式化库错误或凭据。
跨平台实现、原生存储往返和桌面实际 exporter 验收分别登记。

每次应用配置生成独立的 128 位来源标识和 256 位随机令牌；Windows 使用
BCrypt 系统随机源，Generic Credential 按当前用户保存，持久化限定本机。
Windows 原生并行测试已复现写入成功、立即回读缺失、10 ms 后读取到相同内容；
建立凭据、失败回收及撤销的变更前读取，只对缺失回读额外等待最多 5 次、每次 10 ms。
内容不符与读取错误立即拒绝，不重复写入。认证请求仍使用一次读取，缺失时拒绝；
这项本机观察不推广为所有 Windows 环境的 API 一致性保证。
删除后须回查缺失才报成功；Windows 仍读到完全相同的自有内容时，最多等待同样的
50 ms，不再次删除。外部替换、读取错误或超限均报告失败，保留其他来源。
凭据同时保存应用数据目录、客户端、配置路径。请求按来源标识读取对应凭据，
常数工作量比较令牌，并检查应用目录与允许的 HTTP 路径；重启不放宽认证。
没有凭据、存储不可用、认证头缺失/重复/无效一律拒绝，先鉴权再读正文及解压。
不接受浏览器 Origin 或声明转发的请求；这不能证明持有令牌的进程身份，
也不能防止同一用户故意转发，仍仅支持用户确认的本机 Agent。

Claude 使用 logs 专用 headers，Codex 使用 exporter headers；两个来源只允许
`/v1/logs`；CodeBuddy 2.98.0 使用发布记录明确的 generic headers（Bearer 空格
按官方例子编码 `%20`），只允许 `/v1/traces/supplemental` 并保持隔离。
rolling 文档新增的 traces-specific headers 不用于核验旧版支持，已有该键时手工核对。
现有远端或其他输出目标保留，不能向它们注入本应用的令牌。
用户配置需要 exporter 可读取的令牌，系统凭据库保存应用的认证副本；
因此不能称为令牌只存在凭据库。配置、条件回滚备份和撤销资料仅留本机。
预览只显示占位符，检查不生成/写入凭据；IPC、用量库、导出和诊断不返回令牌。

先核对预览版本，真实绑定接收器，再建立凭据并写用户配置。
配置失败回收本次凭据、恢复本次启用状态；撤销先吊销该凭据，随后仅恢复仍属于
本次配置的键。吊销失败须报告，不把取消失败报成成功。既有配置令牌失效时，
只有仍指向本应用的目标才提供重新确认配置，不自动改用户文件。
撤销句柄仍是当前进程内接口约定；重启后的持久撤销与完整安装生命周期另验。

依据核验日期：exporter/Windows 为 2026-10-04，跨平台存储为 2026-10-05。

- [Claude Code monitoring](https://code.claude.com/docs/en/monitoring-usage)：logs headers、用户 env 与认证。
- [Codex sample configuration](https://developers.openai.com/codex/config-sample/)：otlp-http headers 表。
- [OTLP exporter 规范](https://opentelemetry.io/docs/specs/otel/protocol/exporter/)：按 signal 的 headers 与键值格式。
- [CodeBuddy 2.98.0](https://www.codebuddy.ai/docs/cli/release-notes/v2.98.0)、[环境变量](https://www.codebuddy.ai/docs/cli/env-vars)、[安装说明](https://www.codebuddy.ai/docs/cli/installation)：官网索引正文核验了版本/headers/包名；直接请求失败，不称为实时下载成功，未核验版本不自动套用。
- [BCryptGenRandom](https://learn.microsoft.com/en-us/windows/win32/api/bcrypt/nf-bcrypt-bcryptgenrandom)：系统随机源。
- [CredWriteW](https://learn.microsoft.com/en-us/windows/win32/api/wincred/nf-wincred-credwritew)、[CredReadW](https://learn.microsoft.com/en-us/windows/win32/api/wincred/nf-wincred-credreadw)、[CredDeleteW](https://learn.microsoft.com/en-us/windows/win32/api/wincred/nf-wincred-creddeletew)：用户范围、持久化和精确回收。

合成 HTTP、失败恢复与真实 Windows 凭据库测试分别记录，不能替代产品实际导出验收。
Linux 原生存储/HTTP 和 macOS 交叉类型检查见
[跨平台记录](../../validation/desktop-usage/platform-auth-continuation.md)；macOS 原生 Keychain
跨进程往返与真实 HTTP 撤销见 [本批 CI](../../validation/desktop-usage/ci-plan-validation.md)。真实桌面与
exporter 验收仍独立保留。Windows 并行存储失败已复现写后短暂缺失，并增加有界
变更回读与删除确认；首次失败、跨进程撤销异常及最终复测集中到
[来源规则继续验收](../../validation/desktop-usage/source-policy-upgrades.md)。
