<script lang="ts">
  import { api } from '../lib/api';
  import type { HeatmapDto, SummaryQuery } from '../lib/api';
  import { t, i18n, fmtPrecise } from '../lib/i18n.svelte';

  type Cell = HeatmapDto['cells'][number];
  let {
    query, reloadKey = 0, oncells, isDark = false,
  }: {
    query: SummaryQuery;
    reloadKey?: number;
    oncells?: (cells: Cell[]) => void;
    isDark?: boolean;
  } = $props();
  let cells = $state<Cell[]>([]);
  let failed = $state(false);
  let loadSequence = 0;

  // UTC calendar arithmetic preserves the backend's local date buckets across DST.
  const dayMs = (day: string) => Date.parse(day + 'T00:00:00Z');
  const gridStart = $derived(cells.length ? dayMs(cells[0].day) - (cells[0].weekday - 1) * 86_400_000 : 0);
  const weekOf = (day: string) => Math.floor((dayMs(day) - gridStart) / 604_800_000);
  const weekCount = $derived(cells.length ? weekOf(cells[cells.length - 1].day) + 1 : 0);
  const maxCalls = $derived(Math.max(1, ...cells.map((c) => c.calls)));
  const activity = $derived.by(() => {
    let activeDays = 0;
    let streak = 0;
    let longestStreak = 0;
    let peak: Cell | null = null;
    for (const cell of cells) {
      if (cell.available && cell.calls > 0) {
        activeDays++;
        streak++;
        longestStreak = Math.max(longestStreak, streak);
        if (!peak || cell.calls > peak.calls) peak = cell;
      } else {
        streak = 0;
      }
    }
    return { activeDays, longestStreak, peak };
  });
  const weekdayLabels = $derived(Array.from({ length: 7 }, (_, i) =>
    new Intl.DateTimeFormat(i18n.locale, { weekday: 'short', timeZone: 'UTC' })
      .format(new Date(Date.UTC(2024, 0, i + 1)))));
  const monthLabels = $derived.by(() => {
    const labels: { week: number; text: string }[] = [];
    let month = '';
    for (const cell of cells) {
      const next = cell.day.slice(0, 7);
      if (next !== month) {
        month = next;
        const label = {
          week: weekOf(cell.day),
          text: new Intl.DateTimeFormat(i18n.locale, {
            month: 'short',
            ...(labels.length === 0 || cell.day.slice(5, 7) === '01' ? { year: 'numeric' } : {}),
            timeZone: 'UTC',
          }).format(new Date(cell.day + 'T12:00:00Z')),
        };
        if (labels.at(-1)?.week === label.week) labels[labels.length - 1] = label;
        else labels.push(label);
      }
    }
    return labels;
  });

  function level(calls: number): number {
    if (calls <= 0) return 0;
    return Math.min(4, Math.max(1, Math.ceil((calls / maxCalls) * 4)));
  }

  function cellLabel(cell: Cell): string {
    const date = new Intl.DateTimeFormat(i18n.locale, { dateStyle: 'full', timeZone: 'UTC' })
      .format(new Date(cell.day + 'T12:00:00Z'));
    return cell.available
      ? date + ' · ' + t('trend.calls') + ': ' + fmtPrecise(cell.calls) + ' · ' + t('trend.tokens') + ': ' + fmtPrecise(cell.total_tokens) + (cell.partial ? ' · ' + t('heatmap.partial') : '')
      : date + ' · ' + t('heatmap.unavailable');
  }

  function shortDate(day: string): string {
    return new Intl.DateTimeFormat(i18n.locale, { month: 'short', day: 'numeric', timeZone: 'UTC' })
      .format(new Date(day + 'T12:00:00Z'));
  }

  async function load() {
    const sequence = ++loadSequence;
    try {
      const result = await api.heatmap(query);
      if (sequence !== loadSequence) return;
      cells = result.cells;
      failed = false;
      oncells?.(result.cells);
    } catch {
      if (sequence !== loadSequence) return;
      cells = [];
      failed = true;
      oncells?.([]);
    }
  }

  $effect(() => {
    void query;
    void reloadKey;
    ++loadSequence;
    const timer = setTimeout(() => void load(), 200);
    return () => { ++loadSequence; clearTimeout(timer); };
  });
</script>

<div class:dark={isDark} class="heatmap" aria-label={t('heatmap.title')}>
  {#if failed}
    <p class="muted">{t('heatmap.loadFailed')}</p>
  {:else if cells.length}
    <div class="heatmap-main">
    <div class="heatmap-scroll">
      <div class="calendar-wrap">
        <div class="months" style={'--weeks:' + weekCount}>
          {#each monthLabels as month (month.week + month.text)}
            <span style={'grid-column:' + (month.week + 1)}>{month.text}</span>
          {/each}
        </div>
        <div class="calendar-body">
          <div class="weekdays">
            {#each weekdayLabels as label}<span>{label}</span>{/each}
          </div>
          <div class="days" style={'--weeks:' + weekCount}>
            {#each cells as cell (cell.day)}
              <span
                class:unavailable={!cell.available}
                class:partial={cell.partial}
                class:level1={cell.available && level(cell.calls) === 1}
                class:level2={cell.available && level(cell.calls) === 2}
                class:level3={cell.available && level(cell.calls) === 3}
                class:level4={cell.available && level(cell.calls) === 4}
                class:empty={cell.available && cell.calls === 0}
                style={'grid-column:' + (weekOf(cell.day) + 1) + ';grid-row:' + cell.weekday}
                data-day={cell.day}
                title={cellLabel(cell)}
                role="img"
                aria-label={cellLabel(cell)}
              ></span>
            {/each}
          </div>
        </div>
      </div>
    </div>
    <div class="activity-stats">
      <div><span>{t('heatmap.activeDays')}</span><strong>{fmtPrecise(activity.activeDays)}</strong></div>
      <div><span>{t('heatmap.longestStreak')}</span><strong>{fmtPrecise(activity.longestStreak)}</strong></div>
      <div class="peak"><span>{t('heatmap.peakDay')}</span><strong>{activity.peak ? shortDate(activity.peak.day) : '—'}</strong><small>{activity.peak ? fmtPrecise(activity.peak.calls) + ' ' + t('trend.calls') : ''}</small></div>
    </div>
    </div>
    <div class="legend">
      <span>{t('heatmap.noCalls')}</span>
      <span class="swatch empty"></span>
      <span class="swatch level1"></span>
      <span class="swatch level2"></span>
      <span class="swatch level3"></span>
      <span class="swatch level4"></span>
      <span class="legend-gap"></span>
      <span class="swatch unavailable"></span>
      <span>{t('heatmap.unavailable')}</span>
      <span class="swatch partial"></span>
      <span>{t('heatmap.partial')}</span>
    </div>
    <p class="coverage">{t('heatmap.coverage')}</p>
  {:else}
    <p class="muted">{t('common.empty')}</p>
  {/if}
</div>

<style>
  .heatmap { --empty: #e4ebf6; --line: #fff; --level1: #b8cef0; --level2: #719fe1; --level3: #386ec5; --level4: #1d4f9d; color: var(--text-secondary); }
  .heatmap.dark { --empty: #30415f; --line: #161f32; --level1: #405f95; --level2: #4f84ca; --level3: #4db7cb; --level4: #e9bf70; }
  .heatmap-main { display: flex; align-items: flex-start; flex-wrap: wrap; gap: 14px; }
  .heatmap-scroll { overflow-x: auto; max-width: 100%; padding: 8px 2px 12px; }
  .activity-stats { display: grid; grid-template-columns: repeat(2, minmax(95px, 1fr)); gap: 8px; flex: 1; min-width: 190px; padding-top: 8px; }
  .activity-stats > div { display: flex; flex-direction: column; gap: 5px; min-height: 66px; padding: 9px 11px; background: var(--bg-input); border: 1px solid var(--border-light); border-radius: 9px; }
  .activity-stats span { font-size: 11px; color: var(--text-muted); }
  .activity-stats strong { color: var(--text); font-size: 20px; line-height: 1.15; }
  .activity-stats small { color: var(--text-secondary); }
  .activity-stats .peak { grid-column: 1 / -1; }
  .calendar-wrap { --size: 21px; --gap: 4px; width: max-content; }
  .months, .days { display: grid; grid-template-columns: repeat(var(--weeks), var(--size)); gap: var(--gap); }
  .months { margin: 0 0 7px 48px; height: 18px; color: var(--text-muted); font-size: 11px; white-space: nowrap; }
  .months span { align-self: end; }
  .calendar-body { display: flex; gap: 10px; }
  .weekdays { display: grid; grid-template-rows: repeat(7, var(--size)); gap: var(--gap); width: 38px; flex: none; color: var(--text-muted); font-size: 11px; }
  .weekdays span { display: flex; align-items: center; }
  .days { grid-template-rows: repeat(7, var(--size)); }
  .days span, .swatch { display: block; background: var(--empty); border: 1px solid var(--line); border-radius: 5px; box-sizing: border-box; }
  .days span { width: var(--size); height: var(--size); transition: transform .15s ease, box-shadow .15s ease; }
  .days span:hover { transform: scale(1.12); box-shadow: 0 2px 9px #0003; position: relative; z-index: 1; }
  .level1 { background: var(--level1) !important; }
  .level2 { background: var(--level2) !important; }
  .level3 { background: var(--level3) !important; }
  .level4 { background: var(--level4) !important; }
  .unavailable { background: repeating-linear-gradient(135deg, var(--empty) 0 3px, transparent 3px 6px) !important; opacity: .55; }
  .partial { outline: 2px dashed var(--level3); outline-offset: -3px; }
  .legend { display: flex; align-items: center; gap: 5px; color: var(--text-secondary); font-size: 11px; margin-top: 2px; }
  .legend-gap { width: 12px; }
  .swatch { width: 13px; height: 13px; flex: none; }
  .coverage { color: var(--text-muted); font-size: 12px; margin: 5px 0 0; line-height: 1.5; }
  .muted { color: var(--text-muted); font-size: 12px; }
</style>
