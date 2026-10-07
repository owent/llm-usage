<script lang="ts">
  import type { AppSettings, BudgetStatusDto } from '../lib/api';
  import { api } from '../lib/api';
  import { budgetDisplay } from '../lib/budget-threshold';
  import { t } from '../lib/i18n.svelte';
  let { settings, user }: { settings: AppSettings | null; user: string } = $props();
  let notice = $state<BudgetStatusDto | null>(null);
  $effect(() => {
    const scope = JSON.stringify([user, settings?.budget, settings?.timezone, settings?.pricing?.enabled]);
    notice = null;
    if (!settings?.budget?.enabled) return;
    let active = true;
    let pending = false;
    async function poll() {
      if (!active || pending) return;
      pending = true;
      try {
        const status = await api.budgetStatus(true, user);
        if (active && scope && status.newly_triggered) notice = status;
      } catch { /* The next poll retries an interrupted or changed query. */ }
      finally { pending = false; }
    }
    void poll();
    const timer = setInterval(() => void poll(), 10_000);
    return () => { active = false; clearInterval(timer); };
  });
</script>

{#if notice}
  <aside class="budget-reminder" role="status" aria-live="polite">
    <p>{t('budget.reached', { amount: budgetDisplay(notice.current ?? '0', notice.metric), threshold: budgetDisplay(notice.threshold, notice.metric), unit: notice.metric === 'total_tokens' ? 'token' : notice.currency })}</p>
    {#if notice.coverage_limited}<p>{t('budget.limited')}</p>{/if}
    <button type="button" onclick={() => { notice = null; }}>{t('budget.dismiss')}</button>
  </aside>
{/if}

<style>
  .budget-reminder { margin: .75rem 1rem; padding: .75rem 1rem; border: 1px solid var(--accent, #5078bb); border-radius: .5rem; }
  p { margin: .25rem 0; }
  button { margin-top: .5rem; font: inherit; }
</style>
