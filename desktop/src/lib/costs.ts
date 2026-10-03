import type { CostCurrencyRowDto } from './api';
export function unpricedReasonKey(reason: string): string {
  return ['no_known_usage','no_provider','no_model','channel_unknown','no_price_row','model_ambiguous','tier_ambiguous','token_anomaly','internal_overflow'].includes(reason)
    ? `cost.reason.${reason}` : 'cost.reason.other';
}
export function formatAmount(locale:string,currency:string,minor:unknown):string {
  // ECharts represents missing line points as '-' (not null) in tooltips.
  // Do not coerce null/empty strings to zero or format sentinels as NaN.
  if(typeof minor!=='number' || !Number.isFinite(minor)) return '—';
  try { return new Intl.NumberFormat(locale,{style:'currency',currency,currencyDisplay:'narrowSymbol'}).format(minor/100); }
  catch { return `${(minor/100).toFixed(2)} ${currency}`; }
}
export function pricedAmounts(rows:CostCurrencyRowDto[]):CostCurrencyRowDto[] {
  return rows.filter((r)=>r.currency && r.priced_event_count>0);
}

/** Hundredths of a minor currency unit per million tokens; preserve small rates. */
export function formatUnitPrice(locale:string,currency:string,hundredths:number|null):string {
  if(hundredths===null) return '—';
  return `${currency} ${new Intl.NumberFormat(locale,{maximumFractionDigits:4}).format(hundredths/10000)}`;
}
