# llm-usage

已有 LLM 用量看板原型见 [previous-draft](previous-draft/README.md)，运行合同尚未核验。

- 共享规则：[AGENTS.md](AGENTS.md)。
- 维护流程与按需资源：[Skills](.agents/skills/README.md)。

## 文档验证

Node.js 22+，在仓库根目录运行：

```powershell
npm ci --ignore-scripts --no-audit --no-fund
npm run lint:docs
```

根 package.json 依赖仅用于文档检查；业务测试与发布流程尚未核验。
