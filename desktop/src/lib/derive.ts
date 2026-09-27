/**
 * 前端派生计算（纯函数，无 IPC）：周期耗时汇总、图表分组数据透视等。
 */
import type { ChartSeriesRowDto, PeriodDto } from './api';

/** A model can appear under several providers; a name-only pie combines them. */
export function mergeNamedValues(rows: { name: string; value: number }[]) {
  const sums = new Map<string, number>();
  for (const row of rows) {
    const name = row.name.toLowerCase();
    sums.set(name, (sums.get(name) ?? 0) + row.value);
  }
  return [...sums].map(([name, value]) => ({ name, value })).sort((a, b) => b.value - a.value);
}

export interface DurationStats {
  /** 每次调用平均耗时（毫秒）；无已知数据时 null。 */
  avgMs: number | null;
  /** 已知周期的总耗时（毫秒）；无已知数据时 null。 */
  totalMs: number | null;
}

/** 从 periods 汇总平均/总耗时（只累计 total_duration_ms 已知的周期，避免未知补零）。 */
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

/** 分组单元（一个系列在一个时间标签上的聚合值；null = 未知，不补零）。 */
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

/** chart_series 行透视结果：labels 时间序、names 系列名序、cells 按 (系列, 标签) 取值。 */
export interface ChartGroupData {
  labels: string[];
  names: string[];
  cells: Map<string, ChartGroupCell>;
  cell(name: string, label: string): ChartGroupCell | undefined;
}

/**
 * 把 chart_series 行（后端按 标签+系列 GROUP BY，行唯一）透视为图表可用的
 * 二维结构；缺失的 (系列, 标签) 组合视为该时段无记录（查询时图表按 0/— 处理）。
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
