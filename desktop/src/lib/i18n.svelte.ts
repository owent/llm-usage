/**
 * 多语言消息目录（F3 合同：docs/design/desktop-usage/i18n.md）。
 * - zh-CN 默认语言 + en；语言协商：用户设置 → 系统语言 → 默认；
 * - 缺失键回退默认语言并 console.warn（开发可诊断）；
 * - 键名点分命名空间，{name} 插值；复数按 key.one/key.other（zh 只有 other）；
 * - 数字/日期/百分比用 Intl（ECMA-402）；统计口径（时区/周起始）不随语言改变；
 * - 切换即时生效（runes 响应式，无需重启）。
 */
export type Locale = 'zh-CN' | 'en';

const zhCN: Record<string, string> = {
  'app.title': 'LLM 用量',
  'app.subtitle': '本地 Agent 用量看板',
  'nav.overview': '总览',
  'nav.trend': '趋势',
  'nav.sources': '数据源',
  'nav.settings': '设置',
  'action.refresh': '刷新今日',
  'action.refreshing': '采集中…',
  'action.export': '导出',
  'action.exportCsv': '导出 CSV（展示用）',
  'action.exportExchange': '导出交换 JSON（含来源身份）',
  'common.loading': '加载中…',
  'common.empty': '尚无数据。启用数据源并刷新后，这里会显示用量统计。',
  'common.error': '查询失败：{message}',
  'common.unknown': '未知',
  'common.all': '全部',
  'cards.calls': '模型调用',
  'cards.input': '输入 token',
  'cards.output': '输出 token',
  'cards.total': '总 token',
  'cards.cacheRead': '缓存读',
  'cards.cacheWrite': '缓存写',
  'cards.cacheRatio': '缓存输入占比',
  'cards.conflicts': '冲突记录',
  'cards.excluded': '未计入（归属未核验）',
  'cards.revision': '数据修订 {revision}',
  'filter.range': '范围',
  'filter.granularity.day': '日',
  'filter.granularity.week': '周',
  'filter.granularity.month': '月',
  'filter.agent': 'Agent',
  'filter.model': '模型',
  'filter.quick.7': '近 7 天',
  'filter.quick.30': '近 30 天',
  'filter.quick.365': '近一年',
  'trend.title': '用量趋势',
  'trend.calls': '调用次数',
  'trend.tokens': 'token',
  'trend.inProgress': '（进行中）',
  'trend.partial': '（部分历史）',
  'hourly.title': '今日逐小时',
  'heatmap.title': '活跃热力图（周 × 小时）',
  'table.model': '模型',
  'table.agent': 'Agent',
  'table.calls': '调用',
  'table.input': '输入',
  'table.output': '输出',
  'table.total': '总量',
  'table.cacheRead': '缓存读',
  'table.cacheWrite': '缓存写',
  'table.unknownFields': '未知字段 {count}',
  'sources.title': '数据源',
  'sources.agent': 'Agent',
  'sources.instance': '实例',
  'sources.health': '健康',
  'sources.status': '状态',
  'sources.lastSuccess': '最近成功',
  'sources.compatFiles': '兼容尝试 {count} 个文件（版本未验证）',
  'sources.incompatibleFiles': '不兼容 {count} 个文件',
  'sources.enabled': '启用',
  'sources.disabled': '停用',
  'sources.none': '未发现本机数据源。安装并使用对应 Agent 后重启应用。',
  'refresh.status.running': '正在采集 {agent}…',
  'refresh.status.done': '上次刷新 {time}：新增 {added}、更新 {updated}',
  'settings.title': '设置',
  'settings.timezone': '统计时区',
  'settings.weekStart': '周起始',
  'settings.weekStart.monday': '周一',
  'settings.weekStart.sunday': '周日',
  'settings.retention': '明细保留（天，留空不限）',
  'settings.interval': '自动刷新间隔（秒，0 关闭）',
  'settings.language': '界面语言',
  'settings.manualRoots': '手工数据源根目录（每行一个）',
  'settings.saved': '已保存',
  'settings.saveFailed': '保存失败：{message}',
  'settings.host': '本机来源身份',
  'settings.dbPath': '数据库位置',
  'settings.note': '时区/周起始只影响统计展示口径，不改变已存储数据。',
  'export.done': '已导出：{path}',
  'export.failed': '导出失败：{message}',
};

const en: Record<string, string> = {
  'app.title': 'LLM Usage',
  'app.subtitle': 'Local agent usage dashboard',
  'nav.overview': 'Overview',
  'nav.trend': 'Trends',
  'nav.sources': 'Sources',
  'nav.settings': 'Settings',
  'action.refresh': 'Refresh today',
  'action.refreshing': 'Scanning…',
  'action.export': 'Export',
  'action.exportCsv': 'Export CSV (display)',
  'action.exportExchange': 'Export exchange JSON (with provenance)',
  'common.loading': 'Loading…',
  'common.empty': 'No data yet. Enable sources and refresh; usage will appear here.',
  'common.error': 'Query failed: {message}',
  'common.unknown': 'unknown',
  'common.all': 'All',
  'cards.calls': 'Model calls',
  'cards.input': 'Input tokens',
  'cards.output': 'Output tokens',
  'cards.total': 'Total tokens',
  'cards.cacheRead': 'Cache read',
  'cards.cacheWrite': 'Cache write',
  'cards.cacheRatio': 'Cached input ratio',
  'cards.conflicts': 'Conflicting records',
  'cards.excluded': 'Excluded (unverified)',
  'cards.revision': 'Data revision {revision}',
  'filter.range': 'Range',
  'filter.granularity.day': 'Day',
  'filter.granularity.week': 'Week',
  'filter.granularity.month': 'Month',
  'filter.agent': 'Agent',
  'filter.model': 'Model',
  'filter.quick.7': 'Last 7 days',
  'filter.quick.30': 'Last 30 days',
  'filter.quick.365': 'Last year',
  'trend.title': 'Usage trend',
  'trend.calls': 'Calls',
  'trend.tokens': 'Tokens',
  'trend.inProgress': ' (in progress)',
  'trend.partial': ' (partial history)',
  'hourly.title': 'Today by hour',
  'heatmap.title': 'Activity heatmap (weekday × hour)',
  'table.model': 'Model',
  'table.agent': 'Agent',
  'table.calls': 'Calls',
  'table.input': 'Input',
  'table.output': 'Output',
  'table.total': 'Total',
  'table.cacheRead': 'Cache read',
  'table.cacheWrite': 'Cache write',
  'table.unknownFields': '{count} unknown fields',
  'sources.title': 'Data sources',
  'sources.agent': 'Agent',
  'sources.instance': 'Instance',
  'sources.health': 'Health',
  'sources.status': 'Status',
  'sources.lastSuccess': 'Last success',
  'sources.compatFiles': '{count} files via compat attempt (unverified)',
  'sources.incompatibleFiles': '{count} incompatible files',
  'sources.enabled': 'Enabled',
  'sources.disabled': 'Disabled',
  'sources.none': 'No local sources discovered. Install and use an agent, then restart.',
  'refresh.status.running': 'Scanning {agent}…',
  'refresh.status.done': 'Last refresh {time}: added {added}, updated {updated}',
  'settings.title': 'Settings',
  'settings.timezone': 'Statistics timezone',
  'settings.weekStart': 'Week starts on',
  'settings.weekStart.monday': 'Monday',
  'settings.weekStart.sunday': 'Sunday',
  'settings.retention': 'Detail retention (days, empty = unlimited)',
  'settings.interval': 'Auto refresh interval (seconds, 0 = off)',
  'settings.language': 'Language',
  'settings.manualRoots': 'Manual source roots (one per line)',
  'settings.saved': 'Saved',
  'settings.saveFailed': 'Save failed: {message}',
  'settings.host': 'Local origin host',
  'settings.dbPath': 'Database location',
  'settings.note': 'Timezone/week start only affect display; stored data is unchanged.',
  'export.done': 'Exported: {path}',
  'export.failed': 'Export failed: {message}',
};

const catalogs: Record<Locale, Record<string, string>> = {
  'zh-CN': zhCN,
  en,
};

export const DEFAULT_LOCALE: Locale = 'zh-CN';

function detectSystemLocale(): Locale {
  for (const tag of navigator.languages ?? [navigator.language]) {
    if (tag.startsWith('zh')) return 'zh-CN';
    if (tag.startsWith('en')) return 'en';
  }
  return DEFAULT_LOCALE;
}

export const i18n = $state({
  locale: DEFAULT_LOCALE as Locale,
  /** 用户显式选择过的语言（设置持久化）；null 表示尚未选择，用系统语言。 */
  userChoice: null as Locale | null,
});

export function initLocale(userChoice: string | null): void {
  i18n.userChoice = (userChoice === 'zh-CN' || userChoice === 'en' ? userChoice : null);
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
  if (minutes < 1) return '<1 min';
  if (minutes < 60) return `${minutes} min`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours} h`;
  return `${Math.round(hours / 24)} d`;
}
