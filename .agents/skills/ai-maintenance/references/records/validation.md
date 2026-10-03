# 维护验证记录

本页是初始化及迁移时的证据快照，不能用其旧状态推断当前工程。
当前规则与命令见根 AGENTS.md，最新业务及规则检查见
[最新验收](../../../../../docs/validation/desktop-usage/current-acceptance.md)。
历史命令中的 docs/ai 是当时真实路径，不是当前调用入口。

## 环境与证据范围

日期：2026-09-24；cwd：D:/workspace/github/owent/llm-usage。
初始 Git main 无提交，工作区仅有 Kilo 忽略配置，无跟踪文件、业务代码、
构建、测试、CI 或 Plan。上级目录到磁盘根未发现 AGENTS.md。
现有 .kilo/.gitignore 保持原内容。
版本、工具回退见 [终端记录](../terminal-tools.md)，客户端见 [兼容表](../clients.md)。

本次有用户授权的仓库维护和本地静态验证，不写用户配置、不调用发布接口，
不 commit/push。当前 harness 支持文件补丁、shell 和只读网页调研；
客户端目录验证使用本机 Codex 的实际协议，不使用伪造的工具名。

## 已执行检查

| ID | 命令或方法 | 实际结果与限制 |
| --- | --- | --- |
| V01 | git rev-parse --show-toplevel、git status --porcelain=v1、git ls-files、rg --files --hidden | Git 根正确，初始零跟踪文件；不是业务实现审查 |
| V02 | Get-Command、各 --version、code --list-extensions --show-versions、shim 内容核验 | 高频 CLI 路径/版本已确认；fd/sd/bat 不在 PATH；不等同整盘未安装 |
| V03 | npm install --ignore-scripts --no-audit --no-fund | 退出 0，安装 87 个文档工具包，固定 markdownlint-cli2 0.23.3 并生成完整性锁 |
| V04 | npm run lint:docs | 当前首轮修正后 12 个 Markdown 文件、0 问题，退出 0；最终文件集结果在下节 |
| V05 | python -X utf8 系统 skill-creator/scripts/quick_validate.py .agents/skills/ai-maintenance | 退出 0，Skill is valid；不证明实际触发或完整规范/安全验收 |
| V06 | codex app-server generate-json-schema --out build/ai-init/schema | 退出 0；以安装版 schema 检查 initialize、skills/list 参数 |
| V07 | python -X utf8 build/ai-init/check-discovery.py | 退出 0；根和 docs/ai 各发现 1 个本仓库 Skill，enabled=true、errors=0 |
| V08 | 同 V07，第二个隔离进程使用 skills.config 临时禁用覆盖 | 根和 docs/ai 各发现同一 Skill 且 enabled=false、errors=0；没有配置写入 |
| V09 | 源要求逐段读至结束标记、逐项映射、实际正文回读 | 覆盖记录保留源行号与附件哈希；静态清单不代替运行效果 |

V07/V08 合计 4 个真实目录/启用状态断言，模型调用数 0，用户配置写入数 0。
使用子进程 stdio，没有启动模型会话或自定义 Agent；stdin 关闭后等待进程退出，
超时只终止本次自有进程。脱敏结果保存在
build/ai-init/discovery-result.json；临时 schema、检查脚本和结果均在已忽略的
build/ai-init/，保留供本地复核，不进入提交。

初始沙箱启动 CreateProcessAsUserW failed: 5 属平台进程错误，
使用平台允许的执行重试后继续，未降低安全设置。
Skill 验证器首跑使用 Windows 默认 GBK 导致 UnicodeDecodeError；
改用 Python -X utf8 后通过，未改 Skill 编码或校验器。
首次 lint 报 7 个空行问题，修正文档后通过，未关闭规则。
MD033 只允许实际使用的 a 锚点；MD013 对表格/代码块按结构排版，
其他默认规则保留。

## 最终静态验收

| 检查 | 实际命令/方法 | 结果 |
| --- | --- | --- |
| 完整 Markdown 集 | npm run lint:docs | 13 文件，0 问题，退出 0 |
| 相对路径与锚点 | node build/ai-init/check-docs.mjs，使用 markdown-it 解析实际链接 | 13 文件、390 条本地引用均可达，退出 0 |
| 覆盖结构与工具表 | 同上，检查 ID 唯一、章节与真实工具行 | 24 章、290 项、31 工具；无重复 ID |
| 原输入反查 | 去除围栏后提取章节、列表/表格，逐项比对源行；模板单独核对 | 24 章、225 个列表/表格源行，未映射 0；12 个填写模板子项及角色目标已登记 |
| 锁文件安装 | `npm ci --ignore-scripts --no-audit --no-fund --registry=https://registry.npmjs.org` | 87 包，退出 0；没有执行安装脚本 |
| 锁文件来源 | jq 解析所有 resolved URL 并核对主机 | 仅 registry.npmjs.org，保留 integrity；不修改用户 npm 配置 |
| Skill YAML | yq --front-matter=extract -o=json '.' .agents/skills/ai-maintenance/SKILL.md | name、description、字符串 metadata 解析正确，退出 0 |
| 退出码合同 | jq -n -e false；jq -n -e empty；rg 查询不存在的固定标记 | 分别 1、4、1，按合同判断，非测试失败 |
| 忽略规则 | git check-ignore 对 build、node_modules、.env、development/secret 的代表路径 | 四项均命中，退出 0 |
| 差异与保护 | git diff --check、git status --short、回读 .kilo/.gitignore | 退出 0；初始无跟踪文件，另以完整文件检查覆盖本次未跟踪产物 |

AGENTS.md 为 72 行/4124 字节，Skill 为 48 行/3046 字节；完整要求留在按需证据中。
所有实际引用均已在仓库内落地，没有依赖聊天附件路径的运行入口。
当前只有文档及文档工具，没有业务单元测试数量或生产结果。

初始 npm 安装继承本机腾讯镜像，已在任务临时目录从 npm 官方 registry
生成并核验新锁文件，再替换本次生成的锁文件和执行 npm ci。
无全局配置修改。完整检查脚本及脱敏 JSON 保留在已忽略的 build/ai-init/，
永久记录以本页为准，不依赖临时产物才能理解验收结果。

## 未执行的运行验收

- AGENTS.md 在新模型会话中的实际加载、同名冲突、权限拒绝和 Skill 无副作用调用，
  没有用目录列表冒充已通过。
- 20 条真实自动路由查询及 3 个启用/未启用对照任务未执行。
  当前执行策略未允许额外启动模型代理；无模型目录探测不受此限制。
  [评估集合](../skill-evaluation.md)有输入、判据和记录要求，不能据此宣称效果。
- 其他 12 个客户端的团队采用范围、对应版本/会话及实际加载未确认。
- 业务用途、技术栈、build/test/typecheck 和真实服务验收没有可用项目合同。

上述缺口在 [覆盖记录](initialization-coverage.md)保留阻塞 ID。
项目 MCP、永久自定义 Agent、OpenSpec/Superpowers 和生产部署未采用，
实际运行验收不属于本次范围；相应未来流程已落文档。

## 规则迁移验证

本轮范围：迁移 AI 维护规则、缩短入口、使用 Use when、增加写作指导；不修改 previous-draft 内容。
8 个规则/证据文件迁入 Skill references，其中大份证据归 references/records；原目录索引并入 Skill。
回读摘要、正文和全部本地链接，保留原 290 个覆盖 ID、状态和 31 项工具。
真实模型触发、行为质量对照仍未执行，本轮不重新声称初始化已完整验收。

| 检查 | 实际命令/方法 | 结果 |
| --- | --- | --- |
| 本次 Markdown 文件 | node node_modules/markdownlint-cli2/markdownlint-cli2-bin.mjs --no-globs AGENTS.md README.md '.agents/**/*.md' | 13 文件，0 问题，退出 0 |
| 全仓库 Markdown | npm run lint:docs | 14 文件，退出 1；6 个问题均在未修改的 previous-draft/README.md：L26/L65 的 MD040 共 2 项、L35 的 MD060 共 4 项 |
| 本地引用与覆盖结构 | node build/ai-skills-refactor/check-docs.mjs | 14 文件、396 条本地引用、25 章、296 个独立 ID、31 项工具；退出 0 |
| 内容保留 | 与迁移前快照逐项比较，检查 ID/状态和正文差异 | 原 290 个覆盖 ID 与状态全部保留；operations/terminal-tools 除链接外正文不变；其他变更逐段回读 |
| Skill 格式 | python -X utf8 系统 skill-creator/scripts/quick_validate.py .agents/skills/ai-maintenance | Skill is valid，退出 0 |
| 原生发现与禁用 | python -X utf8 build/ai-skills-refactor/check-discovery.py | 根和 references 各发现 1 个 Skill；启用/临时禁用共 4 个断言通过，errors=0，模型调用/用户配置写入均为 0 |
| 差异与旧目录 | git diff --check；核对迁移映射和原路径 | 退出 0，docs/ai 已移除；当前产物未跟踪，另以完整文件检查覆盖 |

入口大小按 UTF-8 文件字节统计，不作为模型 token 数或行为收益：

| 入口 | 迁移前 | 迁移后 | 字节减少 |
| --- | --- | --- | --- |
| SKILL.md | 48 行 / 3046 字节 | 22 行 / 1260 字节 | 58.6% |
| AGENTS.md | 72 行 / 4124 字节 | 49 行 / 2490 字节 | 39.6% |

规则和历史证据按场景读取，没有为减小入口删除正文或原初始化阻塞项。
previous-draft 在本轮工作中新增；只读取 README 修正“空仓库”描述，未修改原型或执行其业务程序。
全仓库 lint 的原型问题保持原状，未改配置排除该目录或关闭规则。
尝试通过 npm script 传入 --no-globs 时，本机 npm 返回 EUNKNOWNCONFIG；
范围检查改为直接调用已安装的 markdownlint-cli2，规则与根配置相同。
迁移快照、映射及脱敏检查结果保留在已忽略的 build/ai-skills-refactor/，不作为运行时依赖。
