# Windows 与 Debian 容器安装验收

日期 2026-10-05；版本 0.2.1；合同见
[安装生命周期](../../design/desktop-usage/installation-lifecycle.md)。
Windows cwd 为仓库根；Linux cwd 为 WSL Debian 本任务独立工作副本。
用户已取消 macOS 桌面与特定硬件要求，允许 Windows 本机及 Linux Podman 安装验收。

## 环境与制品

Windows 11 Pro x64 10.0.26300，Node 24.21.0、Rust 1.98.1、Tauri CLI 2.12.0、
PowerShell 7.6.6、WebView2 154.0.4258.53。旧 NSIS 是既有实际 0.2.0 包，
源码 revision 未确认，不能仅凭版本归到某次提交。当前包基于 0c5d63c 与本工作树
修改；安装在 build/install-lifecycle/windows/ 的 Unicode/空格目录，来源与数据隔离。

Linux：WSL 2 Debian 13.7 x86_64、kernel 6.18.40.1、rootless Podman 5.4.2。
普通 acceptance 用户 UID 1000，Xvfb/Openbox、独立 D-Bus、WebKitWebDriver，
实际 GTK/WebKit 前端与 IPC；默认 seccomp，无 privileged、宿主目录挂载
或关闭 WebKit 沙箱。提取轮次无额外 capability，FUSE 轮次仅添加 SYS_ADMIN；
GUI 进程有效能力仍为 0。容器运行 `--network=none`，仅打包控制操作使用映射容器 root。
Debian 官方基础镜像固定
`docker.io/library/debian@sha256:7792b1f7702a86946cd518db72b6a407302c3e9bc1635634368b878189e8221c`；
最终 GUI 镜像 ID
`8fdeddb5aadaf9e6afcb68434ae4269b669ba23d776830b52adbef2684758154`，
由原 GUI 镜像 `311d072b80b15c3a57a9afd8135deed530931aec6e4afac81361f35f6f19793e`
增加 fonts-noto-cjk 1:20240730+repack1-1 得到。
旧 Linux 包从 2e8ceadecbba0b93e10777c51b4c0d01b4b61dd3 的原锁文件构建；
新包包含本轮跨平台凭据、早期 CLI 分流与最新 Qwen/OpenCode 能力说明；Windows
专属卸载代码不进入 Linux 二进制。此前包摘要分别保存在根 build/install-lifecycle/
final-windows-metadata.json、final-linux-metadata.json，与本次最终包分开记录。

| 实际制品 | 字节 | SHA-256 |
| --- | ---: | --- |
| Windows 0.2.0 NSIS | 3,901,720 | 4572792f5a7010fc5dfbdd02fc0e3155a5c64ee38a974328a9a3df3400ffbab5 |
| Windows 0.2.1 NSIS | 3,925,533 | c058671a65c8f084fbcb5af22a5180e5a55b2da31fcc0e66df9dcd8900d5f893 |
| Windows 0.2.1 exe | 9,935,872 | 39aa599ae3330fe3320d6a2d46a1b804c1da0efefc18ff7387e5b41cdc6a5af1 |
| Linux 0.2.0 deb | 4,885,222 | 90a48c2f3d94c5e4ed86e0540a83995b9a9b6439f38018baf2fa685edf32b679 |
| Linux 0.2.1 deb | 5,441,170 | ba4a4b9b0692eab659fc34c380cc8dc1f85588aeaf6dc624d7c4d496aa835b09 |
| Linux 0.2.1 AppImage | 111,122,936 | 7a6e070c9b86d1ba7092c7e5b9bffeac617f0665381d031b71e331eb95a413cb |

## Windows 结果与修复

实际发现独立卸载删除程序/启动项后，留下自有分钟任务。已增加 GUI/数据库初始化前的
卸载清理入口，按完整定义确认任务归属，持有现有库写者锁并关闭意图；失败阻止 NSIS
删除程序。实际 COM 的 UserId 返回账户名而非 XML SID，按原生账户解析 SID 核对。
官方 NSIS 模板默认无路径核验的 Run 删除会删其他值，已仅移除该语句并保留上游许可；
模板来源固定 [tauri-cli-v2.12.0](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.0/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi)。

```powershell
npm run build:desktop
npm run test:install:windows -- --previous-installer desktop/src-tauri/target/release/bundle/nsis/LLMUsage_0.2.0_x64-setup.exe --previous-version 0.2.0
```

均退出 0，最终能力说明/删除前定义重查纳入包后，正式运行 12 项通过：旧包安装及两个真实快捷方式；旧程序导入；升级；
原生 IPC 注册任务/启动项；回滚；再次升级；卸载清理与数据保留；重装保留数据并保持
后台关闭；CLI 拒绝运行中的写者；真实 NSIS 收到失败中止并保留程序供重试；
再次卸载保留其他启动值/同前缀任务。始终为 1 个合成事件 / 15 token，schema 11。
结束回查产品注册、进程、快捷方式、启动项及自有/测试诱饵任务均无残留。

最新能力说明纳入包后再次完整 12 项退出 0，结果为
build/install-lifecycle/windows/1791209633008/result.json；最终包另存
build/install-lifecycle/artifacts/fuse-continuation/windows/，摘要为
fuse-continuation-windows-metadata.json。此前一次紧接进程退出的包操作返回 2，未确认原因；
控制脚本加入退出后短等待，正式完整往返通过，不把该等待写成产品缺陷修复。

## Linux 结果与边界

```sh
npm run test:install:linux -- \
  --previous-deb build/install-lifecycle/LLMUsage_0.2.0_amd64.deb \
  --deb build/install-lifecycle/LLMUsage_0.2.1_amd64.deb \
  --appimage build/install-lifecycle/LLMUsage_0.2.1_amd64.AppImage \
  --image localhost/llm-usage-lifecycle-cjk-verified:20261005 --skip-build-image
```

退出 0，8 组通过：五轮实际 deb GUI（旧版、升级、回滚、再次升级、重装）及一轮
AppImage 提取 GUI，每轮 5 项（前端、隔离发现、原生 IPC/SQLite、五页、平台状态），
合计 30 项；remove 与 purge 各验证数据保留，共 32 项断言。
旧/新版本均保留 1 个合成事件 / 15 token，未迁移失败、重复或丢失；卸载实际 exe 不存在。
Linux 如实报告 Windows 专属系统任务 unsupported。截图与结果在
build/install-lifecycle/linux/1791202022165/；结束本次容器移除，专属存储无运行容器。

初始截图中文为方框：原测试镜像只有拉丁字体，DOM/IPC 成功不足以认证文字可读。
已在可复用 Containerfile 添加官方 [Debian CJK 字体](https://packages.debian.org/trixie/fonts-noto-cjk)，
下载包从 [官方镜像列表](https://www.debian.org/mirror/list)中的镜像取得，
按 [官方下载页](https://packages.debian.org/trixie/all/fonts-noto-cjk/download)
核对 56,674,044 字节及 SHA-256
`f5dc28a754e17327d99f0a612134d92c8dd6187314ae967cb77f25df60860139` 后离线安装。
新字体覆盖检查真实拒绝原拉丁镜像；补齐字体后的全部 8 组退出 0，并人工复核
最终 deb/AppImage 截图中文可读。CDN 的慢速/重试下载已精确中止并以校验后包替代，
测试环境修复不改应用包；字节/摘要仍与上表一致。

映射 /dev/fuse 的默认 rootless 环境实际报 `fusermount: mount failed: Operation not permitted`。
后续对同一镜像/包的对比确认 WSL 内核已有 fuse，设备为 0666；单独映射设备失败，
显式添加 SYS_ADMIN 后，UID 1000 的 `--appimage-mount` 返回实际只读 FUSE 挂载，
默认 seccomp 保留。参数范围按 [Podman 5.4.2 文档](https://docs.podman.io/en/v5.4.2/markdown/podman-run.1.html#cap-add-capability)
核对，没有使用 privileged、rootful Podman 或修改宿主全局配置；sudo 已可用但无需调用安装。

可复用入口追加 `--appimage-mode fuse` 后，实际 GUI exe 位于已核对 FUSE 挂载点的
usr/bin/LLMUsage。挂载类型 fuse.LLMUsage.AppImage、ro/nosuid/nodev、
user_id/group_id=1000；应用四个 UID 均为 1000、CapEff=0、Seccomp=2。
关闭 GUI 后再次读取 mountinfo，确认自有挂载释放。最初 34 项结果仍在
build/install-lifecycle/linux/1791206172914/；缩放检查与最终包复验结果见下。
[提取替代方案](https://docs.appimage.org/user-guide/troubleshooting/fuse.html)仍独立保留先前 32 项通过结果。

继续补充 [GTK 3 官方 X11 缩放](https://docs.gtk.org/gtk3/x11.html)对照：
`--gui-scale 1|2` 分别设置 GDK_SCALE=1/2、GDK_DPI_SCALE=1；独立 Xvfb DPI=96，
屏幕 1440×1000 / 2880×2000。实际 WebKit devicePixelRatio 为 1 / 2，
两轮均无 CSS zoom（1）、逻辑窗口 1036×780；缩放 2 截图为 2072×1560。
每轮 8 组共 40 项（38 项 GUI/缩放/挂载/释放 + 2 项数据保留）退出 0，五页均无
文档级横向溢出，并人工复核中文截图。两轮结果分别在
build/install-lifecycle/linux/1791209706834/（1）与 1791209741062/（2），
Windows 摘要/截图为 fuse-continuation-scale-{1,2}-result.json 与对应 appimage-fuse /
upgrade-again.png；包摘要和专属容器残留 0 见 fuse-continuation-linux-metadata.json。
上表新 deb/AppImage 均参与这两轮完整生命周期；镜像摘要保持不变。
这是容器内真实 GTK 窗口缩放验收，不认证宿主显示设置、物理显示器或屏幕阅读器。

本轮 Debian 容器的包/GUI 结果不扩展为其他发行版、宿主完整桌面、
登录/注销、通知/托盘或原生开机启动。macOS 桌面与指定硬件不再列为本轮缺口。

## 最终检查

Windows `npm run verify` 退出 0：Rust 908（核心 810、应用 98；默认忽略 8），
前端 21、脚本 4；类型无错误/告警，Markdown 189 文件、资源、fmt、Clippy 与前端构建通过。
最终 Windows `test:headless` 11 项退出 0；卸载归属/未来 schema/写者锁单元 2 项与
最终 Clippy 均退出 0。Linux 最终 Clippy、Qwen 3 项 / OpenCode 8 项合同退出 0，
deb/AppImage 构建与新包回读两源真实载体退出 0，保持用量和更新后的能力说明。
两平台安装脚本 --help、JS/Python/PowerShell AST 与 shell 语法检查退出 0。
最后仅调整新增合同测试的临时库隔离：根 build 下独立临时目录与纳秒标签避免 PID
复用；Windows/Debian OpenCode 8 项、fmt/Clippy 再次通过，生产代码与受测包未变。
实际命令日志集中到 `build/install-lifecycle/final-*` 与 `fuse-continuation-*`，
源和制品检查结果分别保存；
没有提交、推送或发布，远端 CI 未触发。
