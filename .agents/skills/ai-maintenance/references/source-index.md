# 来源索引

<a id="fields"></a>

## 字段与维护

每条事实记录 claim/scope、source_url/source_version、verified_at/method、
installed_version/status、review_cadence/update_trigger、impact/owner。
下面表格共用：verified_at = 2026-09-24，owner = 仓库维护者；
review_cadence = 相关任务前复核，并每季度检查仍在使用的易变事实；
update_trigger = 升级、弃用、安全公告、加载失败、实际行为变化。
不自动升级工具；当前有效记录更新，历史由 Git 追踪。

## 当前采用的事实

| ID | claim / scope | source_url / source_version | method / installed_version / status | impact |
| --- | --- | --- | --- | --- |
| S01 | Codex 根到 cwd 的规则链、override 优先、默认 32 KiB | [官方规则](https://learn.chatgpt.com/docs/agent-configuration/agents-md)，rolling | 打开正文；CLI 0.155.0-alpha.16.3；文档已核验，完整运行见验证记录 | AGENTS、clients |
| S02 | Codex 共享 Skills、向上发现、同名不合并、自动调用默认开启 | [Build skills](https://learn.chatgpt.com/docs/build-skills)，rolling | 打开正文；同 S01；文档已核验 | Skill 路由、clients |
| S03 | Skill 必填字段、名称/描述限制、可选字段和渐进加载 | [规范](https://agentskills.io/specification)，rolling | 打开正文；本地 Skill 静态校验另记 | maintenance、SKILL |
| S04 | 触发样本包含正反近似场景，真实读取轨迹，多次及留出集 | [描述评估](https://agentskills.io/skill-creation/optimizing-descriptions)，rolling | 打开正文；不涉及安装版本；文档已核验 | skill-evaluation |
| S05 | 同输入、Skill 有无/旧版对照及产物质量评估 | [输出评估](https://agentskills.io/skill-creation/evaluating-skills)，rolling | 打开正文；真实模型评估尚未执行 | skill-evaluation |
| S06 | Kilo 规则和 Skills 分产品核验，不推断全客户端一致 | [规则](https://kilo.ai/docs/customize/agents-md)、[Skills](https://kilo.ai/docs/customize/skills)，rolling | 打开正文；VS Code 扩展 7.7.9；文档已核验，加载未验证 | clients |
| S07 | markdownlint-cli2 本地开发依赖、JSONC 配置、CLI globs | [维护者 README](https://github.com/DavidAnson/markdownlint-cli2)，main/rolling | 正文及 npm 元数据；0.23.3，Node >=22；运行另记 | package、lint 配置 |
| S08 | rg 默认忽略与隐藏文件、搜索/枚举选择 | [维护者 GUIDE](https://github.com/BurntSushi/ripgrep/blob/master/GUIDE.md)，master/rolling | 正文与 rg --version；15.2.0；版本及本次搜索已验证 | terminal-tools |
| S09 | jq -e false/null 与无结果退出码区分 | [手册](https://jqlang.org/manual/)，1.8 | 正文、jq --version；1.8.2；实际退出码另记 | terminal-tools |
| S10 | Mike Farah yq 的实现身份及 YAML 查询 | [维护者 README](https://github.com/mikefarah/yq)，master/rolling | 正文、yq --version；4.53.6；实际解析另记 | terminal-tools |
| S11 | PowerShell 原生参数模式、引号及 --% 边界 | [解析](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_parsing?view=powershell-7.5)，7.5 文档 | 正文；本机 7.6.6/Windows 参数模式；不宣称全选项兼容实测 | terminal-tools |
| S12 | PowerShell 编码默认因版本和命令不同 | [编码](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_character_encoding?view=powershell-7.5)，7.5 文档 | 正文；本机 7.6.6；新文件显式 UTF-8 | terminal-tools |
| S13 | Claude 原生读取的版本/会话门槛、导入四跳、路径规则与加载检查 | [Memory](https://code.claude.com/docs/en/memory)，rolling | 打开正文；PATH 无客户端；文档已核验，运行未验证 | maintenance、clients |
| S14 | VS Code Local/Agent Host 的 prompt files 支持不同，Agent Host 迁移到 Skills | [Prompt files](https://code.visualstudio.com/docs/agent-customization/prompt-files)，rolling | 打开正文；VS Code 1.139.0，实际会话类型未确认 | maintenance、clients |

根及 desktop 的依赖清单与锁文件均已恢复；实际版本与命令以当前文件为准，
不能从初始化快照推断当前状态。
业务设计的独立来源与版本见 [调研索引](../../../../docs/design/desktop-usage/research.md)，
不复制到 AI 客户端兼容来源表；此处不把滚动主分支等同锁文件版本。
OpenAI 规则页面的 .md 端点读取失败，已使用成功打开的 HTML 正文，
不把失败响应当作事实依据。

## 条件流程与未核验候选

工具完整清单来自本次用户输入，其原始附件哈希和章节映射见
[覆盖表](records/initialization-coverage.md)。除 S08–S12 及本地探测项，
其余工具仅保留候选用途，不宣称本轮核验版本、下载、安全或性能。
[客户端表](clients.md)中的未使用产品同样保留待核查官方入口。

OpenSpec、Superpowers、MCP、ClawHub 未采用，不复制原模板中的 latest/current
版本结论。相关流程作为本项目条件式设计约束；真正启用前从以下官方入口核对版本、
字段、授权及实现，然后新增事实记录：

- [OpenSpec 命令](https://github.com/Fission-AI/OpenSpec/blob/main/docs/commands.md)、
  [Superpowers 发布](https://github.com/obra/superpowers/releases)。
- [MCP 规范入口](https://modelcontextprotocol.io/specification)。
- [ClawHub API](https://docs.openclaw.ai/clawhub/api)、
  [安全审计](https://docs.openclaw.ai/clawhub/security-audits)。
- [OWASP 密钥管理](https://cheatsheetseries.owasp.org/cheatsheets/Secrets_Management_Cheat_Sheet.html)、
  [GitHub Actions 安全](https://docs.github.com/en/actions/reference/security/secure-use)、
  [AWS 幂等重试](https://aws.amazon.com/builders-library/making-retries-safe-with-idempotent-APIs/)。

以上候选链接本轮未逐页核验，不能作为已完成接入、安全认证或生产验收证据。

## 本地写作参考

2026-09-24 读取以下两个工作区中的
`.agents/skills/ai-agent-maintenance/references/writing-guidance.md`，
提取写作方法并按本仓库改写。这里记录来源，运行时不依赖外部路径。
HEAD 用于定位当时仓库，文件哈希用于识别实际读到的工作区内容，
不假定该文件与 HEAD 完全相同。

| 仓库 | 当时 HEAD | 文件 SHA-256 |
| --- | --- | --- |
| atframework/AICodeReviewer | 782fb752f1a588a2cc99ec29e9e037860ed113bb | CCD79F3D50BDDB0334D3319B2203D58F8A48DD86DD010F71E015763D659D4309 |
| atframework/atsf4g-co | 1f9aea233bdd3e58a362e7127fe8665b2928a741 | A71879408C0C3C535C5740D03BDEEF8897D147E01D8EA193218DD226F702DD15 |

采用内容：事实先行、具体动词、稳定术语、段落结构、中英文空话检查及技术语义例外。
未采用内容：其他项目的业务路径、站点禁词执行脚本、历史记录改写，
或仅凭词频识别 AI 作者的推断。
责任人为仓库维护者；在写作规则变更、实际误判或样例改变时复核。
原参考文件中的外部网页本轮未重新核验，其旧核验日期不转写成本次核验。
