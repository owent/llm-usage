# 文档站与语言要求

<a id="文档站与语言合同"></a>

<a id="documentation-site-and-language-requirements"></a>

<a id="documentation-site-and-language-contract"></a>

状态：已发布，导航和布局优化已核验。既有桌面设计说明仍是应用行为的主要说明；[Plan.md](../../Plan.md)
统一维护当前工作和验收条件。

<a id="content-and-ownership"></a>

## 内容与归属

使用文档、架构设计、开发文档及源码注释默认使用英文。文档在
`docs/zh-CN/<仓库相对路径>` 下保存完整简体中文对应版本。开发参考包括字段说明、
实现检查、测试预期和资源说明。AI 规则、`.agents/skills/`、`Plan.md` 及
`docs/design/desktop-usage/execution.md` 执行计划仅维护原件，不翻译或生成站点页面。
翻译保留版本标识、覆盖 ID、
数量、命令、来源、失败、未知值及历史结果与当前结果的区别。解释或复现行为需要时，
原文引述及运行时示例保持原样。应用翻译目录保持完整。

英文文件是主要维护版本。两种语言同步修改，经过检查的翻译清单记录双方哈希；
缺少对应版本或修改后未核验时文档检查失败。相对文档链接指向对应语言，
源码和资源链接指向实际仓库文件。既有显式章节链接标识保持稳定。

源码注释使用英文，中文对应内容在中文开发文档中按仓库路径和稳定注释标识索引。
仅注释修改必须保留运行时字符串、样本、许可及可执行行为。

公共指南在 `docs/site/content/` 中保存中英文配对页面，覆盖安装、首次采集、
看板阅读、来源、客户端支持、遥测、调度、估算、用量和费用提醒、保留、交换、隐私、
排障、无障碍、开发、架构、数据规则、适配器、测试、发行和文档维护。
既有设计说明和验证记录从主要维护文件生成参考页面，不维护另一份可编辑副本。

每份文档直接由当前大语言模型完整翻译，比较原文和译文后才登记已审阅的文档。
两种语言均按工程[写作指导](../../.agents/skills/ai-maintenance/references/writing-guidance.md)
修正表达，写出实际文件、字段规则、检查或结果，避免含糊的研发简写。
标题、导航、功能卡片、按钮和图注也要审阅。

<a id="site-behavior"></a>

## 站点行为

使用 Astro 和 Starlight 构建静态站。英文路径 `/`，中文路径 `/zh-cn/`。
首次访问根页面按浏览器首个受支持语言选择，英文为回退。用户明确选择优先，
在本机保存。分享的深链接保持其显式语言。语言切换保留对应页面、查询及片段；
本地存储不可用不阻断导航。没有 JavaScript 时，根文档默认英文。

使用 Starlight 的可访问导航、本地搜索、目录和主题控件，结合工程品牌及响应式字体。
支持跟随系统、亮色、暗色。导航、搜索文案、内容及截图说明使用所选语言。
不需要分析追踪、远端字体、登录或第三方运行时请求。

站点和首页标题使用“AI usage dashboard” / “AI 用量看板”。介绍突出 token、调用、缓存
及趋势；数据存储和本地来源限制在隐私与数据源说明中解释。首页首个下载按钮打开对应
语言的安装页下载章节。尚无 Release 安装包时，链接已有构建制品并说明登录要求。

宽屏下扩展正文、表格和截图。左侧菜单与右侧目录各自可操作，避免空导航分组及内容
重叠。按已审阅原件清单生成 Starlight 多语言 `slug` 链接，不依赖其默认内容目录。
两种语言均核验实际分组链接、展开、键盘导航、当前页标识及手机菜单。

<a id="visual-design"></a>

## 视觉设计

页面背景、阅读区、导航和卡片分别配色。亮色主题采用柔和的冷灰蓝页面及阅读背景、
深蓝页头和青绿操作按钮。避免偏黄、奶油色背景及大面积亮白。卡片和装饰图标背景也
使用冷色。暗色主题采用深蓝背景、较亮的卡片和薄荷绿操作按钮。
正文和图注在默认、悬停、选中、聚焦时均应清晰。普通文字对比度至少为 4.5:1，必要的
控件标识与相邻颜色对比度至少为 3:1；装饰性分隔线不承担控件状态。正文链接保留下划线，
选中菜单加粗，避免只靠颜色表达含义。

仅在页面外侧留白和首页插图区加入静态、稀疏的 CSS 点阵，插图周围使用间隔较大的圆弧。
长文阅读区、表格、卡片及截图图注保持不透明，不铺纹理。装饰不响应鼠标、不承载信息，
不需要额外下载，在系统强制颜色和打印模式下隐藏。保留跟随系统与手动主题选择、减少
动态效果设置、宽屏正文空间和手机导航。实际审阅中英文首页与指南的亮暗主题截图；
对比度检查不能单独说明视觉效果，也不代表完整的 WCAG 符合性检查。

<a id="screenshots"></a>

## 截图

使用隔离合成来源及应用数据，采集真实桌面程序。不暴露本机用户路径、凭据、
会话正文或账户详情。分别设置英文和中文后截图，截屏前仅将界面中本任务的
测试路径匿名化为 Demo 路径，不改变数值或控件，并在来源说明中记录。
保留原 PNG 像素、清晰文字和有用
示例数据，不添加水印或 AI 重建。记录应用版本、视口、主题、语言、数据来源
及检查。截图用于说明界面，不替代真实供应商验收。

首页用整行截图配合功能说明，展示总览、趋势、详情、数据源和设置。截图语言与页面
一致，主题跟随站点选择，包括与系统设置不同的手动主题。保持原图比例，提供对应
语言的替代文字和图注，并链接到原 PNG，方便在小屏上查看细节。后续截图按需加载。
仓库 README 展示对应语言的总览示例，看板指南在字段说明旁补充详情示例。
在图片旁说明使用合成数据。浏览器检查覆盖图片加载、语言、主题切换、原图链接、
键盘访问及窄屏布局。

<a id="build-and-publication"></a>

## 构建与发布

根 npm 命令统一管理依赖安装、开发、检查和构建。生成内容、预览输出和任务
产物位于仓库根 `build/`。仅跟踪撰写内容、配置、脚本和已经核对的截图。

`gh-pages` 分支根目录保存编译后的静态站，包含 `llm-usage.atframe.work` 的
`CNAME` 和 `.nojekyll`。专用流水线在文档、翻译输入、站点工具和发布配置改变
时检查构建，仅发布默认分支成功构建。PR 可检查预览但没有发布权限。
发布并发串行化，保留分支历史，不强推其他分支。

Git 与 GitHub 命令最长运行 60 秒，不等待交互输入。Pages 构建轮询期限为十分钟，
包含请求时间。写操作超时后先检查远端分支或构建实际状态，再决定是否重试。
CI 作业另有十五分钟运行上限。

单独的管理员参数 `--configure-environment` 检查 `github-pages` 部署环境。如果环境使用
自定义分支规则，
添加文档发布作业需要的精确 `main` 分支，并读回核对。保留全部既有分支、标签规则
及其他环境保护设置。普通 CI 不修改这些设置，也不获得 Administration 权限。
Pages 本身仍构建已编译的 `gh-pages` 分支。

通过受支持的 GitHub API 配置 Pages 与域名。`llm-usage.atframe.work` 指向
`owent.github.io` 的 DNS CNAME 单独配置。DNS 代理可能返回 A/AAAA 地址，不公开原 CNAME；
报告缺少记录前须核对实际 DNS 与 HTTPS 响应。`CNAME` 文件本身不建立 DNS 或 HTTPS。
构建、分支发布、Pages 部署、DNS 和 HTTPS 分别报告核验状态。

<a id="acceptance"></a>

## 验收

- 使用文档、架构设计、开发文档和源码注释完整提供中英文。
- AI 规则、Skills 和执行计划不保留译文副本，也不生成站点页面。
- 链接、章节标识、清单、截图语言及生成产物检查。
- 静态生产构建成功，两种语言本地搜索可用。
- 浏览器核验系统语言选择、手动选择、深链接、存储失败、亮暗主题、桌面/移动布局、
  键盘导航和坏链接。
- 真实中英文桌面截图，附来源记录。
- 注释修改的适当应用检查及 `git diff --check`。
- 真实 gh-pages 发布和已观察 Pages 部署；DNS/HTTPS 独立核验，或明确报告尚缺 DNS 前提。

<a id="verified-implementation-sources"></a>

## 已核验实施依据

2026-10-07 获取：

- [Starlight 多语言](https://starlight.astro.build/guides/i18n/)：本地化内容路径、导航和
  UI；缺少翻译会回退，本工程覆盖检查须阻止必需页面出现该情况。
- [Starlight 组件替换](https://starlight.astro.build/reference/overrides/)：扩展页面 head
  和语言控件，同时保留主题/导航行为。
- [Astro 多语言](https://docs.astro.build/en/guides/internationalization/)：请求语言 API
  需要按需渲染，静态站入口使用浏览器语言选择。
- [Astro GitHub Pages 部署](https://docs.astro.build/en/guides/deploy/github/)。
- [GitHub Pages 发布源](https://docs.github.com/en/pages/getting-started-with-github-pages/configuring-a-publishing-source-for-your-github-pages-site)。
- [GitHub Pages REST API](https://docs.github.com/en/rest/pages/pages)。
- [GitHub 自定义域名](https://docs.github.com/en/pages/configuring-a-custom-domain-for-your-github-pages-site/managing-a-custom-domain-for-your-github-pages-site)。

2026-10-08 获取：

- [Starlight 侧栏导航](https://starlight.astro.build/guides/sidebar/)：显式 `slug` 链接使用译文标题并指向对应语言页面。
- [Starlight 样式](https://starlight.astro.build/guides/css-and-tailwind/)：内容宽度变量与自定义 CSS。实际布局及侧栏目录假设以已安装 0.42.5 源码为依据。
- [GitHub 制品下载](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/download-workflow-artifacts)：需要登录并具备读取权限。
- [GitHub 部署分支规则](https://docs.github.com/en/rest/deployments/branch-policies?apiVersion=2026-03-10)。
- [Cloudflare DNS 代理行为](https://developers.cloudflare.com/dns/proxy-status/)：代理记录公开 Cloudflare 地址，而非源站地址。
- [W3C 文字对比度](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html)：
  普通文字至少为 4.5:1，比较时使用未经舍入的计算结果。
- [W3C 非文字对比度](https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html)：
  必要的控件及状态标识至少为 3:1，与纯装饰边界分别处理。
- [W3C CSS 装饰图像](https://www.w3.org/WAI/WCAG22/Techniques/css/C9)：
  CSS 可以提供装饰图像，避免为辅助技术增加无关信息。
- [Material 颜色用途](https://github.com/material-components/material-components-android/blob/master/docs/theming/Color.md)：
  区分背景、容器及对应的文字颜色。本项目参考这种配色组织方式，不依赖 Android 组件。

registry 报告 Astro 7.3.6、Starlight 0.42.5。其 Node engine 与 peer 范围按安装后的
锁文件检查，不升级桌面应用依赖。
