# 选区联动与 Kilo 来源健康验收

<a id="linked-chart-selection-and-kilo-source-health-acceptance"></a>

2026-10-03–04；Windows 11 Pro x64、Node 24.21.0、Rust 1.98、Edge/WebView2
154.0.4258.53。版本 0.2.1；依赖以锁文件为准，环境见 [最新验收](current-acceptance.md)。

<a id="current-behavior"></a>

## 当前行为

- “近 2 个自然日”查询设置时区中的昨天和今天，不是滚动 24 小时。
- 趋势 token、调用与会话、价格参考曲线，总览历史两图及今日小时图均可横向拖选；
  松开后查询，支持反向选择和恢复，高亮在关联时间图之间同步。
- 趋势汇总、模型/Agent 分布、模型表及当前价参考共用选区；图表仍保留原范围。
  今日和历史选区独立。热力图和周分布放在最后并说明不受选区限制。
- Kilo 独立快照差异保留对账；逐行错误持续提示核对，可变行修正/删除后重评。
  不完整明细不参与完整对账，不通过 SQL 转型补零；未知版本仍是兼容读取。

刷选依据已核对安装包 BrushView/BrushController/BrushModel 源码及
[Apache ECharts action 文档](https://apache.googlesource.com/echarts-doc/+/24fe90e684ee48fa8ec5ab7e7a9f9c2fd9397e4e/en/api/action.md)：
lineX 的 coordRange 为坐标边界，brushEnd 对应刷选完成；使用此事件查询，
程序更新选框不触发重复选区请求。实际鼠标回归独立核验这些行为。

<a id="local-kilo-cause-and-recovery"></a>

## 本机 Kilo 根因与恢复

只读 SQL 仅输出角色、版本和数值聚合，没有输出消息正文、路径或会话身份。
唯一不一致会话属于 7.3.42：25 条 assistant 消息五互斥字段合计 706,116 token，
字段类型有效；session 五列快照合计为 0。原逻辑将 reconcile_mismatch 当作
逐行解析失败，错误地让整个文件 degraded。该差异不能用于核验 7.3.42。

新解析器 kilo-message-tokens-4 沿实际 discover → 注册 → 扫描 → 提交路径，
自动使旧处理位置失效并重读。副本核对通过后，在与应用共用的单写者文件锁保护下，
先 Online Backup，再修复已授权的本机统计库；不直接清空健康状态或诊断。

| 核对项 | 本机实际结果 |
| --- | --- |
| 原始来源读取 | 1 个库，13,947 行，13,376 条可解析调用；既有保留下界继续排除已过期明细 |
| 健康 | degraded → active_compat，需核对文件 1 → 0；未知历史版本兼容状态保留 |
| 元数据更新 | 34 条留存 Kilo 调用，解析器 3 → 4，无新增或真实内容冲突 |
| 数据保护 | 4,050 条留存事件及全部汇总指标不变，原始 Agent 库字节不变，诊断历史保留 |
| 重复读取不增加记录 | 第二次扫描新增/更新/冲突均 0，data_revision 保持 401 |

本轮一致备份、脚本和脱敏输出仅在根 build/trend-range-kilo/，该目录已忽略。
恢复备份需遵守应用写锁；原始来源不用恢复。

<a id="validation"></a>

## 验证

Edge 模拟 IPC 的真实鼠标回归已通过：正反向拖选、松开前不查询、三个趋势曲线、
总览历史两图及今日小时图、模型与 Agent 占比、模型表费用、整段恢复、过期响应、
自然日日期边界及原有十语言/主题/宽窄布局检查。图片放 build/browser-smoke/。

Rust 回归覆盖有效明细与独立快照不一致、旧健康/已消费位置自动重放、未知版本仍未核验、
坏 JSON/角色/类型/单条总量矛盾跨增量窗保留、修正/删除后恢复，以及解析器 3 → 4
不产生 token 冲突。完整检查退出 0：Rust 838、前端 20、脚本 3；类型无错误/告警。
Windows release/NSIS 退出 0，3,791,588 字节；无界面 11 项，原生 10 项及 20 次首屏
P95 741.5 ms。原生鼠标在两个真实合成小时之间拖选，再单点限制到 1 调用并恢复，
范围标签与实际 IPC/SQLite 数值一致；Tauri 调用入口保持原状。
命令与边界集中在 [最新验收](current-acceptance.md)。

浏览器模拟 IPC 不代替原生桌面验证结果；未安装、提交、推送或触发远端 CI。
