# 桌面客户端验证记录

当前结论见 [最新验收](current-acceptance.md)，清理取消、原生语言/键盘/缩放、无 GUI
任务、百万/千万查询及增量刷新见 [原生与规模验收](plan-finalization.md)。
查询/筛选加速、Windows 后台控制、遥测限制及本轮资源复测见
[剩余计划执行](plan-execution.md)。
Linux/macOS 凭据实现、Windows/Linux 原生验证及 Linux 制品见
[跨平台继续执行](platform-auth-continuation.md)。
Windows NSIS 与 Debian Podman 真实 GUI/包生命周期见 [安装验收](installation-lifecycle.md)；
官方 Qwen 0.25.0 与本地模型实际 token/后台覆盖边界见 [容器来源](container-sources.md)。
本目录保存各阶段实际执行记录，规则见 [execution.md](../../design/desktop-usage/execution.md)：

- 只有完成实际检查并记录结果后才创建记录文件，不预填"通过"；模板见 [TEMPLATE.md](TEMPLATE.md)。
- 每条记录包含命令、cwd、OS/运行时、锁定版本、退出码、测试数量、实际结果、失败及未执行项。
- 区分静态检查、本机真实测量、CI/WSL 与真实桌面验收结果，不互相替代。
- 脱敏 fixture 的中间产物放已忽略的 `build/desktop-usage-validation/`，本目录只引用其清单与结论，
  不复制私人数据或绝对个人路径。

包含 2026-09-25 起九个提交的审查、归档统计、UI 改进及本轮验证见
[2026-09-27 审查记录](review-2026-09-27.md)。

应用图标、资源预览、Windows 图标嵌入与 LFS 迁移的实际结果见 [静态资源验证](static-assets.md)。
最近两次提交的缺陷、修复、升级兼容与回归结果见 [M0/M1 审查](m0-m1-review.md)。
M2 的 Codex 适配器验收见 [m2a-codex.md](m2a-codex.md)；pi/oh-my-pi/Claude/Gemini/Qwen
五源恢复验收见 [m2bc-resumed.md](m2bc-resumed.md)（中断经过见
[m2bc-suspended.md](m2bc-suspended.md)）；适配器目录化迁移、未知版本兼容尝试与
Codex 逐版本 fixture 见 [m2d-layout-versions.md](m2d-layout-versions.md)。
M3/M4 的 kilo/zcode/kimi 双源适配器见 [m34-kilo-zcode-kimi.md](m34-kilo-zcode-kimi.md)；
cline/dsh/hermes/openclaw 基于文档或源码实现的适配器见 [m3-doclevel-cmdh.md](m3-doclevel-cmdh.md)。
M7 部分的 release 构建与包体测量见 [m7-build-partial.md](m7-build-partial.md)。
M1a 来源身份与存储分区见 [m1a-provenance.md](m1a-provenance.md)；
M6 主体功能（界面/刷新/调度/导出/headless/i18n）见 [m6-desktop-core.md](m6-desktop-core.md)，
其中记录了 Roaming 占位文件事故与 kilo/zcode 适配器中断状态。
