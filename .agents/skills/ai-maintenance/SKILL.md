---
name: ai-maintenance
description: "Use when: maintaining this repository's AI rules, skills, client compatibility, or maintenance evidence."
metadata:
  owner: "repository-maintainers"
---

# AI 维护

先核对任务范围与 Git 现状，按下表仅读相关资源，不预载整个目录。
修改后运行 `npm run lint:docs` 并检查引用；迁移保留覆盖 ID、来源和未验收项。

| Use when                               | 读取                                                                                                  |
| -------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| 维护规则分层、工程流程或文档约定       | [维护合同](references/maintenance.md)                                                                 |
| 编写或修改回复、注释、文档、PR 说明    | [写作指导](references/writing-guidance.md)                                                            |
| 选择 CLI 或处理 Windows 执行问题       | [终端工具](references/terminal-tools.md)                                                              |
| 接入 MCP、调试外部服务、部署或处理秘密 | [操作边界](references/operations.md)                                                                  |
| 核验客户端入口与加载差异               | [客户端记录](references/clients.md)                                                                   |
| 修改易变事实或核对官方依据             | [来源索引](references/source-index.md)                                                                |
| 实质修改 Skill 描述或流程              | [触发与质量评估](references/skill-evaluation.md)                                                      |
| 恢复初始化或审查交付证据               | [覆盖表](references/records/initialization-coverage.md)、[验证记录](references/records/validation.md) |
