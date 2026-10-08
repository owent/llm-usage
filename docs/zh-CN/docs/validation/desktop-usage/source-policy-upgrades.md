# 来源规则升级与 SDK file 接入

<a id="source-rule-upgrades-and-sdk-file-integration"></a>

日期：2026-10-06；版本 0.2.1，基于 `c2c8f6f1c44f3dc9e398842d9dca2c93da76680c`
的未提交工作树。cwd 为 D:/workspace/projs/github/owent/llm-usage；未提交、推送或发布。
依据：[数据规则](../../design/desktop-usage/data-contract.md)、
[遥测配置](../../design/desktop-usage/copilot-otel.md)、
[认证要求](../../design/desktop-usage/receiver-auth.md)。真实样本环境、官方下载摘要与
原始独立统计见 [容器来源](container-sources.md)，安装基线见 [生命周期](installation-lifecycle.md)。

本文保留 Qwen/OpenCode/凭据/Orca 阶段的受测制品和结果；后续八个 M8 客户端的
真实样本、规则纠正及最终 Windows/Linux 成品复验见 [M8 容器样本](m8-container-samples.md)。

<a id="opencode-per-record-versions-and-old-processing-positions"></a>

## OpenCode 逐记录版本与旧处理位置

旧实现按库内最高 session.version 选择整个文件依据，空会话也可能影响判断；
50,000 行上限与 60 秒重叠窗口组合可能跳过尚未处理的旧行。现在只有具有
step-finish 用量记录的会话参与文件状态，每条事件使用其所属会话版本；
仅真实核验过的 1.18.34 注册为 known_version，其他版本保留 latest_fallback。
游标含更新时间、稳定行 ID 和未完成窗口起点；完整有效扫描才完成规则标记，
未知版本和真实坏行状态跨增量窗口保留。SQL 类型错误不阻断其他有效记录。

旧摘要中的 parse_basis 曾未参与元数据归一化，导致 parser/basis 升级仍发生冲突；
现在比较完整 canonical/legacy 摘要，仅允许解析依据变化。token、模型、质量、
归属和修订变化仍进入原有冲突选择规则，诊断历史不删除，汇总与检查点同事务。

OpenCode 11 项、元数据升级 10 项与 MiMo 共享读取回归通过；覆盖真实注册表 discover、
未变化的旧游标/完整旧摘要、混合版本、空会话、同毫秒分页、重复读取、坏行恢复及
检查点失败回滚。真实 1.18.34 主循环仍不包含默认标题请求；不从差额补造事件。

## Qwen 0.25.0 SDK file

按固定官方源码 `6788c035698a0ada471c958d1e789e96c6cddd9b` 与真实 file 核验。
FileExporter 输出连续 pretty JSON；`resource._rawAttributes` 是键值对数组，
LLM span 身份位于 `_spanContext`，kind=0。解析器只接入该版本逐次
qwen-code.llm_request，不叠加 api_response 日志、HTTP span 和指标。
对象、字节、数量及时间均有界，半对象与超限保留对象起点；错误对象不掩盖后续有效调用。

真实主循环 input/output 为 10,226/2，后台自动记忆为 5,639/556，缓存读 3；
API response、LLM span 与 CLI 独立合计 2 次、16,423 token。
SDK 未输出 cache write 或推理是否已包含在 output 的供应商依据：不补 cache write 为 0，
只有已知 reasoning=0 才派生总量；非零推理保留 total 未知。同 session 不证明父子调用。

SDK 与原生按主机/用户/会话/本地日择一，trace+span 副本在同主机/用户内择一。
原生和副本保存在库中并记录排除原因；SDK 其他版本、未知归属/会话保持隔离。
已保留的原生来源/日归档分区阻止重叠 SDK，明细已删除也不叠加。范围设置、排除、事件、
检查点和汇总同事务；清空清理该范围身份。部分覆盖进入历史覆盖提示，不降级合法来源健康。

8 项设计要求检查覆盖两个导入顺序、重复扫描/副本、跨主机与本地日、归档分区、未知归属/版本、
非零推理、半对象/超限、坏对象/坏 token 与事务回滚。应用只读 exporter 检查和
已核验应用管理目录定向发现也通过，其他 supplemental 输出继续隔离。

追加已归档 SDK 的新目录副本回归，首次退出 101：归档统计 2 次/16,423 被新副本
再加为 4 次/32,846。已有逐次 trace/span 在明细删除后无法用于副本择一，现核对
同主机/用户/本地日的完整 SDK 来源归档分区，不能证明不重叠的新输出保留
qwen_sdk_partition_sealed 排除及覆盖提示，不改归档贡献。另沿实际 enforce_retention
执行清理、重开数据库，再扫描迟到原生/SDK 副本，旧统计保持 2 次/16,423、明细 0；
持久化清理下限继续生效。测试中原生手工文件不符合 discover 布局的失败已纠正为
真实 .qwen/tmp/project/chats 路径，不归为产品缺陷。最终上述 8 项均退出 0。

新 deb 在无网络 rootless Podman 读取保存的原生及 SDK 后，实际有效统计为
2 次、input 15,865、output 558、cache read 3、total 16,423；原生排除 1 条，
另一目录的 SDK 副本排除 2 条，重扫不新增用量。目标 Qwen/OTel 来源 health=ok。
初次手工副本目录也产生其他适配器的格式诊断，不能把这些诊断算成 SDK 解析失败。

<a id="source-routing-defects-found-by-the-actual-package"></a>

## 实际包发现的来源路由缺陷

实际新包的 Qwen 手工 `.qwen` 根被 Claude 的共享 type 探测误认领，
12 个来源、0 事件、8 诊断；通过 QWEN_RUNTIME_DIR 则为 1 条/10,228。
按允许字段核对真实首条用户记录及固定 ChatRecord 源码，确认 Qwen message.parts 与
usageMetadata 和完整身份。该明确布局/形状现在只定向 Qwen；目录名或共享 type 单独不能核验格式。
整个旧来源无事件/日/周期/原生汇总时，恢复错误归属并释放该文件的错误检查点；
保留历史诊断、启用状态和原始字节。有历史贡献时不迁移。实际修复包的手工根为
1 个 Qwen 来源、1 条事件、无诊断。整个注册表的旧消费游标、坏行和保留边界回归通过。

Goose 对直接手工 SDK JSON 文件先打开 SQLite，随后查询报 NotADatabase 并使该来源失败。
现先核对 16 字节 SQLite magic；非 SQLite 仍保留 unknown_format 诊断，允许 OTel
按明确 SDK 格式认领，重复不再跨适配器登记。既有 M8 SQLite 设计要求检查和专项回归通过。

<a id="native-windows-credential-failures-and-correction"></a>

## Windows 原生凭据失败与修正

再次复现建立凭据返回 credential_store_unavailable。测试诊断仅含操作、数值错误码
及缺失/不同/失败状态；没有凭据、目标或认证头。实际观察为 CredWriteW 成功，
紧接的 CredReadW 缺失，10 ms 后能读取完全相同的内容，50/100 ms 后仍相同。
修正仅对变更流程的缺失读取额外等待最多 50 ms；不同内容或读取错误立即拒绝，
认证请求仍只读取一次。失败回收与撤销只比较完整自有内容，删除后确认缺失；
Windows 仍读到相同内容时最多再等待 50 ms，外部替换/错误/超限报告失败，不再次删除。

写入修正后两组各 100 轮并行测试通过；此前一次跨进程撤销后仍存在凭据，
另一次在加入删除确认后的第 60 轮复现，10/50/100 ms 后仍可认证。
子进程成功退出不证明已处理输入，增加 --nocapture 与明确处理确认后，
两组各 100 轮、两线程并行测试全部退出 0，确认子进程已处理完整绑定。
此前撤销异常的根因仍未确认，不能据后续通过关闭该异常。首次失败与最终结果
分开保存在根 build/plan-final-push/。
按失败日志时间定位的 4 个确切测试目录中，2 个完整自有绑定被精确回收，
同时两线程额外 40 次建立/撤销通过，自有残留 0；其他来源凭据未清理。
最终回查新增失败在内的 5 个确切目录，无待回收绑定，再做 40 次往返仍全部成功、
自有残留 0。官方 [CredWriteW](https://learn.microsoft.com/en-us/windows/win32/api/wincred/nf-wincred-credwritew)
与 [CredReadW](https://learn.microsoft.com/en-us/windows/win32/api/wincred/nf-wincred-credreadw)
说明没有给出该时序的原因；不将观察扩大为系统缓存或线程安全结论。

<a id="actual-linux-screen-reader"></a>

## Linux 真实屏幕阅读器

在既有固定 Debian GUI 镜像上，增加官方发行包 Orca 48.1-1+deb13u2、Speech Dispatcher
0.12.0-5/espeak-ng、pyatspi 2.46.1-1 与 xdotool；镜像 ID
4575f4ba7437ce291ce8e1633292f232ad5899d1551c06ed20aced61f3fc212d。
普通 UID 1000、独立 D-Bus/X11、默认 seccomp、无网络/宿主挂载，音频为 ALSA null。
调试参数依 [Orca 官方说明](https://orca.gnome.org/debugging)和镜像内实际命令核对。

初次真实焦点事件中，导航按钮 name 为空，Orca 只读 button；只在临时 DOM 注入
显式名称的诊断探针能够读出五项。成品据此为导航/品牌链接补随语言更新的 aria-label，
窄侧栏隐藏文字仍有名称。语言准备走实际设置表单并保存，不由后台 IPC 模拟前端更新；
未识别的语言值和包含设置子导航的过宽选择器失败均保留为测试缺陷。

可复用 test:install:linux 增加 --screen-reader，仅在当前包重装后运行该独立轮次。
原生 Tab/Enter、实际 AT-SPI button 焦点、五页标题和真实 speech output 一起核对；
不注入名称。Orca 48.1 的 debug.py 使用缓冲写入，正式检查曾在语音已产生但文件
尚未冲刷时误报。安装版 /usr/bin/orca 仅以 open(..., 'w') 打开该调试文件；
测试 launcher 对这一行精确设置 buffering=1，其他事件/语音代码不变，升级后正文
不匹配即拒绝运行。逐项等待实际语音并回查 AT-SPI 焦点；PID 以父进程/UID 核对，
Orca 会自行修改进程名，不能凭启动命令行认领。正常终止未在测试期限内完成的
失败保留；最终自有阅读器/语音进程采用有界 TERM/KILL/wait 回收，不核验 Orca 的
退出可靠性。未改成品辅助技术默认环境。

当前包正式退出 0：五个导航的 AT-SPI button 名称与 speech output 分别为
Overview、Trends、Sources、Details、Settings，原生 Tab/Enter 后实际页面标题一致；
品牌链接读为 LLM Usage。截图中为窄侧栏，仅图标可见，显式名称仍完整。
结果/日志/截图保存在 build/plan-final-push/，容器完整生命周期结果在
WSL build/install-lifecycle/linux/1791264897354/。该结果不核验物理音频可听性、全部页面
控件、十语言屏幕阅读器或 Windows 辅助技术。

<a id="stage-checks-and-remaining-coverage"></a>

## 本阶段检查与待补边界

最终 npm run verify 退出 0：Rust 927（核心 826、应用 101，默认忽略 8）、
前端 21、脚本 4；Markdown 191 文件、资源、Svelte 0 错误/告警、fmt、
Clippy -D warnings 和前端构建通过。Debian 全 workspace Rust 924
（核心 826、应用 98，默认忽略 9），显式系统凭据 6 项通过，keyring 已回收。
Windows 当前 release 的无界面 11 项、接收器 8 项、自有凭据残留 0，以及 NSIS
生命周期 12 项通过；NSIS 结果为 build/install-lifecycle/windows/1791264668246/。
一次安装前置检查因并行运行本任务桌面测试被正确拦截（0 项），待桌面进程退出后
串行复验通过，不能归为产品安装失败。最终 release 原生桌面 17 项、20 次首屏
P95 747.1 ms 退出 0，结果为 build/plan-completion/native/1791264943840/。
浏览器 Edge 模拟 IPC 的语言/主题/选区/筛选/分页与窄布局回归退出 0。
Linux 当前包完整 deb/FUSE/GTK/Orca 生命周期 9 组、47 项通过；此前缩放 2
的 40 项结果仍独立保留，不声称本轮重复该比例。最终 deb 回读 Qwen 原生/SDK/
直接手工文件副本为 2 次/16,423、原生排除 1 条/SDK 副本排除 2 条，目标来源正常；
初次其他格式探测的 1 条诊断保留。OpenCode 两份原始库回读各 1 次/299，缓存读
3/0、health=ok、parser=opencode-step-finish-parts-2、known_version，重扫不新增用量。

| 本阶段实际制品 | 字节 | SHA-256 |
| --- | ---: | --- |
| Windows exe | 9,989,120 | 5b3e16b2d9be5d4f25bd4886ea521a109a7d5f06ee6ddd06f2c22b23fb1e3073 |
| Windows NSIS | 3,941,612 | e34aa3eebade224e865dac185a157081634a7035e2244971fda2bae2d3060076 |
| Debian deb | 5,466,678 | 69abe4b2463459d2969bf582d874d1e82796b14e77e5823f4ea5f38dc1adbd46 |
| Linux AppImage | 111,143,416 | 76b33bc5500672ba3dd6f0d115bd874fc453eac69b3283a7552da76b25137457 |

本次 Windows/Linux 构建均退出 0，旧 0.2.0 包与镜像摘要仍见安装历史记录。
所有一次性库、脚本、原始核验记录与日志位于根 build/；规则和 Skill 只同步相关设计要求。

仍待其他产品/版本非空真实样本、OpenCode 标题逐次记录、Qwen 云端/主循环缓存命中，
其他 exporter 认证/重传/采样/父子 span、Copilot 新 CLI/JetBrains 真实导出及完整出站审计。
macOS 桌面与特定硬件已取消要求；macOS 原生凭据、真实辅助技术、宿主注销/登录及
远端 CI/发布另有环境或授权依赖，不用本地构建和容器退出代替。
