# 应用标识与静态资源

<a id="application-identity-and-static-assets"></a>

<a id="design-contract"></a>

<a id="设计合同"></a>

<a id="design-specifications"></a>

## 设计说明

标识名为 **Usage U**：开口的 U 表示 usage，中间的刻度表示被观测的用量，
上方琥珀色小点作为视觉识别点。它不表示连接状态、实时采集或零用量。
深墨色圆角底承载青绿色标记；单色版保留 U 与刻度，省略小点。
这是本仓库原创的 SVG 几何设计，无外部图片、字体、品牌标志或运行时请求。

| 颜色 | 色值 | 用途 |
| --- | --- | --- |
| 墨色 | `#102A35` | 图标底色、浅色主题线条 |
| 青绿 | `#53DDC5` | U 形主体 |
| 浅薄荷 | `#D8FFF3` | 中央刻度 |
| 琥珀 | `#F2B86B` | 品牌识别点；不承担状态语义 |
| 灰绿 | `#77958F` | 插画辅助线 |

小于 24 px 使用专门绘制的 favicon 或单色托盘标记。品牌图标不加文字，
不依赖渐变或颜色表达必需信息。插画不嵌入文案，状态必须由界面文字说明。
预览页同时展示浅/深背景、原始小尺寸与三种空状态。

实际浏览器、Windows 包体与 LFS 检查见 [验证记录](../../docs/validation/desktop-usage/static-assets.md)。

<a id="files-and-consumers"></a>

## 文件与使用

| 文件 | 消费方 |
| --- | --- |
| `app-icon.svg` | 1024 × 1024 源文件；修改后重新生成派生文件 |
| `tray-dark.svg`、`tray-light.svg` | 16 × 16 托盘源，分别用于浅色/深色背景 |
| `../src-tauri/icons/` | Tauri 配置已有的 PNG、ICO、ICNS；保留 CLI 生成的 Store/移动端配套文件，不代表这些平台已支持 |
| `../src-tauri/icons/tray/` | 16/20/24/32/48 px 单色 PNG 资源；macOS 黑色 template 图仅为资源，不代表已接入托盘 |
| `../public/brand/` | 生成的 SVG、256/512/1024 px PNG；当前窗口标题使用 SVG |
| `../public/favicon.svg`、`../public/favicon.ico` | 网页页签；SVG 小尺寸单独优化，ICO 为兼容回退 |
| `../public/ui/` | 24 px 导航/操作线条图标，SVG 随系统深浅主题切换；强制主题可通过 CSS mask 使用 |
| `../public/illustrations/` | 未添加来源、筛选无匹配、来源受限三种状态资源；资源本身不能据此确认采集能力 |
| `../asset-preview.html` | 本地审阅页，`npm --prefix desktop run dev` 后访问 `/asset-preview.html`；不进入 release 入口 |

图标控件由调用方提供可访问名称；旁边已有同义文字的图片使用 `alt=""`。
空状态文案示例：“还没有添加数据源”“当前筛选没有匹配记录”“部分来源暂时不可读取”。
无记录、读取失败和真实的零用量必须分开。Windows 可选关闭到托盘已在
`../src-tauri/src/tray.rs` 实施，目前使用应用窗口图标；专用单色资源及其他平台不能据此确认额外托盘接入。

<a id="generation-and-checks"></a>

## 生成和检查

仓库根，Node.js 22+；使用 desktop 锁文件中的 `@tauri-apps/cli`（当前 2.12.0），无额外绘图依赖：

```powershell
git lfs install --local
git lfs pull
npm --prefix desktop ci
npm run assets:generate
npm run assets:check
```

生成命令只重建上表中的派生图标，保留 UI/插画源文件；重复运行应产生相同字节。
CLI 2.11.5 的 ICNS 图层顺序不固定；脚本按类型排序并保留旧式 RGB/透明度图层对，图层内容不变。
渲染输出先写入临时目录，再复制到目标；渲染失败不会替换原图标，任何检查失败返回非零。
检查命令核验源文件、PNG 格式/尺寸、ICO/ICNS 目录、派生文件一致性和 LFS 属性，
遇到未下载的 LFS 指针直接失败。`--help` 可查看参数；`assets:check` 不改动资源。
回滚设计时同时恢复 SVG 源与派生资源，或恢复源后重新生成。

## Git LFS

根 `.gitattributes` 管理图片（含 SVG）、字体、媒体、压缩包、可执行文件、库、数据库与 Python 字节码。
整个 `desktop/public/`、Tauri 图标目录及原型的预构建 JS/静态 JSON 也使用 LFS。
TS/Svelte/CSS 源码、配置、文档与生成脚本仍用普通 Git，以便审阅。
构建目录和依赖目录继续忽略；LFS 规则不意味着应把安装包、缓存或本机数据加入仓库。
原型已跟踪的数据库和字节码只迁移存储，不修改其内容或追补新数据。

迁移采用 `.gitattributes` 配合指定文件的 `git add --renormalize`，转换当前索引，
不改写历史。历史提交中的普通 Git blob 保留。
新增资源经 `git add` 保存为 LFS 指针；工作树仍保存可直接打开的实际文件。
CI checkout 必须设置 `lfs: true`；使用资源前运行检查命令，避免把指针当图片打包。

2026-09-24 核对的依据：
[Tauri 图标规范](https://v2.tauri.app/develop/icons/)、
[Git LFS 3.7.1 迁移说明](https://github.com/git-lfs/git-lfs/blob/v3.7.1/docs/man/git-lfs-migrate.adoc)、
[当前固定 checkout 的 LFS 输入](https://github.com/actions/checkout/blob/d23441a48e516b6c34aea4fa41551a30e30af803/README.md)。
