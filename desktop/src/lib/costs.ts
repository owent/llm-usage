import type { CostCurrencyRowDto } from './api';
export function formatAmount(locale:string,currency:string,minor:number):string {
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
