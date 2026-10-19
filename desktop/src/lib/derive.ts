/**
 * Pure frontend calculations without IPC: period durations and grouped chart data.
 */
import type { ChartSeriesRowDto, PeriodDto, MetricSumsDto } from './api';

export type TokenMetric = 'input' | 'output' | 'total';
export interface TokenDisplay { value: string | null; lower: boolean }

/** Conservative display bounds over identical partitions; never add overlapping totals and buckets. */
export function tokenDisplay(sums: Partial<MetricSumsDto>, metric: TokenMetric): TokenDisplay {
  const integer = (v: unknown): bigint | null => typeof v === 'string' && /^\d+$/.test(v) ? BigInt(v) : null;
  const sum = (values: (bigint | null)[]) => values.every(v=>v===null) ? null : values.reduce<bigint>((a,v)=>a+(v??0n),0n);
  const max = (a: bigint | null,b: bigint | null) => a===null ? b : b===null ? a : a>b ? a : b;
  const input = integer(sums.input_total_known);
  const output = integer(sums.output_total_known);
  const total = integer(sums.total_tokens_known);
  const inputBound = max(input,sum([sums.uncached_known,sums.cache_read_known,sums.cache_write_known].map(integer)));
  const incompleteInput = input===null || (sums.input_unknown_count??0)>0;
  const incompleteTotal = total===null || (sums.total_unknown_count??0)>0;
  const value = metric==='input' ? (incompleteInput ? inputBound : input)
    : metric==='output' ? output : incompleteTotal ? max(total,sum([inputBound,output])) : total;
  const lower = metric==='input' ? incompleteInput : metric==='output'
    ? (sums.output_unknown_count??0)>0 : incompleteTotal;
  return {value:value?.toString()??null,lower:value!==null && lower};
}

export function formatTokenDisplay(value: TokenDisplay, format: (value:string|null)=>string): string {
  return `${value.lower ? '≥ ' : ''}${format(value.value)}`;
}

export function tokenShare(sums: MetricSumsDto) {
  const metrics = Object.fromEntries((['input','output','total'] as const).map(metric=>[metric,tokenDisplay(sums,metric)])) as Record<TokenMetric,TokenDisplay>;
  return {value:metrics.total.value===null?null:Number(metrics.total.value),
    input:metrics.input.value===null?null:Number(metrics.input.value),
    output:metrics.output.value===null?null:Number(metrics.output.value),
    lower:{total:metrics.total.lower,input:metrics.input.lower,output:metrics.output.lower}};
}

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
  display: Record<TokenMetric,TokenDisplay>;
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
      const sums = r.sums ?? {input_total_known:r.input,output_total_known:r.output,total_tokens_known:r.total,
        uncached_known:r.uncached,cache_read_known:r.cache_read,cache_write_known:r.cache_write};
      cells.set(key(r.series, r.label), {
        calls: r.calls,
        input: r.input === null ? null : Number(r.input),
        cacheRead: r.cache_read === null ? null : Number(r.cache_read),
        cacheWrite: r.cache_write == null ? null : Number(r.cache_write),
        uncached: r.uncached == null ? null : Number(r.uncached),
        cacheRatio: r.cache_ratio ?? null,
        output: r.output === null ? null : Number(r.output),
        total: r.total === null ? null : Number(r.total),
        display:{input:tokenDisplay(sums,'input'),output:tokenDisplay(sums,'output'),total:tokenDisplay(sums,'total')},
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
