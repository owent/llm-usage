export function createZedTimeFormatter(locale: string, timezone: string): Intl.DateTimeFormat {
  const options: Intl.DateTimeFormatOptions = { dateStyle: 'medium', timeStyle: 'short' };
  try {
    return new Intl.DateTimeFormat(locale, { ...options, timeZone: timezone });
  } catch {
    try {
      return new Intl.DateTimeFormat(locale, { ...options, timeZone: 'UTC' });
    } catch {
      return new Intl.DateTimeFormat('en', { ...options, timeZone: 'UTC' });
    }
  }
}

export function formatZedInterval(
  startMs: number | null,
  endMs: number,
  formatter: Intl.DateTimeFormat,
): string {
  const start = startMs !== null && Number.isFinite(startMs) ? formatter.format(startMs) : '—';
  const end = Number.isFinite(endMs) ? formatter.format(endMs) : '—';
  return `${start} – ${end}`;
}

export function shouldShowZedThreads(
  ready: boolean,
  tab: string,
  agentFilter: string,
  modelFilter: string,
): boolean {
  return ready && tab === 'overview' && (!agentFilter || agentFilter === 'zed') && !modelFilter;
}

export function availableAgents(
  catalogAgents: { agent: string }[] | undefined,
  sources: { agent: string; user_id: string }[],
  currentUser: string,
  hasZedThreads: boolean,
): string[] {
  return Array.from(new Set([
    ...(catalogAgents ?? []).map((a) => a.agent),
    ...sources.filter((s) => s.user_id === currentUser && s.agent === 'zed').map((s) => s.agent),
    ...(hasZedThreads ? ['zed'] : []),
  ])).sort();
}

export function shouldHideEmptyOverview(
  showZedThreads: boolean,
  zedThreadCount: number,
  zedThreadsError: string,
): boolean {
  return showZedThreads && (zedThreadCount > 0 || zedThreadsError !== '');
}
