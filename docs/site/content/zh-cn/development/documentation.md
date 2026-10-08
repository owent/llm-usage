---
title: 文档维护
description: 维护完整译文并核验发布站点。
sidebar:
  order: 6
---

## 内容归属

使用文档、架构设计和开发文档以英文为主。完整中文对应版本位于 `docs/zh-CN/<仓库相对路径>`。
用户/开发指南位于 `docs/site/content/`，在 `zh-cn/` 下保持相同页面路径。
维护英文原件和对应中文版本，不编辑 `build/` 中生成的参考页。
AI 规则、`.agents/skills/`、`Plan.md` 和 `docs/design/desktop-usage/execution.md`
仅保留原件，不翻译或生成站点页面。

`Plan.md` 维护活动状态，桌面设计说明维护行为，`adapters.md` 维护支持范围，
`research.md` 保存版本化产品依据，验证记录维护命令、测量和失败。翻译完整记录，
不改变数字、标识、未知状态、首次失败或验证范围。直接由当前大语言模型翻译，逐份比较
完整原文和译文，两种语言均按[写作指导](https://github.com/owent/llm-usage/blob/main/.agents/skills/ai-maintenance/references/writing-guidance.md)审阅表达。

英文源码注释和索引中文对照同步维护。运行时目录值、字面样本、原语言依据和第三方
许可不能盲目当作普通文案改写。仅注释工作保留可执行内容原样。

## 新增页面

撰写页提供标题、简洁描述和侧栏顺序；新增同名中文页并使用对应语言链接。
中英文截图分开，提供有意义的 alt 及合成来源说明。

MDX 页面导入 `@docs/components/AppScreenshot.astro`，传入页面名（`overview`、`trend`、`details`、
`sources` 或 `settings`）、对应语言（`en` 或 `zh-CN`）及该语言的替代文字。
图注放在组件内部。组件按站点主题选择亮色或暗色原图，并链接到完整尺寸的 PNG。
首张示例使用 `eager`，后续图片按需加载。示例占满内容行，给控件和文字留出空间。
Markdown 指南可以直接链接对应语言的 PNG；仓库 README 的相对截图链接在发布到
站点时转换为站内 `/screenshots/` 链接。
`@docs/` 别名相对于 `docs/site/src/` 解析；构建准备将撰写页复制到 `build/` 后仍然有效。
相对组件导入则会按复制后的文件位置解析。

构建准备将使用文档、架构设计和开发文档导入参考类别，通过 Markdown 语法节点转换文档/源码链接，
不改变代码块示例，并保留 route 中版本标点。不创建额外可编辑设计说明副本。
内容加载器去除嵌套页面 ID 末尾的 `/index`，使 `zh-cn/index.mdx` 对应中文语言根
`zh-cn`，避免 Starlight 在同一 URL 生成英文回退页。根目录 `index.mdx` 保持 ID `index`，
由 Starlight 自行规范。其他 ID 中的版本标点保持不变。

`scripts/sidebar.mjs` 读取英文撰写页元数据及仓库文档清单，保留页面顺序和嵌套版本目录，
向 Starlight 提供 `slug` 链接，由其解析对应语言标题和当前页状态。Starlight 0.42.5
按目录自动生成的功能假设内容位于 `src/content/docs/`；本工程生成文件位于根 `build/`，
因此该模式会产生空分组。未经实际链接核验不要恢复它。产物检查拒绝缺少必需链接，
浏览器检查展开七个分组，并在桌面和手机视口使用鼠标及键盘导航。

宽屏布局为左侧菜单和右侧目录各留固定空间，剩余内容扩展至最多 110rem，首页为 100rem。
检查响应式边界及 1920、2560 像素屏幕。首页下载按钮和用户下载链接均指向
`https://github.com/owent/llm-usage/releases/latest`，对应语言的安装指南说明软件包选择及运行要求。

主题颜色集中在 `src/styles/custom.css`：页面、阅读区、卡片、侧栏和边框分别使用
`--usage-*` 变量，文字及操作按钮使用 Starlight 变量。页头和首页主区域在两种主题中
使用相同的深蓝配色。外侧留白的静态 CSS 点阵间隔为 48px，插图区为 40px；圆弧装饰
不响应鼠标。阅读区和图注保持不透明背景。系统强制颜色与打印模式隐藏装饰。
避免增加动态背景或外部图片、字体下载。

浏览器测试在两种语言、两种主题的首页、看板指南和本开发页上，按实际计算样式及透明
颜色合成结果测量文字对比度。键盘焦点和悬停按钮分别检查，测量结果与视口截图一并保存。
若文字背景包含需要另行测量的 CSS 图片，检查会拒绝处理。除数值外，还须审阅中英文
截图、手机布局和宽屏阅读区。这些抽样检查不能替代完整的无障碍审查。

翻译核验清单保存已检查双方哈希。缺配对或修改后未经对应审查时内容检查失败。
更新核验条目前检查双方含义和完整性；哈希相符本身不证明翻译质量。

## 验证与截图

```powershell
npm run check:docs
npm run test:docs
npm run build:docs
npm run test:docs:browser
npm run lint:md
git diff --check
```

更新 UI 截图前构建桌面 release，再运行 `npm run docs:screenshots`。
采集工具初始化隔离存储，GUI 启动前保存仅手工根，通过真实 IPC 核对 240 次合成调用，
并分别采集中英文亮暗主题。目视检查图片、核对来源并提交真实 LFS 资源。
截图是界面示例，不能据此确认真实供应商/版本。

## 发布与恢复

文档流水线发布前检查构建。默认分支文档变化将编译结果发布到 `gh-pages` 根目录，
保留历史、`CNAME` 和 `.nojekyll`；PR 不能发布。Pages 部署显式执行，因为单独使用
`GITHUB_TOKEN` 推送不会触发分支 Pages 构建。

首次发布在所有检查通过后，由管理员的 GitHub CLI 会话使用发布脚本的
`--configure-pages` 参数。创建或修改 Pages 设置需要 Administration 与 Pages 权限；
普通 CI 不授予 Administration 权限。后续构建更新线上分支前，会核对已配置的来源分支、
根目录和自定义域名。
单独的管理员参数 `--configure-environment` 会在 `github-pages` 部署环境采用自定义分支
规则时添加精确的 `main` 分支，保留既有规则及其他保护设置，并读回核对。文档发布作业从 `main` 运行，
Pages 构建已编译的 `gh-pages` 分支。普通 CI 仍仅使用既有 contents/pages 权限。
只有得到部署分支规则修改授权后才能使用此参数；单独使用 `--configure-pages` 不会修改这些规则。
接口说明见 GitHub 的[部署分支规则 API](https://docs.github.com/en/rest/deployments/branch-policies?apiVersion=2026-03-10)。
在干净的源码提交上完成内容、单元、生产构建和浏览器检查后，运行
`node docs/site/scripts/stamp-output.mjs` 记录源码 revision 及输出文件摘要。
发布脚本拒绝未记录验收的输出、变更后的资源及与当前 `main` 不一致的 revision。
流水线在上传已验收制品前完成此步骤。

域名为 `llm-usage.atframe.work`，DNS 路由指向 `owent.github.io`。
启用 DNS 代理后，公开查询可能返回代理的 A/AAAA 地址，不显示原始 CNAME。
GitHub Pages 设置、DNS 和 HTTPS 分别核验，域名文件本身不代表自定义域名部署成功。
失败构建不进入线上分支。恢复时构建已知良好源码 revision 后重新发布，不强推其他分支，
不删除应用数据。
