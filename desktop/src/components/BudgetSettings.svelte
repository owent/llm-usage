<script lang="ts">
  import type { BudgetSettings as Settings, BudgetStatusDto } from '../lib/api';
  import { api } from '../lib/api';
  import { budgetDisplay, budgetThreshold } from '../lib/budget-threshold';
  import { t } from '../lib/i18n.svelte';
  let { value = $bindable(), pricingEnabled }: { value: Settings; pricingEnabled: boolean } = $props();
  let status = $state<BudgetStatusDto | null>(null);
  let raw = $state('');
  let invalid = $state(false);
  $effect(() => { try { raw = budgetDisplay(value.threshold, value.metric); } catch { /* Keep incomplete decimal input for validation. */ } });
  $effect(() => { void api.budgetStatus().then((s) => { status = s; }).catch(() => { status = null; }); });
  function update(rawValue: string) {
    raw = rawValue;
    try { value.threshold = budgetThreshold(rawValue, value.metric); invalid = false; }
    catch { invalid = true; value.threshold = rawValue; }
  }
</script>

<h4>{t('budget.title')}</h4>
<label><input type="checkbox" role="switch" bind:checked={value.enabled} /> {t('budget.enabled')}</label>
<div class="fields">
  <label>{t('budget.period')}<select bind:value={value.period}><option value="day">{t('budget.day')}</option><option value="month">{t('budget.month')}</option></select></label>
  <label>{t('budget.metric')}<select value={value.metric} onchange={(e) => { value.metric = e.currentTarget.value as Settings['metric']; value.threshold = value.metric === 'total_tokens' ? '1000000' : '100000000'; invalid = false; }}><option value="total_tokens">{t('budget.tokens')}</option><option value="estimated_cost">{t('budget.cost')}</option></select></label>
  {#if value.metric === 'estimated_cost'}<label>{t('budget.currency')}<input maxlength="3" value={value.currency} oninput={(e) => { value.currency = e.currentTarget.value.toUpperCase(); }} /></label>{/if}
  <label>{t('budget.threshold')}<input inputmode="decimal" value={raw} aria-invalid={invalid} oninput={(e) => update(e.currentTarget.value)} /></label>
</div>
{#if invalid}<p class="bad">{t('budget.invalid')}</p>{/if}
{#if value.metric === 'estimated_cost' && !pricingEnabled}<p>{t('budget.pricingRequired')}</p>{/if}
<p>{t('budget.hint')}</p>
{#if status?.enabled}<p>{t('budget.current', { amount: status.current === null ? '—' : budgetDisplay(status.current, status.metric), unit: status.metric === 'total_tokens' ? 'token' : status.currency })}</p>{/if}
{#if status?.coverage_limited}<p>{t('budget.limited')}</p>{/if}

<style>
  .fields { display: flex; flex-wrap: wrap; gap: 1rem; margin-top: 1rem; }
  .fields label { display: flex; flex-direction: column; gap: .35rem; }
  input, select { font: inherit; }
  .bad { color: var(--error, #c33); }
  p { font-size: .85rem; }
</style>
