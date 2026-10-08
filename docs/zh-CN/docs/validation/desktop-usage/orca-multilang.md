# Debian Orca 十语言导航验收

<a id="debian-orca-navigation-acceptance-in-ten-languages"></a>

日期 2026-10-06；十语言测试先在 [Cline 阶段制品](cline-container-sample.md) 通过，
当前 [OpenClaw 阶段制品](openclaw-container-sample.md) 已重建并再次完整复验通过。
要求见 [语言](../../design/desktop-usage/i18n.md)与
[安装生命周期](../../design/desktop-usage/installation-lifecycle.md)。

WSL Debian 13.7、rootless Podman 5.4.2，镜像 ID
4575f4ba7437ce291ce8e1633292f232ad5899d1551c06ed20aced61f3fc212d。
Orca 48.1、pyatspi 2.46.1、Speech Dispatcher/espeak-ng 0.12.0；独立 D-Bus 与
ALSA null。网络关闭、无宿主挂载、默认 seccomp；普通 GUI 用户 UID 1000/CapEff=0，
FUSE 容器仅显式添加 SYS_ADMIN。

完整命令沿用 `npm run test:install:linux -- --screen-reader --appimage-mode fuse`
及既有旧/新 deb、当前 AppImage、固定验收镜像参数，退出 0。实际安装/升级/回滚/
卸载/重装、GTK/WebKit IPC 与只读 FUSE 挂载/退出释放仍为 9 组、47 项。
当前成功根为 WSL build/install-lifecycle/linux/1791289583346，之前的
1791287694583 保留 Cline 包范围；转存摘要和原始
Orca 输出在根 build/plan-final-push/orca-multilang-proof.json、orca-result.json、
orca-debug.log、linux-final-proof.json。

十种语言 zh-CN、zh-TW、en、ja、ko、es、fr、de、pt-BR、ru 均通过产品设置
表单保存，回查持久化值与 HTML lang。每种语言的五个主导航按钮共 50 个名称均以
原生 Tab 到达，逐次只检查该次按键之后的 AT-SPI 焦点日志与 Orca 语音输出，再用
原生 Enter 核对实际页面标题；英文品牌名称原有检查保留。未注入产品名称或复用
前一语言的语音证明后一语言，结束恢复英文状态。

首次根 1791287290747 错把导航序号 3 当作设置而找不到语言表单，按真实 App.svelte
顺序修正为 4；第二次根 1791287460071 已产生中文“总览”的真实焦点/语音，但断言
套用了英文详细日志格式。改为实际 `FOCUS MANAGER: Locus of focus is` 格式后完整
重跑通过；两次首失败日志保留，不能列作产品辅助技术故障。

这项结果证明十语言导航名称到达焦点与语音管线，不能据此核验各语言发音、物理可听性、
其他页面控件的完整操作、Windows Narrator/NVDA 或宿主完整桌面。
