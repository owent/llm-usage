import type { BudgetSettings } from './api';

export function budgetThreshold(raw: string, metric: BudgetSettings['metric']): string {
  const input = raw.trim();
  const match = /^(\d+)(?:\.(\d{1,6}))?$/.exec(input);
  if (!match || (metric === 'total_tokens' && match[2])) throw new Error('invalid_budget');
  const amount = metric === 'total_tokens'
    ? BigInt(match[1])
    : BigInt(match[1]) * 1_000_000n + BigInt((match[2] ?? '').padEnd(6, '0'));
  if (amount <= 0n || amount > 9_223_372_036_854_775_807n) throw new Error('invalid_budget');
  return amount.toString();
}

export function budgetDisplay(value: string, metric: BudgetSettings['metric']): string {
  if (metric === 'total_tokens') return value;
  const amount = BigInt(value);
  const fraction = (amount % 1_000_000n).toString().padStart(6, '0').replace(/0+$/, '');
  return `${amount / 1_000_000n}${fraction ? '.' + fraction : ''}`;
}
