/** Display reference only. ECB rates quoted per EUR, verified 2026-10-03. */
export const referenceFx = {
  date: '2026-10-02',
  source: 'https://www.ecb.europa.eu/stats/policy_and_exchange_rates/euro_reference_exchange_rates/html/index.en.html',
  usdPerEur: 11225,
  cnyPerEur: 75259,
} as const;

/** Preserve the caller's integer unit (cents or 1/100 cent per million tokens). */
export function cnyToUsd(currency: string, units: number | null): number | null {
  if (currency !== 'CNY' || units === null || !Number.isSafeInteger(units) || units < 0) return null;
  const denominator = BigInt(referenceFx.cnyPerEur);
  const rounded = (BigInt(units) * BigInt(referenceFx.usdPerEur) + denominator / 2n) / denominator;
  return Number(rounded);
}
