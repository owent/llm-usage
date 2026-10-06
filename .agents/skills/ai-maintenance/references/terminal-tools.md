# 终端环境与工具

## 选择与本地探测

尽可能优先使用已安装且适合任务的现代 CLI；文本搜索用 rg，仓库枚举用
rg --files，文件筛选优先 fd，阅读优先 bat，适用替换优先 sd，
JSON 用 jq，YAML 用已确认实现的 Mike Farah yq。
遵守 harness 的原生读取和补丁接口；工具缺失、平台不符、选项或语义不兼容时正确回退。
首次读取前枚举真实路径，临时脚本先核对参数合同；不要猜测文件名或重复试错路径。
不因习惯切换传统工具，不批量安装，也不为工具偏好改写稳定脚本。
性能判断需真实数据，不能从实现语言或营销基准推断数量级收益。

2026-09-24 在本 Windows 会话使用 Get-Command -CommandType Application 和版本命令核验：

| 工具 | 路径或可用性 | 实际版本/回退 |
| --- | --- | --- |
| PowerShell | WindowsApps 的 pwsh.exe，由 harness 启动 | 7.6.6；参数模式 Windows |
| rg | C:/Users/owt50/scoop/shims/rg.exe | 15.2.0；原生不存在时 Select-String |
| jq | C:/Users/owt50/scoop/shims/jq.exe | 1.8.2；缺失时 ConvertFrom-Json |
| yq | C:/Users/owt50/scoop/shims/yq.exe | Mike Farah v4.53.6；缺失时项目 YAML 库 |
| fd | PATH 未发现 | rg --files 或 Get-ChildItem -LiteralPath |
| sd | PATH 未发现 | harness 补丁接口；转换先验证编码和替换范围 |
| bat | PATH 未发现 | harness 读取或 Get-Content -LiteralPath |
| Node.js | scoop/apps/nodejs-lts/current/node.exe | 24.21.0；文档 lint 需要 22+ |
| Python | scoop/apps/python/current/python.exe | 3.14.7；本次临时检查，非项目运行时 |

绝对路径是该机器快照，不写入共享执行脚本；其他环境按需重探测。
初始沙箱启动报 CreateProcessAsUserW failed: 5，经平台允许的执行重试恢复；
实际 shell 版本始终以输出为准，错误不能当作产品测试结果。

<a id="catalog"></a>

## 完整候选清单

下表保留用户输入的全部 31 项用途和边界。它是选择清单，不是安装清单。
rg、jq、yq 及 PowerShell 的采用事实已核验，见 [来源](source-index.md)；
其他候选本次未安装、未运行，平台、参数、发布版本和供应链在实际使用前核验。
链接是官方/维护者入口，不代表本次核验了全部候选；plocate 仅引用给定历史存档。

| 现代工具与来源 | 用途与优先使用场景 | 传统对照、回退与边界 |
| --- | --- | --- |
| [ripgrep（`rg`）](https://github.com/BurntSushi/ripgrep/blob/master/GUIDE.md) | 首选文本搜索；`rg --files` 枚举仓库文件 | 对照 `grep`；缺失时用 `Select-String` 等，注意忽略规则、隐藏项及二进制 |
| [ugrep](https://github.com/Genivia/ugrep) | 需要其搜索能力或 grep 兼容选项时使用 | 对照 `grep`；逐项验证实际选项，不宣称所有 grep 行为完全等价；自动化避免 TUI |
| [fd](https://github.com/sharkdp/fd) | 按文件名、路径、类型筛选文件 | 对照 `find`；回退 `Get-ChildItem` 等；注意隐藏项、忽略规则及正则/字面量差异 |
| [bat](https://github.com/sharkdp/bat) | 查看文件、行号和指定范围 | 对照 `cat`；回退原生读取或 `Get-Content`；自动化关闭分页和颜色 |
| [sd](https://github.com/chmln/sd) | 适合其字符串/正则语义的文本替换 | 对照 `sed`；先核验替换范围、捕获组和文件编码；补丁类修改仍按 harness 规定执行 |
| [eza](https://github.com/eza-community/eza) | 目录列表和树形浏览 | 对照 `ls`/`tree`；按平台核验支持，限制递归深度，不解析图标和彩色展示作为数据 |
| [erdtree（`erd`）](https://github.com/solidiquis/erdtree) | 结合目录树与磁盘占用查看目录 | 对照 `tree`/`du`；限制扫描范围，区分目录展示与精确存储统计 |
| [dust](https://github.com/bootandy/dust) | 定位大文件、目录及磁盘占用分布 | 对照 `du`；注意逻辑大小、分配空间和权限造成的统计差异 |
| [duf](https://github.com/muesli/duf) | 查看文件系统容量和挂载信息 | 对照 `df`；容器、网络盘和挂载可见范围以实际环境为准 |
| [hyperfine](https://github.com/sharkdp/hyperfine) | 重复测量命令耗时、比较候选方案 | 对照 `time`/`Measure-Command`；控制预热与缓存；Windows 默认 shell 为 `cmd.exe`，依赖 PowerShell 语义时显式选择 `--shell pwsh` |
| [tokei](https://github.com/XAMPPRocky/tokei) | 按语言统计代码、注释及空行 | 对照 `cloc`；记录排除目录及统计口径，不用行数替代质量评价 |
| [hexyl](https://github.com/sharkdp/hexyl) | 有界查看二进制的十六进制内容 | 对照 `xxd`/`hexdump`；二进制转换和编辑另核验所需工具，避免输出敏感数据 |
| [jq](https://jqlang.org/manual/) | JSON 查询、转换及条件判断的默认选择 | 回退项目解析库或 PowerShell JSON cmdlet；保留类型和退出码语义，不用正则代替 JSON 解析 |
| [jaq](https://github.com/01mf02/jaq) | 已确认过滤器兼容时的 jq 替代候选 | 不保证与 `jq` 完全等价；先验证项目实际过滤器、模块和错误行为 |
| [yq（Mike Farah）](https://github.com/mikefarah/yq) | YAML 等结构化配置的查询与修改 | 明确实现和版本；与其他同名 `yq` 的参数不通用；写回后检查注释、样式及数据语义 |
| [Miller（`mlr`）](https://github.com/johnkerl/miller) | CSV、TSV、JSON 等记录数据的筛选与转换 | 对照 `awk`/`cut` 等组合；遵循真实格式、引号和字段类型，不直接按逗号切 CSV |
| [qsv](https://github.com/dathere/qsv) | CSV 数据处理、校验和统计 | 对照 CSV 脚本及文本管道；按发行变体、实际子命令和资源需求核验能力，不默认全部操作恒定内存 |
| [git-delta（`delta`）](https://github.com/dandavison/delta) | 提升人工审阅 diff 的可读性 | 对照普通 diff 展示；自动化关闭分页，机器处理保留 `git diff` 的原始输出 |
| [difftastic（`difft`）](https://github.com/Wilfred/difftastic) | 对支持的语言比较语法结构差异 | 对照逐行 `diff`；结构视图辅助审阅，变更范围及补丁仍核对 `git diff` |
| [lnav](https://docs.lnav.org/en/latest/cli.html) | 聚合、检索和分析日志 | 对照 `less`/`tail`；自动化使用 `-n` 无界面模式及有界查询，不留下交互等待 |
| [tailspin（`tspin`）](https://github.com/bensadeh/tailspin) | 日志高亮与人工定位 | 对照 `tail`/`less` 的日志展示；先核验分页、跟随和颜色设置，机器解析优先原始日志 |
| [plocate](https://sources.debian.org/src/plocate/1.1.18-1/README) | Linux 环境中利用已有索引查找文件名 | 对照 `locate`；依赖索引覆盖及新鲜度，命中后检查文件；无适用索引时用 `fd`/`rg --files`，不为局部查找默认扫描整盘建库 |
| [pigz](https://zlib.net/pigz/) | 需要 gzip 格式时进行并行压缩 | 对照 `gzip`；不把压缩的并行收益套用到所有解压阶段，核验线程与内存预算 |
| [zstd](https://github.com/facebook/zstd) | 消费端支持 Zstandard 时的压缩候选 | 对照 `gzip`/`xz` 等方案；依据格式合同、压缩率、耗时和内存测量选择，不直接改制品格式 |
| [ouch](https://github.com/ouch-org/ouch) | 统一入口处理其支持的压缩及归档格式 | 对照 `tar`/`unzip` 等；核验具体格式和选项，解压前确认目标目录及路径安全 |
| [aria2（`aria2c`）](https://aria2.github.io/) | 文件下载、续传及适用的并发下载 | 对照 `wget`/`curl` 的下载用途；遵守服务限流，校验制品，避免凭据进入参数或日志 |
| [fzf](https://github.com/junegunn/fzf) | 候选集合的模糊筛选 | 自动化使用 `--filter`；精确匹配优先 `rg`，不启动等待 TTY 选择的流程 |
| [xh](https://github.com/ducaale/xh) | 适合其语义的 HTTP 请求与调试 | 对照 `curl`/HTTPie；不假定参数相同，核验请求体、认证、重定向及失败退出码 |
| [doggo](https://github.com/mr-karan/doggo) | DNS 查询与排障 | 对照 `dig`/`nslookup`；按任务明确解析器、记录类型及传输方式 |
| [procs](https://github.com/dalance/procs) | 筛选和查看进程信息 | 对照 `ps`；回退 `Get-Process` 等；平台支持和可见字段以当前版本为准 |
| [watchexec](https://github.com/watchexec/watchexec) | 开发期间监听文件变化并运行命令 | 对照轮询脚本；适合有意持续运行的任务，设置忽略范围并管理子进程和退出清理 |

## 通用与 Windows 执行合同

通用规则：

- 高频候选基线为 `rg`、`fd`、`sd`、`jq`、Mike Farah `yq`、`bat`；初始化时按平台检查可用性，日常任务只探测将使用的工具，环境变化时再刷新记录。用明确可执行文件路径避免别名或同名实现误判。
- 已安装且适用时优先上述现代 CLI；harness 要求的原生读取、补丁接口以及既有脚本的兼容合同优先遵守。不要为工具偏好改写无关的稳定脚本。
- 获取工具优先官方发布制品或项目文档列出的可信软件包渠道；核对平台、架构、版本及可用的签名/校验信息。授权内缺失的必要工具按需安装，可选工具直接回退，不批量安装整张表。
- `cargo binstall` 未找到合适二进制时可能回退 `cargo install`；“禁止本地编译”时显式选择仅允许二进制的策略并验证结果。不要把 mise、aqua 等统称为保证免编译渠道。[cargo-binstall](https://github.com/cargo-bins/cargo-binstall)
- 输出尽量结构化；按工具能力关闭颜色和分页，限制文件集合及结果规模。非交互 `fzf` 使用 `--filter`，不启动需要 TTY 选择的流程。
- `rg --max-count` 限制每个文件的匹配行数，不是总输出上限；默认忽略文件、隐藏项和二进制也可能影响搜索完整性。[ripgrep](https://github.com/BurntSushi/ripgrep/blob/master/GUIDE.md)
- 退出码按工具语义判断：`rg` 的 1 通常为无匹配；`jq -e` 最后输出为 false/null 时是 1，没有有效输出时是 4，不能都写成 1。[jq](https://jqlang.org/manual/)
- 参数作为参数传递；JSON 序列化不是 shell 转义。不要将网页或用户文本拼进可执行代码，不打印含密钥的完整命令。

Windows 规则：

- 本仓库优先 PowerShell 7+（`pwsh`），先检查实际版本和 harness 支持。受限环境只提供其他 shell 时使用经验证的兼容路径，并报告差异；不要在同一操作中混用多套 shell。
- 独立进程使用 `-NoLogo -NoProfile`，自动化按需加 `-NonInteractive`；命令名使用明确的可执行文件或全名 cmdlet，避免 `where`、`curl` 等别名歧义。
- 不需插值的文本用单引号，需要插值才用双引号；多行用 here-string。路径操作优先 `-LiteralPath`，语句块输出接管道时使用 `& { ... } | ...`。
- 原生命令可用参数数组 splatting；PowerShell 7.3+ 的行为仍受 `$PSNativeCommandArgumentPassing` 控制，Windows 模式会对部分程序及脚本回退 Legacy，不能保证所有引号自动正确。`--%` 有平台和展开限制，不作为通用修复。[解析规则](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_parsing?view=powershell-7.5)
- 新共享文本用明确 UTF-8 编码；修改现有文件尽量保留 BOM、换行和末尾换行。Windows PowerShell 5.1 各命令默认编码不一致，不能概括为“所有写文件都是 UTF-16LE”。[编码](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_character_encoding?view=powershell-7.5)
- cmdlet 关键路径用 `-ErrorAction Stop`；执行原生命令后立即保存 `$LASTEXITCODE`，按其退出码合同判断，再调用下一命令。
- 后台辅助进程不应意外弹窗；Windows 用 `Start-Process` 时按需加 `-WindowStyle Hidden`，跟踪 PID、日志和退出清理。
- 启动器或沙箱基础设施错误与脚本错误分开诊断；遵守平台提供的权限重试机制，不降低安全设置掩盖故障。

## 本次回退与校验

本任务使用 rg 枚举/搜索、jq/yq 结构化检查、harness 补丁修改。
fd/sd/bat 缺失时分别回退 rg/PowerShell、补丁和 Get-Content。
复杂覆盖统计与链接核验使用一次性确定性脚本；不以脚本替代已有搜索工具。
jq -e 的 false/null 返回 1，无输出返回 4；rg 无匹配为 1，错误码单独报告。
安装必要文档工具采用锁定的 npm 包和完整性校验，禁用安装脚本；
无关 CLI 不安装。cargo-binstall 的二进制策略需使用前查当时版本，
不能把 mise/aqua 等渠道概括为永不本地编译。
