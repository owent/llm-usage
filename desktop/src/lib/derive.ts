/**
 * Pure frontend calculations without IPC: period durations and grouped chart data.
 */
import type { ChartSeriesRowDto, PeriodDto } from './api';

/** Native Copilot turn input is a lower bound for display only;
 * never replace complete query/export totals with it. */
export function observedTokenTotal(input:string|number|null,output:string|number|null):string|null {
  if(input===null || output===null
    || (typeof input==='number' && !Number.isSafeInteger(input))
    || (typeof output==='number' && !Number.isSafeInteger(output))
    || !/^\d+$/.test(String(input)) || !/^\d+$/.test(String(output))) return null;
  return (BigInt(input)+BigInt(output)).toString();
}

/** Short display keeps complete totals, observed bounds and unknown distinct. */
export function tokenTotalLabel(locale:string,total:string|number|null,input:string|number|null,output:string|number|null):string {
  const value=total===null ? observedTokenTotal(input,output) : observedTokenTotal(total,0);
  return value===null ? '—' : `${total===null ? '≥ ' : ''}${BigInt(value).toLocaleString(locale)}`;
}

/** A model can appear under several providers; name-only pies combine them. */
export function mergeNamedValues(rows: { name: string; value: number }[]) {
  const sums = new Map<string, number>();
  for (const row of rows) {
    const name = row.name.toLowerCase();
    sums.set(name, (sums.get(name) ?? 0) + row.value);
  }
  return [...sums].map(([name, value]) => ({ name, value })).sort((a, b) => b.value - a.value);
}

export interface DurationStats {
  /** Mean call duration in milliseconds; null without known data. */
  avgMs: number | null;
  /** Known period duration sum in milliseconds; null without known data. */
  totalMs: number | null;
}

/** Sum/average known total_duration_ms periods only, without zero-filling unknowns. */
export function durationStatsOf(periods: PeriodDto[]): DurationStats {
  let totalMs = 0;
  let calls = 0;
  let known = false;
  for (const p of periods) {
    if (p.sums.total_duration_ms !== null) {
      totalMs += Number(p.sums.total_duration_ms);
      calls += p.sums.duration_sample_count;
      known = true;
    }
  }
  return {
    avgMs: known && calls > 0 ? totalMs / calls : null,
    totalMs: known ? totalMs : null,
  };
}

/** One series/time-label value; null means unknown, not zero. */
export interface ChartGroupCell {
  calls: number;
  input: number | null;
  cacheRead: number | null;
  cacheWrite: number | null;
  uncached: number | null;
  cacheRatio: number | null;
  output: number | null;
  total: number | null;
}

/** chart_series output: chronological labels, series names and cells keyed by series/label. */
export interface ChartGroupData {
  labels: string[];
  names: string[];
  cells: Map<string, ChartGroupCell>;
  cell(name: string, label: string): ChartGroupCell | undefined;
}

/**
 * Turn unique backend GROUP BY label/series rows into chart arrays.
 * Missing series/label pairs mean no records for that period; queries
 * let charts render those gaps as zero or a dash.
 */
export function pivotChartSeries(rows: ChartSeriesRowDto[]): ChartGroupData {
  const labels: string[] = [];
  const names: string[] = [];
  const labelSet = new Set<string>();
  const nameSet = new Set<string>();
  const cells = new Map<string, ChartGroupCell>();
  const key = (name: string, label: string) => `${name}\u0000${label}`;
  for (const r of rows) {
    if (!labelSet.has(r.label)) {
      labelSet.add(r.label);
      labels.push(r.label);
    }
    if (!nameSet.has(r.series)) {
      nameSet.add(r.series);
      names.push(r.series);
    }
    if (!cells.has(key(r.series, r.label))) {
      cells.set(key(r.series, r.label), {
        calls: r.calls,
        input: r.input === null ? null : Number(r.input),
        cacheRead: r.cache_read === null ? null : Number(r.cache_read),
        cacheWrite: r.cache_write == null ? null : Number(r.cache_write),
        uncached: r.uncached == null ? null : Number(r.uncached),
        cacheRatio: r.cache_ratio ?? null,
        output: r.output === null ? null : Number(r.output),
        total: r.total === null ? null : Number(r.total),
      });
    }
  }
  names.sort((a, b) => a.localeCompare(b));
  return {
    labels,
    names,
    cells,
    cell: (name, label) => cells.get(key(name, label)),
  };
}
