/**
 * 多语言消息目录（F3 约定：docs/design/desktop-usage/i18n.md）。
 * - 语言协商：用户设置 → 系统语言 → 默认；
 * - 缺失键回退默认语言并 console.warn（开发可诊断）；
 * - 键名点分命名空间，{name} 插值；复数按 key.one/key.other（zh 只有 other）；
 * - 数字/日期/百分比用 Intl（ECMA-402）；统计时间规则（时区/周起始）不随语言改变；
 * - 切换即时生效（runes 响应式，无需重启）。
 */
import { translatedCatalogs } from './locales';
import { telemetryCatalogs } from './telemetry-locales';
import { dashboardCatalogs } from './dashboard-locales';
import { polishCatalogs } from './polish-locales';
import { referenceCatalogs } from './reference-locales';
import { schedulingCatalogs } from './scheduling-locales';
import { completionCatalogs } from './completion-locales';

export const LANGUAGE_OPTIONS = [
  ['zh-CN', '简体中文'],
  ['zh-TW', '繁體中文'],
  ['en', 'English'],
  ['ja', '日本語'],
  ['ko', '한국어'],
  ['es', 'Español'],
  ['fr', 'Français'],
  ['de', 'Deutsch'],
  ['pt-BR', 'Português (Brasil)'],
  ['ru', 'Русский'],
] as const;
export type Locale = (typeof LANGUAGE_OPTIONS)[number][0];

const zhCN: Record<string, string> = {
  'quota.sectionTitle': 'Agent 额度',
  'quota.premiumRequests': 'Premium 请求',
  'quota.usedLabel': '已用',
  'quota.remaining': '剩余 {n}',
  'quota.snapshotAt': '快照 {time}',
  'quota.trend': '每日额度快照',
  'quota.note': '账户级请求配额（非 token；所有设备/入口共享）',
  'quota.empty': '刷新后显示额度（本机缓存）',
  'cards.cacheWrite': '缓存写入',
  'sources.localHint': '管理本机 Agent 的采集状态和用户归属。归档数据随刷新自动检查。',
  'sources.search': '搜索 Agent 或来源路径',
  'sources.noMatch': '没有匹配的数据源。',
  'sources.health.ok': '已就绪',
  'sources.health.degraded': '部分数据需核对',
  'sources.health.error': '读取失败',
  'sources.health.unknown': '等待采集',
  'workspace.local': '本机用量工作台',
  'workspace.private': '数据保存在本机',
  'workspace.updated': '最近采集 {time}',
  'workspace.filterReset': '清除筛选',
  'insights.title': '统计覆盖情况',
  'insights.activeDays': '活跃天数',
  'insights.knownUsage': '用量已知比例',
  'insights.knownHint': '按有记录的用量条目计算',
  'insights.durationSamples': '耗时样本',
  'insights.conflicts': '待核查记录',
  'insights.retained': '部分明细已清理，会话数和耗时无法完整还原。历史用量仍保留。',
  'insights.note': '未知数据以 — 显示；统计仅涵盖已采集的本机记录。',
  'app.title': 'LLM 用量',
  'app.subtitle': 'Agent 用量看板',
  'nav.overview': '总览',
  'nav.trend': '趋势',
  'nav.sources': '数据源',
  'nav.settings': '设置',
  'nav.details': '详情',
  'action.refresh': '采集并刷新',
  'action.refreshing': '采集中…',
  'common.loading': '加载中…',
  'common.empty': '尚无数据。启用数据源并刷新后，这里会显示用量统计。',
  'common.error': '查询失败：{message}',
  'common.unknown': '未知',
  'common.all': '全部',
  'cards.input': '输入 token',
  'cards.input.hint': '（含缓存）',
  'cards.output': '输出 token',
  'cards.total': '总 token',
  'cards.cacheRead': '缓存读取',
  'cards.cacheMiss': '普通输入',
  'cards.cacheRatio': '缓存读取占比',
  'cards.conflicts': '冲突记录',
  'cards.excluded': '未计入（归属未核验）',
  'cards.revision': '数据修订 {revision}',
  'cards.sessions': '会话数',
  'cards.avgDuration': '平均耗时',
  'cards.totalDuration': '总耗时',
  'cards.activeDays': '活动天数',
  'header.autoRefresh': '自动刷新',
  'header.autoRefresh.off': '关闭',
  'header.autoRefresh.sec': '{n} 秒',
  'header.autoRefresh.min': '{n} 分钟',
  'header.autoRefresh.hint': '界面数据自动重新查询的间隔，与后台采集间隔相互独立',
  'filter.range': '范围',
  'filter.granularity.label': '粒度',
  'filter.granularity.hour': '小时',
  'filter.granularity.day': '日',
  'filter.granularity.week': '周',
  'filter.granularity.month': '月',
  'filter.agent': 'Agent',
  'filter.model': '模型',
  'filter.quick.2days': '近 2 个自然日',
  'filter.quick.today': '当天',
  'filter.quick.7': '近 7 天',
  'filter.quick.30': '近 30 天',
  'filter.quick.365': '近一年',
  'overview.historyFilterHint': '仅影响历史趋势（今日数据不受影响）',
  'trend.calls': '调用次数',
  'trend.sessions': '会话数',
  'trend.tokens': 'token',
  'trend.inProgress': '（进行中）',
  'trend.partial': '（部分历史）',
  'trend.chart.calls': '调用与会话',
  'trend.chart.tokens': 'token 用量',
  'trend.metric.ratio': '缓存读取占比',
  'hourly.title': '今日逐小时',
  'heatmap.title': '活跃热力图（每天一格）',
  'heatmap.noCalls': '无调用',
  'heatmap.unavailable': '无按日数据',
  'heatmap.partial': '部分按日数据',
  'settings.tab.costs': '费用',
  'cost.title': '费用估算（参考）',
  'cost.disabled': '费用估算未启用；可在「设置 → 费用」中开启',
  'cost.atTime': '按发生时价',
  'cost.currentSim': '按当前价格模拟',
  'cost.sourceReported': '来源记录金额',
  'cost.total': '金额',
  'cost.coverage': '覆盖 {priced} / {known} token',
  'cost.partialCount': '{count} 条部分估算',
  'cost.unpricedCount': '{count} 条未计价',
  'cost.detailLimited': '仅明细保留期内可按当前价格模拟',
  'cost.freshness': '价格快照 {id} · {date}',
  'cost.noData': '所选范围暂无费用数据',
  'cost.unpricedReasons': '未计价原因',
  'cost.settings.enabled': '启用费用估算',
  'cost.settings.hint': '估算基于本地价格快照（订阅实付不等于该值）；供应商渠道未选定时不套价。',
  'cost.settings.provider': '供应商',
  'cost.settings.region': '地区',
  'cost.settings.channel': '渠道',
  'cost.settings.ttl': '缓存写档',
  'cost.settings.addProvider': '添加供应商默认',
  'cost.settings.remove': '删除',
  'cost.settings.snapshots': '价格快照',
  'cost.settings.import': '导入快照 JSON',
  'cost.settings.recompute': '重算费用',
  'cost.settings.recomputeStarted': '重算已开始，完成后刷新查看。',
  'cost.refresh.title': '在线价格刷新（models.dev）',
  'cost.refresh.enable': '启用在线刷新',
  'cost.refresh.hint': '仅从 models.dev 社区目录下载公开按量价目（HTTPS GET，不携带本机数据）；缓存在有效期内不重复下载，下载失败时回退到上一次成功下载的结果。不含各类订阅/Coding Plan 价格；无精确价目时回退参考该模型官方提供商的按量付费价。',
  'cost.refresh.ttl': '缓存有效期（天）',
  'cost.refresh.now': '立即刷新',
  'cost.refresh.running': '正在刷新…',
  'cost.refresh.cacheLine': '上次下载 {date}（{days} 天前）',
  'cost.refresh.never': '从未下载',
  'cost.refresh.result.fetched': '已下载并导入 {rows} 条价格（{id}）',
  'cost.refresh.result.cacheFresh': '缓存仍在有效期内（{date} 下载），未发起网络请求',
  'cost.refresh.result.usedCache': '下载失败，已回退到 {date} 的缓存导入（{message}）',
  'cost.refresh.result.noCache': '下载失败且无可用缓存：{message}',
  'cost.fallbackCount': '{count} 条官方回退定价',
  'heatmap.loadFailed': '热力图加载失败。',
  'heatmap.activeDays': '已知活跃天数',
  'heatmap.longestStreak': '已知最长连续活跃',
  'heatmap.peakDay': '已知最高调用日',
  'heatmap.coverage': '每格代表一个本地日期。斜纹表示无法按日还原；虚线表示只有部分按日数据。',
  'time.lessThanMinute': '不到 1 分钟',
  'table.model': '模型',
  'table.agent': 'Agent',
  'table.calls': '调用',
  'table.input': '输入',
  'table.output': '输出',
  'table.total': '总量',
  'table.cacheRead': '缓存读取',
  'table.unknownFields': '未知字段 {count}',
  'sources.title': '数据源',
  'sources.agent': 'Agent',
  'sources.instance': '实例',
  'sources.health': '健康',
  'sources.status': '状态',
  'sources.lastSuccess': '最近成功',
  'sources.schedule': '提取计划',
  'sources.schedule.inherit': '继承全局',
  'sources.schedule.daily': '每日定时',
  'sources.schedule.weekly': '每周定时',
  'sources.schedule.nextDue': '下次',
  'sources.schedule.time': '时间',
  'sources.compatFiles': '兼容读取 {count} 个文件',
  'sources.degradedFiles': '需核对 {count} 个文件',
  'sources.unsupportedFiles': '未识别 {count} 个文件',
  'sources.incompatibleFiles': '不兼容 {count} 个文件',
  'sources.missingFiles': '源文件已被清理 {count} 个',
  'sources.missingFiles.hint': '这些源文件已被对应 Agent 自行删除或压实，其历史用量只保存在本应用存档中；清空数据后无法从源重采（清空前会自动备份）。',
  'sources.enabled': '启用',
  'sources.disabled': '停用',
  'sources.none': '未发现本机数据源。安装并使用对应 Agent 后重启应用。',
  'refresh.status.running': '正在采集 {agent}…',
  'refresh.status.done': '上次刷新 {time}：新增 {added}、更新 {updated}',
  'refresh.progress': '采集进度 {percent}%',
  'refresh.eta': '约 {eta}',
  'settings.tab.general': '常规',
  'settings.archives.hint': '采集会自动读取已支持的会话归档。来源已删除且从未采集的历史无法恢复。',
  'settings.tab.retention': '归档保留',
  'settings.tab.system': '开机与后台',
  'settings.tab.identity': '来源身份',
  'settings.tab.export': '导出',
  'settings.save': '保存',
  'settings.timezone': '统计时区',
  'settings.weekStart': '周起始',
  'settings.weekStart.auto': '跟随语言地区',
  'settings.weekStart.autoEffective': '当前生效：{value}',
  'settings.weekStart.monday': '周一',
  'settings.weekStart.sunday': '周日',
  'settings.retention.events': '明细（原始事件）',
  'settings.retention.hourly': '小时汇总',
  'settings.retention.daily': '按天汇总',
  'settings.retention.weekly': '按周汇总',
  'settings.retention.monthly': '按月汇总',
  'settings.retention.yearly': '按年汇总',
  'settings.retention.defaultDays': '默认 {days} 天',
  'settings.retention.yearlyForever': '留空 = 终身保留',
  'settings.retention.days': '天',
  'settings.retention.warning': '缩短保留立即生效，且已删除的数据不可恢复。',
  'settings.invalidNumber': '保留天数需为不小于 1 的整数',
  'settings.invalidInterval': '间隔需为 0–86400 的整数秒',
  'settings.invalidTimezone': '请输入有效的 IANA 时区名',
  'settings.timezone.search': '搜索时区',
  'settings.timezone.noMatch': '无匹配时区',
  'settings.interval': '今日刷新提取间隔（秒，0 关闭）',
  'settings.interval.hint': '默认 3600（每小时）',
  'settings.restoreDefaults': '恢复默认设置',
  'settings.defaultsPending': '已恢复默认值，保存后生效',
  'settings.language': '界面语言',
  'settings.theme': '主题',
  'settings.theme.system': '跟随系统',
  'settings.theme.light': '亮色',
  'settings.theme.dark': '暗色',
  'settings.manualRoots': '手工数据源根目录（每行一个）',
  'settings.saved': '已保存',
  'settings.saveFailed': '保存失败：{message}',
  'settings.host': '本机来源身份',
  'settings.hostnameAlias': '显示别名',
  'settings.hostnameAlias.hint': '仅用于辨认本机来源，不改变身份标识',
  'settings.dbPath': '数据库位置',
  'settings.schemaVersion': '数据库结构版本',
  'settings.note': '时区更改会重算仍保留的明细；仅剩汇总的历史无法改换时区。',
  'system.autoStart': '开机自动运行',
  'system.autoStart.hint': '登录 Windows 后自动启动本应用（当前用户，无需管理员）',
  'system.state.on': '已开启',
  'system.state.off': '已关闭',
  'system.refreshTask': '后台刷新任务',
  'system.refreshTask.hint': '系统计划任务：应用未运行时也会自动补采集',
  'system.refreshTask.install': '安装',
  'system.refreshTask.uninstall': '卸载',
  'system.refreshTask.installed': '已安装',
  'system.refreshTask.notInstalled': '未安装',
  'system.refreshTask.interval': '每小时（无界面运行）',
  'system.unsupported': '当前平台暂不支持',
  'system.loading': '读取系统状态…',
  'system.applied': '已生效',
  'system.actionFailed': '操作失败：{message}',
  'export.title': '导出',
  'export.userFilter': '用户',
  'export.hostFilter': '主机',
  'export.currentTag': '（当前）',
  'export.scopeTitle': '导出范围',
  'export.selectAll': '全选',
  'export.scopeHint': '导出交换包按勾选的用户与主机组合分别生成文件；全部勾选时合并为一份。展示用 CSV 不按此范围过滤。',
  'export.noneSelected': '请至少勾选一个用户和一个主机。',
  'export.doneMulti': '已导出 {count} 个文件：{paths}',
  'export.csv': '导出 CSV（展示用）',
  'export.exchange': '导出聚合数据（不含 session 明细）',
  'export.cancelled': '已取消导出',
  'export.done': '已导出：{path}',
  'export.failed': '导出失败：{message}',
  'users.label': '用户',
  'users.create': '新建用户…',
  'users.namePlaceholder': '用户名',
  'users.createConfirm': '创建',
  'users.createCancel': '取消',
  'users.actionFailed': '用户操作失败：{message}',
  'sources.user': '归属用户',
  'sources.userPlaceholder': '设置归属…',
  'sources.assignFailed': '归属设置失败：{message}',
  'cleanup.stats': '存储统计',
  'cleanup.stats.events': '明细事件',
  'cleanup.stats.hourly': '小时汇总行',
  'cleanup.stats.daily': '按天汇总行',
  'cleanup.stats.period': '周期汇总行',
  'cleanup.stats.diagnostics': '诊断记录',
  'cleanup.stats.size': '库大小（含 WAL）',
  'cleanup.title': '手动清理',
  'cleanup.daysBefore': '清理早于',
  'cleanup.daysUnit': '天',
  'cleanup.run': '执行清理',
  'cleanup.running': '清理中…',
  'cleanup.result': '已删除：明细事件 {events}、小时行 {hourly}、日行 {daily}、周期行 {period}；物化周期行 {materialized}',
  'cleanup.failed': '清理失败：{message}',
  'cleanup.invalidDays': '天数需为不小于 1 的整数',
  'cleanup.statsFailed': '读取存储统计失败：{message}',
  'cleanup.clearAll': '清理全部数据并重新采集',
  'cleanup.clearAllConfirm': '将删除所有统计数据与游标，下次刷新将全量重新采集。此操作不可撤销。',
  'cleanup.clearAllMissing': '警告：有 {n} 个源文件已被对应 Agent 清理（源端不存在），清空后这部分历史无法重新采集。',
  'cleanup.clearAllBackup': '清空前将自动备份当前数据库（backups/ 目录，保留最近 3 份）。',
  'cleanup.clearAllBackupAt': '已备份至：{path}',
  'cleanup.clearAllDone': '已清理：{detail}',
  'cleanup.clearAllTriggered': '已触发全量重新采集，进度见顶栏。',
  'cleanup.clearAllPhase.waiting': '等待当前采集结束…',
  'cleanup.clearAllPhase.backup': '正在备份数据库…',
  'cleanup.clearAllPhase.clearing': '正在清理数据…',
  'cleanup.clearAllPhase.cleared': '清理完成，准备重新采集…',
  'cleanup.clearAllPhase.rescan': '正在全量重新采集…（进度见顶栏）',
  'import.button': '导入全部数据…',
  'import.title': '导入',
  'import.scopeHint': '导入会写入交换包内的全部数据，不受上方导出范围影响。',
  'import.cancelled': '已取消导入',
  'import.done': '导入完成：注册来源 {sources}；日汇总插入 {dailyInserted}、替换 {dailyReplaced}、跳过 {dailySkipped}、冲突 {dailyConflicts}；小时汇总插入 {hourlyInserted}、替换 {hourlyReplaced}、跳过 {hourlySkipped}',
  'import.failed': '导入失败：{message}',
  'overview.todaySection': '今日数据',
  'overview.historySection': '历史趋势',
  'overview.periodSummary': '选中时间点汇总',
  'overview.periodSummary.clear': '清除',
  'overview.periodSummary.hint': '点击选取时段，或按住鼠标横向拖选连续范围；再次点击同一时段取消',
  'overview.today': '今日汇总',
  'overview.today.calls': '今日调用',
  'overview.today.input': '输入 token',
  'overview.today.output': '输出 token',
  'overview.today.total': '总 token',
  'overview.today.cacheRatio': '缓存读取占比',
  'overview.today.sessions': '会话数',
  'overview.today.avgDuration': '平均耗时',
  'overview.todayPie.model': '今日模型用量',
  'overview.todayPie.agent': '今日 Agent 用量',
  'overview.todayTable.model': '今日模型明细',
  'overview.todayTable.agent': '今日 Agent 明细',
  'trend.metric.input': '输入 token',
  'trend.metric.output': '输出 token',
  'trend.metric.total': '总 token',
  'trend.weekday': '周分布',
  'trend.weekdayCoverage': '仅包含仍有每日数据的日期。',
  'trend.pie.model': '模型用量分布',
  'trend.pie.agent': 'Agent 用量分布',
  'trend.summary': '当前范围汇总',
  'chart.dimension.label': '分组',
  'chart.dimension.total': '总用量',
  'chart.dimension.model': '按模型',
  'chart.dimension.agent': '按Agent',
  'chart.dimension.agentModel': '按Agent+模型',
  'chart.loadFailed': '图表分组数据加载失败：{message}',
  'overview.breakdown.input': '缓存读取 {hit} · 普通输入 {miss}',
  'panel.dragHint': '拖拽标题栏调整位置',
  'panel.resizeHint': '拖动调整尺寸；方向键调整宽高，Home 恢复自动高度',
  'panel.hide': '隐藏面板',
  'panel.show': '显示面板',
  'panel.durationSummary': '平均耗时 {avg} · 总耗时 {total}',
  'panel.edit': '编辑布局',
  'panel.editDone': '完成编辑',
  'panel.reset': '重置布局',
  'panel.editHint': '编辑模式：拖动面板标题栏调整顺序，👁/🚫 切换显示，拖动右下角把手调整尺寸。',
  'settings.tab.logs': '日志',
  'logs.title': '诊断日志',
  'logs.time': '时间',
  'logs.code': '代码',
  'logs.field': '字段',
  'logs.instance': '实例',
  'logs.message': '消息',
  'logs.refresh': '刷新',
  'logs.auto': '自动刷新',
  'logs.empty': '暂无诊断记录。',
  'logs.failed': '日志读取失败：{message}',
  'logs.hideRetention': '隐藏保留清理',
  'logs.showRetention': '显示保留清理',
  'details.time': '时间',
  'details.category': '类别',
  'details.duration': '耗时',
  'details.session': '会话',
  'details.prev': '上一页',
  'details.next': '下一页',
  'details.jump': '跳转到页',
  'details.jumpGo': '跳转',
  'details.pageOf': '第 {page} / {pages} 页',
  'details.rowsPerPage': '每页',
  'details.totalRows': '共 {count} 条',
  'details.empty': '当前范围内没有明细事件。',
};

const en: Record<string, string> = {
  'quota.sectionTitle': 'Agent quotas',
  'quota.premiumRequests': 'Premium requests',
  'quota.usedLabel': 'used',
  'quota.remaining': '{n} left',
  'quota.snapshotAt': 'Snapshot {time}',
  'quota.trend': 'Daily quota snapshots',
  'quota.note': 'Account-level request quota (not tokens; shared across devices/surfaces)',
  'quota.empty': 'Quota appears after a refresh (local cache)',
  'cards.cacheWrite': 'Cache write',
  'sources.localHint': 'Manage local Agent collection and user assignments. Archives are checked during collection.',
  'sources.search': 'Search Agent or source path',
  'sources.noMatch': 'No matching sources.',
  'sources.health.ok': 'Ready',
  'sources.health.degraded': 'Some data needs review',
  'sources.health.error': 'Read failed',
  'sources.health.unknown': 'Awaiting collection',
  'workspace.local': 'LOCAL USAGE WORKSPACE',
  'workspace.private': 'Data stays on this device',
  'workspace.updated': 'Last collected {time}',
  'workspace.filterReset': 'Clear filters',
  'insights.title': 'Data coverage',
  'insights.activeDays': 'Active days',
  'insights.knownUsage': 'Known usage',
  'insights.knownHint': 'Share of recorded usage entries',
  'insights.durationSamples': 'Duration samples',
  'insights.conflicts': 'Records to check',
  'insights.retained': 'Some details have expired. Sessions and durations are unavailable; historical usage is retained.',
  'insights.note': 'Unknown values appear as —. Statistics cover collected local records.',
  'app.title': 'LLM Usage',
  'app.subtitle': 'Agent usage dashboard',
  'nav.overview': 'Overview',
  'nav.trend': 'Trends',
  'nav.sources': 'Sources',
  'nav.settings': 'Settings',
  'nav.details': 'Details',
  'action.refresh': 'Collect & refresh',
  'action.refreshing': 'Scanning…',
  'common.loading': 'Loading…',
  'common.empty': 'No data yet. Enable sources and refresh; usage will appear here.',
  'common.error': 'Query failed: {message}',
  'common.unknown': 'unknown',
  'common.all': 'All',
  'cards.input': 'Input tokens',
  'cards.input.hint': ' (incl. cache hits)',
  'cards.output': 'Output tokens',
  'cards.total': 'Total tokens',
  'cards.cacheRead': 'Cache hits',
  'cards.cacheMiss': 'Uncached input',
  'cards.cacheRatio': 'Cached input share',
  'cards.conflicts': 'Conflicting records',
  'cards.excluded': 'Excluded (unverified)',
  'cards.revision': 'Data revision {revision}',
  'cards.sessions': 'Sessions',
  'cards.avgDuration': 'Avg duration',
  'cards.totalDuration': 'Total duration',
  'cards.activeDays': 'Active days',
  'header.autoRefresh': 'Auto-refresh',
  'header.autoRefresh.off': 'Off',
  'header.autoRefresh.sec': '{n}s',
  'header.autoRefresh.min': '{n} min',
  'header.autoRefresh.hint': 'How often the UI re-queries its data (independent of the background collection interval)',
  'filter.range': 'Range',
  'filter.granularity.label': 'Granularity',
  'filter.granularity.hour': 'Hour',
  'filter.granularity.day': 'Day',
  'filter.granularity.week': 'Week',
  'filter.granularity.month': 'Month',
  'filter.agent': 'Agent',
  'filter.model': 'Model',
  'filter.quick.2days': 'Last 2 calendar days',
  'filter.quick.today': 'Today',
  'filter.quick.7': 'Last 7 days',
  'filter.quick.30': 'Last 30 days',
  'filter.quick.365': 'Last year',
  'overview.historyFilterHint': 'Applies to historical trends only (today is unaffected)',
  'trend.calls': 'Calls',
  'trend.sessions': 'Sessions',
  'trend.tokens': 'Tokens',
  'trend.inProgress': ' (in progress)',
  'trend.partial': ' (partial history)',
  'trend.chart.calls': 'Calls & sessions',
  'trend.chart.tokens': 'Token usage',
  'trend.metric.ratio': 'Cached input share',
  'hourly.title': 'Today by hour',
  'heatmap.title': 'Activity heatmap (one day per square)',
  'heatmap.noCalls': 'No calls',
  'heatmap.unavailable': 'Daily data unavailable',
  'heatmap.partial': 'Partial daily data',
  'settings.tab.costs': 'Costs',
  'cost.title': 'Cost estimates (reference)',
  'cost.disabled': 'Cost estimation is off; enable it in Settings → Costs',
  'cost.atTime': 'At occurrence-time prices',
  'cost.currentSim': 'Simulated at current prices',
  'cost.sourceReported': 'Source-reported amounts',
  'cost.total': 'Amount',
  'cost.coverage': 'Coverage {priced} / {known} tokens',
  'cost.partialCount': '{count} partial estimates',
  'cost.unpricedCount': '{count} unpriced events',
  'cost.detailLimited': 'Current-price simulation covers only retained event details',
  'cost.freshness': 'Price snapshot {id} · {date}',
  'cost.noData': 'No cost data for the selected range',
  'cost.unpricedReasons': 'Unpriced reasons',
  'cost.settings.enabled': 'Enable cost estimation',
  'cost.settings.hint': 'Estimates use local price snapshots (subscription bills differ); providers without a selected channel are not priced.',
  'cost.settings.provider': 'Provider',
  'cost.settings.region': 'Region',
  'cost.settings.channel': 'Channel',
  'cost.settings.ttl': 'Cache-write tier',
  'cost.settings.addProvider': 'Add provider default',
  'cost.settings.remove': 'Remove',
  'cost.settings.snapshots': 'Price snapshots',
  'cost.settings.import': 'Import snapshot JSON',
  'cost.settings.recompute': 'Recompute costs',
  'cost.settings.recomputeStarted': 'Recompute started; refresh later to see the results.',
  'cost.refresh.title': 'Online price refresh (models.dev)',
  'cost.refresh.enable': 'Enable online refresh',
  'cost.refresh.hint': 'Downloads only public pay-as-you-go price lists from the models.dev community catalog (HTTPS GET; no local data is sent). The cache is reused within its TTL, and failed downloads fall back to the last successful copy. Subscription/Coding Plan prices are excluded; when no exact price exists, the model\u2019s official provider pay-as-you-go price is used as a fallback.',
  'cost.refresh.ttl': 'Cache TTL (days)',
  'cost.refresh.now': 'Refresh now',
  'cost.refresh.running': 'Refreshing…',
  'cost.refresh.cacheLine': 'Last download {date} ({days} d ago)',
  'cost.refresh.never': 'Never downloaded',
  'cost.refresh.result.fetched': 'Downloaded and imported {rows} price rows ({id})',
  'cost.refresh.result.cacheFresh': 'Cache is still fresh (downloaded {date}); no network request was made',
  'cost.refresh.result.usedCache': 'Download failed; imported the cached copy from {date} instead ({message})',
  'cost.refresh.result.noCache': 'Download failed and no cache is available: {message}',
  'cost.fallbackCount': '{count} official-fallback priced',
  'heatmap.loadFailed': 'Could not load the heatmap.',
  'heatmap.activeDays': 'Known active days',
  'heatmap.longestStreak': 'Longest known streak',
  'heatmap.peakDay': 'Busiest known day',
  'heatmap.coverage': 'Each square is one local date. Hatching means no daily data; a dashed outline means only partial daily data.',
  'time.lessThanMinute': 'Less than 1 minute ago',
  'table.model': 'Model',
  'table.agent': 'Agent',
  'table.calls': 'Calls',
  'table.input': 'Input',
  'table.output': 'Output',
  'table.total': 'Total',
  'table.cacheRead': 'Cache hits',
  'table.unknownFields': '{count} unknown fields',
  'sources.title': 'Data sources',
  'sources.agent': 'Agent',
  'sources.instance': 'Instance',
  'sources.health': 'Health',
  'sources.status': 'Status',
  'sources.lastSuccess': 'Last success',
  'sources.schedule': 'Schedule',
  'sources.schedule.inherit': 'Inherit global',
  'sources.schedule.daily': 'Daily at time',
  'sources.schedule.weekly': 'Weekly',
  'sources.schedule.nextDue': 'Next',
  'sources.schedule.time': 'Time',
  'sources.compatFiles': '{count} files read with a compatible parser',
  'sources.degradedFiles': '{count} files need review',
  'sources.unsupportedFiles': '{count} unrecognized files',
  'sources.incompatibleFiles': '{count} incompatible files',
  'sources.missingFiles': '{count} source files removed',
  'sources.missingFiles.hint': 'These source files were deleted or compacted by their agent. Their usage history only exists in this app\u2019s archive and cannot be re-collected after clearing (an automatic backup is made first).',
  'sources.enabled': 'Enabled',
  'sources.disabled': 'Disabled',
  'sources.none': 'No local sources discovered. Install and use an agent, then restart.',
  'refresh.status.running': 'Scanning {agent}…',
  'refresh.status.done': 'Last refresh {time}: added {added}, updated {updated}',
  'refresh.progress': 'Progress {percent}%',
  'refresh.eta': '≈ {eta}',
  'settings.tab.general': 'General',
  'settings.archives.hint': 'Collection includes supported session archives automatically. History deleted before collection cannot be recovered.',
  'settings.tab.retention': 'Retention',
  'settings.tab.system': 'Startup & background',
  'settings.tab.identity': 'Origin identity',
  'settings.tab.export': 'Export',
  'settings.save': 'Save',
  'settings.timezone': 'Statistics timezone',
  'settings.weekStart': 'Week starts on',
  'settings.weekStart.auto': 'Follow language & region',
  'settings.weekStart.autoEffective': 'Currently: {value}',
  'settings.weekStart.monday': 'Monday',
  'settings.weekStart.sunday': 'Sunday',
  'settings.retention.events': 'Raw events',
  'settings.retention.hourly': 'Hourly rollup',
  'settings.retention.daily': 'Daily rollup',
  'settings.retention.weekly': 'Weekly rollup',
  'settings.retention.monthly': 'Monthly rollup',
  'settings.retention.yearly': 'Yearly rollup',
  'settings.retention.defaultDays': 'default {days} days',
  'settings.retention.yearlyForever': 'empty = keep forever',
  'settings.retention.days': 'days',
  'settings.retention.warning': 'Shortening retention applies immediately; deleted data cannot be recovered.',
  'settings.invalidNumber': 'Retention days must be an integer of at least 1',
  'settings.invalidInterval': 'Interval must be an integer between 0 and 86400 seconds',
  'settings.invalidTimezone': 'Enter a valid IANA timezone name',
  'settings.timezone.search': 'Search timezone',
  'settings.timezone.noMatch': 'No matching timezones',
  'settings.interval': 'Today refresh extraction interval (seconds, 0 = off)',
  'settings.interval.hint': 'Default 3600 (hourly)',
  'settings.restoreDefaults': 'Restore defaults',
  'settings.defaultsPending': 'Defaults restored; save to apply',
  'settings.language': 'Language',
  'settings.theme': 'Theme',
  'settings.theme.system': 'Follow system',
  'settings.theme.light': 'Light',
  'settings.theme.dark': 'Dark',
  'settings.manualRoots': 'Manual source roots (one per line)',
  'settings.saved': 'Saved',
  'settings.saveFailed': 'Save failed: {message}',
  'settings.host': 'Local origin host',
  'settings.hostnameAlias': 'Display alias',
  'settings.hostnameAlias.hint': 'Display only; does not change the identity key',
  'settings.dbPath': 'Database location',
  'settings.schemaVersion': 'Schema version',
  'settings.note': 'Changing timezone rebuilds retained details. Aggregate-only history cannot be rebucketed.',
  'system.autoStart': 'Launch at startup',
  'system.autoStart.hint': 'Start this app automatically at sign-in (current user, no admin required)',
  'system.state.on': 'On',
  'system.state.off': 'Off',
  'system.refreshTask': 'Background refresh task',
  'system.refreshTask.hint': 'A scheduled task that catches up collection even while the app is closed',
  'system.refreshTask.install': 'Install',
  'system.refreshTask.uninstall': 'Uninstall',
  'system.refreshTask.installed': 'Installed',
  'system.refreshTask.notInstalled': 'Not installed',
  'system.refreshTask.interval': 'Hourly (headless)',
  'system.unsupported': 'Not supported on this platform yet',
  'system.loading': 'Reading system status…',
  'system.applied': 'Applied',
  'system.actionFailed': 'Operation failed: {message}',
  'export.title': 'Export',
  'export.userFilter': 'User',
  'export.hostFilter': 'Host',
  'export.currentTag': ' (current)',
  'export.scopeTitle': 'Export scope',
  'export.selectAll': 'Select all',
  'export.scopeHint': 'Exchange exports write one file per selected user/host pair; selecting all combines into a single file. Display CSV is not filtered by this scope.',
  'export.noneSelected': 'Select at least one user and one host.',
  'export.doneMulti': 'Exported {count} files: {paths}',
  'export.csv': 'Export CSV (display)',
  'export.exchange': 'Export aggregate data (no session details)',
  'export.cancelled': 'Export cancelled',
  'export.done': 'Exported: {path}',
  'export.failed': 'Export failed: {message}',
  'users.label': 'User',
  'users.create': 'New user…',
  'users.namePlaceholder': 'User name',
  'users.createConfirm': 'Create',
  'users.createCancel': 'Cancel',
  'users.actionFailed': 'User action failed: {message}',
  'sources.user': 'Owner',
  'sources.userPlaceholder': 'Assign…',
  'sources.assignFailed': 'Failed to assign owner: {message}',
  'cleanup.stats': 'Storage stats',
  'cleanup.stats.events': 'Events',
  'cleanup.stats.hourly': 'Hourly rows',
  'cleanup.stats.daily': 'Daily rows',
  'cleanup.stats.period': 'Period rows',
  'cleanup.stats.diagnostics': 'Diagnostics',
  'cleanup.stats.size': 'Database size (incl. WAL)',
  'cleanup.title': 'Manual cleanup',
  'cleanup.daysBefore': 'Delete data older than',
  'cleanup.daysUnit': 'days',
  'cleanup.run': 'Run cleanup',
  'cleanup.running': 'Cleaning…',
  'cleanup.result': 'Deleted: {events} events, {hourly} hourly rows, {daily} daily rows, {period} period rows; materialized {materialized} period rows',
  'cleanup.failed': 'Cleanup failed: {message}',
  'cleanup.invalidDays': 'Days must be an integer of at least 1',
  'cleanup.statsFailed': 'Failed to read storage stats: {message}',
  'cleanup.clearAll': 'Clear all data & re-collect',
  'cleanup.clearAllConfirm': 'This deletes all statistics and cursors; the next refresh will re-collect everything from scratch. This cannot be undone.',
  'cleanup.clearAllMissing': 'Warning: {n} source files have already been removed by their agents. That history cannot be re-collected after clearing.',
  'cleanup.clearAllBackup': 'A database backup is created automatically before clearing (backups/ directory, latest 3 kept).',
  'cleanup.clearAllBackupAt': 'Backup saved to: {path}',
  'cleanup.clearAllDone': 'Cleared: {detail}',
  'cleanup.clearAllTriggered': 'Full re-collection started; see progress in the top bar.',
  'cleanup.clearAllPhase.waiting': 'Waiting for the current collection to finish…',
  'cleanup.clearAllPhase.backup': 'Backing up the database…',
  'cleanup.clearAllPhase.clearing': 'Clearing data…',
  'cleanup.clearAllPhase.cleared': 'Clearing finished; preparing full re-collection…',
  'cleanup.clearAllPhase.rescan': 'Re-collecting everything… (progress in the top bar)',
  'import.button': 'Import all data…',
  'import.title': 'Import',
  'import.scopeHint': 'Importing loads all data in the package; it is independent of the export scope above.',
  'import.cancelled': 'Import cancelled',
  'import.done': 'Import done: {sources} sources registered; daily inserted {dailyInserted}, replaced {dailyReplaced}, skipped {dailySkipped}, conflicts {dailyConflicts}; hourly inserted {hourlyInserted}, replaced {hourlyReplaced}, skipped {hourlySkipped}',
  'import.failed': 'Import failed: {message}',
  'overview.todaySection': 'Today',
  'overview.historySection': 'Historical trends',
  'overview.periodSummary': 'Selected period',
  'overview.periodSummary.clear': 'Clear',
  'overview.periodSummary.hint': 'Click a period or drag horizontally to select a range; click the same period again to clear',
  'overview.today': 'Today summary',
  'overview.today.calls': 'Calls today',
  'overview.today.input': 'Input tokens',
  'overview.today.output': 'Output tokens',
  'overview.today.total': 'Total tokens',
  'overview.today.cacheRatio': 'Cached input share',
  'overview.today.sessions': 'Sessions',
  'overview.today.avgDuration': 'Avg duration',
  'overview.todayPie.model': 'Today by model',
  'overview.todayPie.agent': 'Today by agent',
  'overview.todayTable.model': 'Today model breakdown',
  'overview.todayTable.agent': 'Today agent breakdown',
  'trend.metric.input': 'Input tokens',
  'trend.metric.output': 'Output tokens',
  'trend.metric.total': 'Total tokens',
  'trend.weekday': 'By weekday',
  'trend.weekdayCoverage': 'Includes dates with retained daily data only.',
  'trend.pie.model': 'Model usage share',
  'trend.pie.agent': 'Agent usage share',
  'trend.summary': 'Current range summary',
  'chart.dimension.label': 'Group by',
  'chart.dimension.total': 'Total',
  'chart.dimension.model': 'By model',
  'chart.dimension.agent': 'By agent',
  'chart.dimension.agentModel': 'By agent + model',
  'chart.loadFailed': 'Failed to load grouped chart data: {message}',
  'overview.breakdown.input': 'Cache read {hit} · uncached {miss}',
  'panel.dragHint': 'Drag the title bar to reorder',
  'panel.resizeHint': 'Drag to resize; arrow keys adjust width/height, Home restores automatic height',
  'panel.hide': 'Hide panel',
  'panel.show': 'Show panel',
  'panel.durationSummary': 'Avg duration {avg} · total {total}',
  'panel.edit': 'Edit layout',
  'panel.editDone': 'Done editing',
  'panel.reset': 'Reset layout',
  'panel.editHint': 'Edit mode: drag panel headers to reorder; use 👁/🚫 to toggle visibility; drag the bottom-right grip to resize.',
  'settings.tab.logs': 'Logs',
  'logs.title': 'Diagnostics',
  'logs.time': 'Time',
  'logs.code': 'Code',
  'logs.field': 'Field',
  'logs.instance': 'Instance',
  'logs.message': 'Message',
  'logs.refresh': 'Refresh',
  'logs.auto': 'Auto refresh',
  'logs.empty': 'No diagnostics yet.',
  'logs.failed': 'Failed to load logs: {message}',
  'logs.hideRetention': 'Hide retention cleanup',
  'logs.showRetention': 'Show retention cleanup',
  'details.time': 'Time',
  'details.category': 'Category',
  'details.duration': 'Duration',
  'details.session': 'Session',
  'details.prev': 'Previous',
  'details.next': 'Next',
  'details.jump': 'Go to page',
  'details.jumpGo': 'Go',
  'details.pageOf': 'Page {page} of {pages}',
  'details.rowsPerPage': 'Rows per page',
  'details.totalRows': '{count} rows',
  'details.empty': 'No detail events in the current range.',
};

const catalogs: Record<Locale, Record<string, string>> = {
  'zh-CN': zhCN,
  en,
  'zh-TW': translatedCatalogs['zh-TW'],
  ja: translatedCatalogs.ja,
  ko: translatedCatalogs.ko,
  es: translatedCatalogs.es,
  fr: translatedCatalogs.fr,
  de: translatedCatalogs.de,
  'pt-BR': translatedCatalogs['pt-BR'],
  ru: translatedCatalogs.ru,
};

for (const [locale, catalog] of Object.entries(catalogs)) {
  Object.assign(catalog, telemetryCatalogs[locale]);
  Object.assign(catalog, dashboardCatalogs[locale]);
  Object.assign(catalog, polishCatalogs[locale]);
  Object.assign(catalog, referenceCatalogs[locale]);
  Object.assign(catalog, schedulingCatalogs[locale]);
  Object.assign(catalog, completionCatalogs[locale]);
}

export const DEFAULT_LOCALE: Locale = 'zh-CN';

export function normalizeLocale(value: string | null): Locale | null {
  if (!value) return null;
  const tag = value.toLowerCase().replace('_', '-');
  if (tag.startsWith('zh-hant') || tag.startsWith('zh-tw') || tag.startsWith('zh-hk') || tag.startsWith('zh-mo')) return 'zh-TW';
  if (tag.startsWith('zh')) return 'zh-CN';
  if (tag.startsWith('en')) return 'en';
  if (tag.startsWith('ja')) return 'ja';
  if (tag.startsWith('ko')) return 'ko';
  if (tag.startsWith('es')) return 'es';
  if (tag.startsWith('fr')) return 'fr';
  if (tag.startsWith('de')) return 'de';
  if (tag.startsWith('pt')) return 'pt-BR';
  if (tag.startsWith('ru')) return 'ru';
  return null;
}

function detectSystemLocale(): Locale {
  for (const tag of navigator.languages ?? [navigator.language]) {
    const locale = normalizeLocale(tag);
    if (locale) return locale;
  }
  return DEFAULT_LOCALE;
}

export const i18n = $state({
  locale: DEFAULT_LOCALE as Locale,
  /** 用户显式选择过的语言（设置持久化）；null 表示尚未选择，用系统语言。 */
  userChoice: null as Locale | null,
});

export function initLocale(userChoice: string | null): void {
  i18n.userChoice = normalizeLocale(userChoice);
  i18n.locale = i18n.userChoice ?? detectSystemLocale();
}

export function setLocale(locale: Locale): void {
  i18n.userChoice = locale;
  i18n.locale = locale;
}

/** 翻译：缺失键回退默认语言并告警；{name} 插值。 */
export function t(key: string, params?: Record<string, string | number>): string {
  let text = catalogs[i18n.locale][key] ?? catalogs[DEFAULT_LOCALE][key];
  if (text === undefined) {
    console.warn(`i18n: missing key "${key}" in all catalogs`);
    return key;
  }
  if (params) {
    for (const [name, value] of Object.entries(params)) {
      text = text.split(`{${name}}`).join(String(value));
    }
  }
  return text;
}

/** 数字格式化（语言地区分组；统计值本身不变）。 */
export function fmtNumber(value: number | string | null | undefined): string {
  if (value === null || value === undefined || value === '') return '—';
  const n = typeof value === 'string' ? Number(value) : value;
  if (!Number.isFinite(n)) return String(value);
  return new Intl.NumberFormat(i18n.locale).format(n);
}

/** 精确数字（千分位分组）：tooltip 显示完整值，如 332,958,868。 */
export function fmtPrecise(value: number | string | null | undefined): string {
  return fmtNumber(value);
}

/** 一位小数（≥100 取整）用于紧凑单位换算。 */
function oneDecimal(x: number): string {
  const r = x >= 100 ? Math.round(x) : Math.round(x * 10) / 10;
  return String(r);
}

/** 数量级自动缩放（卡片与图表 y 轴）：zh 按中文习惯 万/亿/万亿，
 *  en 用 Intl 紧凑记法（K/M/B/T）；<1 万原样显示。 */
export function fmtSmart(value: number | string | null | undefined): string {
  if (value === null || value === undefined || value === '') return '—';
  const n = typeof value === 'string' ? Number(value) : value;
  if (!Number.isFinite(n)) return String(value);
  const abs = Math.abs(n);
  if (i18n.locale === 'zh-CN' || i18n.locale === 'zh-TW') {
    const units = i18n.locale === 'zh-TW' ? ['萬', '億', '兆'] : ['万', '亿', '万亿'];
    if (abs < 10_000) return String(n);
    if (abs < 1e8) return String(oneDecimal(n / 1e4)) + units[0];
    if (abs < 1e12) return String(oneDecimal(n / 1e8)) + units[1];
    return String(oneDecimal(n / 1e12)) + units[2];
  }
  return new Intl.NumberFormat(i18n.locale, {
    notation: 'compact',
    maximumFractionDigits: 1,
  }).format(n);
}

/** 耗时（毫秒）：秒保留 1 位；分钟/小时/天按量级换算（单位缩写双语言通用）。 */
export function fmtDurationShort(ms: number | null | undefined): string {
  if (ms === null || ms === undefined || !Number.isFinite(ms)) return '—';
  const seconds = ms / 1000;
  if (seconds < 1) return `${Math.round(ms)} ms`;
  if (seconds < 60) return `${seconds.toFixed(1)} s`;
  const minutes = seconds / 60;
  if (minutes < 60) return `${minutes.toFixed(1)} min`;
  const hours = minutes / 60;
  if (hours < 24) return `${hours.toFixed(1)} h`;
  return `${(hours / 24).toFixed(1)} d`;
}

/** 百分比（0–1）。 */
export function fmtPercent(ratio: number | null | undefined): string {
  if (ratio === null || ratio === undefined || !Number.isFinite(ratio)) return '—';
  return new Intl.NumberFormat(i18n.locale, { style: 'percent', maximumFractionDigits: 1 }).format(ratio);
}

/** 相对时间（最近成功等）。 */
export function fmtRelative(ms: number | null | undefined): string {
  if (!ms) return '—';
  const delta = Date.now() - ms;
  const minutes = Math.round(delta / 60_000);
  if (minutes < 1) return t('time.lessThanMinute');
  const formatter = new Intl.RelativeTimeFormat(i18n.locale, { numeric: 'always', style: 'short' });
  if (minutes < 60) return formatter.format(-minutes, 'minute');
  const hours = Math.round(minutes / 60);
  if (hours < 24) return formatter.format(-hours, 'hour');
  return formatter.format(-Math.round(hours / 24), 'day');
}

/** 预计剩余时长（<60 秒按秒、<3600 按分钟、否则按小时；单位词随语言）。 */
export function fmtEtaDuration(seconds: number | null | undefined): string {
  if (seconds === null || seconds === undefined || !Number.isFinite(seconds) || seconds < 0) {
    return '';
  }
  const unit: 'second' | 'minute' | 'hour' =
    seconds < 60 ? 'second' : seconds < 3600 ? 'minute' : 'hour';
  const scale = unit === 'second' ? 1 : unit === 'minute' ? 60 : 3600;
  const value = Math.max(1, Math.round(seconds / scale));
  return new Intl.NumberFormat(i18n.locale, {
    style: 'unit',
    unit,
    unitDisplay: 'long',
  }).format(value);
}

/** 字节数人类可读（B/KB/MB/GB/TB；≥100 或个位取整，否则 1 位小数）。 */
export function fmtBytes(bytes: number | null | undefined): string {
  if (bytes === null || bytes === undefined || !Number.isFinite(bytes)) return '—';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const text = unit === 0 || value >= 100 ? String(Math.round(value)) : value.toFixed(1);
  return `${text} ${units[unit]}`;
}
