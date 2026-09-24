# llm-usage

本项目计划提供本地 AI Agent 用量桌面客户端，统计各模型的 token、请求和缓存使用。
实施前的设计准备已完成，尚未开始 M0–M7，也未实现桌面客户端。

已确认 Windows 11 x64 首发，GitHub CI 保留 macOS/Linux，本地可尝试 WSL 构建。
所有 Agent 均保留本地支持计划，缺证 IDE 后移 F1；仅统计本机来源，支持计划中的界面定时提取配置。
用户已允许实施时提取本机真实 Agent 数据验证。

- [执行计划](Plan.md)：未完成任务与验收条件。
- [开工准备](docs/design/desktop-usage/implementation-readiness.md)：已确认范围、真实数据验证流程与检查结论。
- [详细设计](docs/design/desktop-usage/README.md)：架构、统计合同、Agent 接入、配置与测试。
- [调研依据](docs/design/desktop-usage/research.md)：官方资料、固定源码和原型静态核对。
- [定时提取](docs/design/desktop-usage/scheduling.md)与 [平台/CI](docs/design/desktop-usage/platform-ci.md)：后台任务和跨平台验收合同。
- 已有 [previous-draft](previous-draft/README.md) 作为参考，未进行运行验收。

- 共享规则：[AGENTS.md](AGENTS.md)。
- 维护流程与按需资源：[Skills](.agents/skills/README.md)。

## 文档验证

Node.js 22+。2026-09-24 核验发现根 package.json/package-lock.json 缺失，
恢复可复现文档依赖列在 M0，当前不能使用 npm ci 或 npm run lint:docs。
本机已有 markdownlint-cli2 时可在根目录直接检查：

```powershell
node node_modules/markdownlint-cli2/markdownlint-cli2-bin.mjs
git diff --check
```

已有 node_modules 不代表新检出的仓库可以复现安装。业务构建、测试与发布命令在 M0 建立，
文档检查不代替业务验收。
