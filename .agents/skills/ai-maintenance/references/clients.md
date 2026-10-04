# 客户端采用与兼容记录

## 当前范围

2026-09-24：当前 Codex 会话已确认；团队其他客户端范围尚未收到确认。
安装、文件存在、官方文档和实际加载须分别核验。
下表全部 13 个客户端已登记；未知采用情况保留缺口，不能写“不适用”。

| 客户端 | 本机核验与使用状态 | 入口处置和验收 |
| --- | --- | --- |
| Codex | 当前会话；CLI 0.155.0-alpha.16.3；IDE 扩展 26.917.62051 | 根 AGENTS.md、.agents/skills；官方机制已核验，实际测试见验证记录 |
| Claude Code | PATH 未发现，团队使用未知 | 暂不创建 CLAUDE.md；采用后核验版本、会话 flags、原生读取/导入及遮蔽 |
| VS Code Copilot | VS Code 1.139.0；扩展清单仅检出相关 websearch，无法据此确认 Copilot 会话 | 暂无专属文件；确认 Local/Agent Host、嵌套开关、Skills 与 prompt files 支持 |
| OpenCode | PATH 未发现，使用未知 | 暂无 .opencode；核验规则回退、权限、Skills 发现的 worktree 边界 |
| Kilo Code | CLI 启动包装器存在；VS Code 扩展 7.7.9；已有 .kilo/.gitignore | 保留原目录；共享入口，核验 CLI/VS Code 分别的外部 Skills 开关和动态 shell |
| Pi | Scoop shim 指向 pi-coding-agent；安装不代表项目采用 | 暂无 .pi；核验 project trust、规则覆盖、Skills 与 SYSTEM/APPEND_SYSTEM 差异 |
| Oh My Pi | Scoop shim 指向 oh-my-pi；安装不代表项目采用 | 暂无 .omp；核验最近非空原生目录遮蔽、用户来源 opt-in、常驻规则与 skill:// |
| Command Code | PATH 未发现，使用未知 | 暂无 .commandcode；分开核验 Skills 和 Agents 的同名顺序 |
| Zoo Code | 没有项目配置或已确认会话 | 不猜 .zoo 路径；启用前查 .roo、模式规则、开关及 Skills 支持 |
| OpenClaw | 没有项目配置或已确认会话 | 核验 workspace/state/library/workshop、allowlist 和个人来源边界 |
| Hermes Agent | PATH 未发现，使用未知 | 暂无 .hermes；核验 context first-match、项目根 Skills、外部目录与信任 |
| Devin Desktop | 没有项目配置或已确认会话 | 核验 Desktop/Local/Cascade 差异及 .devin/.windsurf 兼容 |
| Antigravity | 没有项目配置或已确认会话 | 核验 2.0/CLI/IDE 及 .agents 入口，不能推导独立 Gemini CLI 行为 |

除 Codex/Kilo 文档及下述 Claude/prompt files 条目外，上表“核验”列是用户输入提供的待核查线索，
不是本次已核验的产品支持承诺；采用时必须打开当时官方正文并比对安装版本。
官方入口见下文，不创建全部客户端目录。
Kilo/Pi/Oh My Pi 的 CLI 版本待在确认为团队使用后记录；
本轮只确认启动器身份，不调用它们的模型、网络或初始化流程。

## 已核验机制

Codex 在启动时从项目根到 cwd 组合规则，每目录优先 AGENTS.override.md，
然后 AGENTS.md，默认总量上限 32 KiB；嵌套不是全树预载。
Skills 从 cwd 向项目根扫描 .agents/skills，同名不会自动合并。
规则和 Skills 使用相反的查找方向，普通 Markdown 链接不构成发现入口。
依据为 [规则](https://learn.chatgpt.com/docs/agent-configuration/agents-md)、
[Skills](https://learn.chatgpt.com/docs/build-skills)，文档为 rolling，
安装版本运行结果单列在 [验证记录](records/validation.md)。

Kilo 官方页面列出根 AGENTS.md、AGENT.md 回退和按读取触发的嵌套规则；
Skills 文档有产品、位置及开关差异。不能把 CLI 选项直接套给 VS Code。
依据为 [规则](https://kilo.ai/docs/customize/agents-md)、
[Skills](https://kilo.ai/docs/customize/skills)，本轮没有 Kilo 会话加载验收。

Claude 的导入边界和 VS Code 的 prompt files 会话差异已核验文档，
见 [维护合同](maintenance.md#客户端兼容与角色)；两者都没有项目会话运行验收。

## 启用时查阅

| 客户端 | 官方入口，当前未采用这些页面中的版本断言 |
| --- | --- |
| Claude Code | [Memory](https://code.claude.com/docs/en/memory)、[Skills](https://code.claude.com/docs/en/skills) |
| VS Code Copilot | [Instructions](https://code.visualstudio.com/docs/agent-customization/custom-instructions)、[Skills](https://code.visualstudio.com/docs/agent-customization/agent-skills)、[Prompt files](https://code.visualstudio.com/docs/agent-customization/prompt-files) |
| OpenCode | [Rules](https://opencode.ai/docs/rules)、[Skills](https://opencode.ai/docs/skills) |
| Pi | [Configuration](https://pi.dev/docs/latest/configuration)、[Skills](https://pi.dev/docs/latest/skills) |
| Oh My Pi | [Context](https://github.com/can1357/oh-my-pi/blob/main/docs/context-files.md)、[Skills](https://github.com/can1357/oh-my-pi/blob/main/docs/skills.md) |
| Command Code | [Memory](https://commandcode.ai/docs/memory)、[Skills](https://commandcode.ai/docs/skills) |
| Zoo Code | [Custom instructions](https://docs.zoocode.dev/features/custom-instructions) |
| OpenClaw | [Skills](https://docs.openclaw.ai/tools/skills)、[Workspace](https://docs.openclaw.ai/concepts/agent-workspace) |
| Hermes Agent | [Context](https://hermes-agent.nousresearch.com/docs/user-guide/features/context-files)、[Skills](https://hermes-agent.nousresearch.com/docs/user-guide/features/skills) |
| Devin Desktop | [Rules](https://docs.devin.ai/desktop/cascade/memories)、[Skills](https://docs.devin.ai/desktop/cascade/skills) |
| Antigravity | [Rules](https://antigravity.google/docs/rules-workflows)、[Skills](https://antigravity.google/docs/skills) |

## 无副作用验收协议

每个实际采用的客户端记录版本、会话类型、配置开关和脱敏调用记录：

1. 从仓库根及 .agents/skills/ai-maintenance/references 子目录开启新会话；查看真实加载文件和规则覆盖来源，
   不能只让模型自报“已加载”。
2. 在目录/选择器中找到 ai-maintenance 并实际调用；
   确认读取 SKILL.md 的工具轨迹与资源可达性。
3. 同名冲突、禁用选项与权限拒绝只在任务自有的隔离 fixture/配置中验证，
   不改用户全局配置，不删除现有 Skills，不访问密钥或发布接口。
4. 使用 [触发与质量集合](skill-evaluation.md)，区分静态格式、
   目录发现、真实调用、行为效果；记录失败及下一步。

当前共享文件可手动读取；这只解决内容可达性，不能替代上述加载验收。
