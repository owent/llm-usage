# 桌面客户端验证记录

本目录保存 M0–M7 的实际执行证据。规则见 [execution.md](../../design/desktop-usage/execution.md)：

- 只有产生实际证据后才创建记录文件，不预填"通过"；模板见 [TEMPLATE.md](TEMPLATE.md)。
- 每条记录包含命令、cwd、OS/运行时、锁定版本、退出码、测试数量、实际结果、失败及未执行项。
- 区分静态检查、本机真实测量、CI/WSL 与真实桌面验收证据，不互相替代。
- 脱敏 fixture 的中间产物放已忽略的 `build/desktop-usage-validation/`，本目录只引用其清单与结论，
  不复制私人数据或绝对个人路径。
