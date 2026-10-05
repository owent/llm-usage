# 安装生命周期验收

2026-10-05 用户已授权 Windows 本机与 WSL/Debian Podman 安装验收，
不要求 macOS 桌面或指定硬件。遵守 [平台合同](platform-ci.md)与
[调度合同](scheduling.md)，命令和实际结果集中到验证记录。

## 环境与载体

- Windows 使用真实当前用户 NSIS 包，先只读核对已有安装、进程、产品注册键和快捷方式。
  本轮无现有安装，使用仓库根 build/ 内独立安装目录、应用数据和合成来源。
  对照已有旧版本包和当前包，记录包 SHA-256、实际安装版本与程序路径。
- Linux 使用任务独立的 rootless Podman 存储、Debian 官方镜像的固定 digest、
  独立用户/目录/显示/D-Bus。实际安装 deb 并通过 GTK/WebKit WebDriver 验证 GUI/IPC。
   容器软件包升级/降级、卸载/重装与 AppImage 提取/FUSE 分别记录，保留数据和重扫幂等。
   测试镜像包含 CJK 字体，GUI 前检查代表性中文字符覆盖，并复核实际截图；DOM 有文字
   不证明字符可读，缺字不能记为完整显示通过。
- 安装软件或容器不会自动产生用量。产品写入的非空本地载体、模型返回的真实 token、
  官方脱敏样本及模拟接口合成用量分别认证；镜像和样本均不得包含账户密钥。

## 行为要求

安装后核对注册信息、快捷方式、实际程序、IPC 和隔离 SQLite。
升级保留用户数据、来源配置及已同意的系统任务，并回查任务执行路径。
回滚先核对数据库兼容；拒绝回滚或需一致备份恢复时如实记录，不能删除数据库凑通过。
卸载默认保留用量/配置，仅移除当前安装对应的自有任务与启动项；其他安装、
其他数据目录或不匹配定义不得因名称前缀相同被删除。

Windows 卸载前的清理入口必须在建立 GUI、数据库迁移和采集器之前分流。
任务按当前可执行文件、名称/参数/身份依据核验，失败回查并阻止卸载删除程序。
升级调用的临时卸载不删除任务；卸载不自动恢复用户 IDE 配置或删除其他数据。
清理操作不能靠任务存在、退出 0 或删除后的不可查询替代完整定义核对。
当前实现要求自有 hash 名称、description、单一 Exec、绝对 data-dir 参数、当前用户 SID、
交互登录和最低运行权限。COM 返回账户名时先解析 SID；快照完整 XML，删除前重查。
只打开任务引用的既有库，用现有单写者锁关闭后台意图与待执行请求，不建库、不迁移；
卸载默认保留其他设置、用量及 schema。任务删除后查 absence；重装不自动恢复后台同意。

NSIS 模板固定官方 tauri-cli-v2.12.0，并保留 MIT 许可；唯一正文差异是移除默认
不核对路径的 Run 删除语句，改由当前安装的清理入口核验归属。升级工具版本须复核
模板差异。PREUNINSTALL 仅独立卸载调用 `--uninstall-cleanup`，失败在程序删除前中止。

登录/注销与进程退出分开；不自动注销宿主 Windows/WSL 用户。仅停止本任务进程与
容器，不全局停止 WSL、删除个人镜像或覆盖已有安装。新脚本须提供无副作用 --help。

## 可复用入口

均在仓库根运行，先构建并准备实际旧/新包；`--help` 不安装、不创建容器。

```powershell
npm run test:install:windows -- --previous-installer <旧NSIS路径> --previous-version 0.2.0
```

Windows 默认当前包来自 release/bundle/nsis，可用 `--installer` 指定。需要无已有
LLMUsage 安装、进程或启动项；快照已有快捷方式并保留。NSIS 控制器超时 180 秒，
测试创建当前用户任务/启动项并精确回收；独立安装、数据、结果在 build/install-lifecycle/windows/。
系统任务须持久化 `manual_roots_only=true` 与合成手工根，不能依赖父进程环境。

```sh
npm run test:install:linux -- --previous-deb <旧deb路径> --deb <当前deb路径> \
  --appimage <当前AppImage路径> --appimage-mode extract
```

Linux 需要 Node 22+ 与 rootless Podman，默认构建固定 Debian GUI 镜像；构建可联网，
运行容器无网络/宿主挂载，文件用 cp 传入。可用 `--image`、`--skip-build-image` 复用
已记录摘要的任务镜像；`--storage-dir` 仅接受仓库根 build/ 内专属目录及空 registry auth。
构建限时 30 分钟、单次控制命令 5 分钟，GUI 请求最长 40 秒；成功/失败收集结果后
停止并移除本次容器，镜像/缓存留在专属存储。输出在 build/install-lifecycle/linux/。
`--gui-scale 1|2` 在独立 X11 显示设置 GTK 窗口缩放和相应屏幕尺寸；以 WebKit
devicePixelRatio 与实际截图核对，逐页检查横向布局。该验证与 CSS 页面缩放、
宿主显示设置和屏幕阅读器分开记录，不改成品默认环境。
`--appimage-mode fuse` 显式为本次 rootless 容器映射 /dev/fuse 并添加 SYS_ADMIN，
保留默认 seccomp。GUI 仍以普通用户运行，应用有效能力为 0；该能力供容器内
fusermount 完成挂载，不使用 privileged 或关闭 WebKit 沙箱。核对实际 FUSE 类型、
只读挂载、挂载所属用户与 AppImage 内的可执行文件，退出后核对挂载已释放。
无此能力的默认容器挂载失败不能归因为 WSL 缺少 FUSE；先逐层检查设备、内核、
用户命名空间及容器权限。sudo 可用于安装缺失组件，但不自动改宿主全局配置。

当前包与实际结果见 [安装验收](../../validation/desktop-usage/installation-lifecycle.md)。
