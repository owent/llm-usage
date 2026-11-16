<script lang="ts">
  import { onMount } from 'svelte';
  import { api, parseError, type ModelAliasSettings, type ModelAliasStatusDto } from '../lib/api';
  import { t, i18n } from '../lib/i18n.svelte';
  let { value = $bindable(), ttl = $bindable(), beforeAction, onupdated }: {
    value: ModelAliasSettings; ttl: string; beforeAction: () => Promise<boolean>; onupdated?: () => void;
  } = $props();
  let status = $state<ModelAliasStatusDto | null>(null);
  let busy = $state(false);
  let error = $state('');
  let message = $state('');
  let active = true;
  async function load() {
    try {
      const next = await api.modelAliasStatus();
      if (!active) return;
      const changed = status !== null && status.revision !== next.revision;
      status = next;
      if (changed) onupdated?.();
    } catch (e) { if (active) error = parseError(e); }
  }
  onMount(() => {
    void load();
    const timer = setInterval(() => void load(), 2000);
    return () => { active = false; clearInterval(timer); };
  });
  async function run(importFile: boolean) {
    if (busy) return;
    busy = true; error = ''; message = '';
    try {
      if (!await beforeAction()) return;
      if (importFile) {
        const path = await api.pickOpenPath('json');
        if (path === null) return;
        const result = await api.importModelAliases(path);
        message = t('models.imported', { count: result.inserted_rules });
        onupdated?.();
      } else {
        await api.refreshModelAliases();
      }
      await load();
    } catch (e) { error = parseError(e); }
    finally { busy = false; }
  }
</script>

<section class="panel" data-testid="model-alias-settings">
  <h3>{t('models.title')}</h3>
  <p>{t('models.hint')}</p>
  <label><input class="switch" role="switch" type="checkbox" bind:checked={value.online_refresh_enabled} data-testid="model-alias-enabled" /> {t('models.enabled')}</label>
  <p class="muted">{t('models.network')}</p>
  <label>{t('models.ttl')} <input bind:value={ttl} inputmode="numeric" data-testid="model-alias-ttl" aria-label={t('models.ttl')} /> {t('models.days')}</label>
  <div class="actions">
    <button type="button" onclick={() => void run(false)} disabled={busy || status?.running || !value.online_refresh_enabled} data-testid="model-alias-refresh">{t('models.refresh')}</button>
    <button type="button" onclick={() => void run(true)} disabled={busy} data-testid="model-alias-import">{t('models.import')}</button>
  </div>
  {#if status}
    <p role="status" data-testid="model-alias-status">{t('models.rules', { count: status.rules, generation: status.generation ?? '—' })}
      {#if status.running} · {t('models.running')}{/if}
    </p>
    {#if status.cache}
      <p class="muted">{t('models.checked', { time: new Date(status.cache.fetched_at_ms).toLocaleString(i18n.locale) })} · {status.cache.rule_version}</p>
    {:else}<p class="muted">{t('models.noCache')}</p>{/if}
    {#if status.error}<p class="bad" role="alert">{t('models.failed', { message: status.error })}</p>{/if}
    {#if status.last_outcome?.error}<p class="bad" role="alert">{t('models.failed', { message: status.last_outcome.error })}</p>{/if}
    {#if status.last_outcome?.first_error}<p class="muted">{t('models.firstError', { message: status.last_outcome.first_error })}</p>{/if}
  {/if}
  {#if message}<p role="status">{message}</p>{/if}
  {#if error}<p class="bad" role="alert">{t('models.failed', { message: error })}</p>{/if}
  <p class="muted">{t('models.conflicts')}</p>
  <p class="muted"><a href={`https://llm-usage.atframe.work/${i18n.locale.startsWith('zh') ? 'zh-cn/' : ''}reference/design/model-aliases/`} target="_blank" rel="noreferrer">{t('models.format')}</a></p>
</section>

<style>
  label { display: flex; align-items: center; gap: .6rem; margin: .8rem 0; flex-wrap: wrap; }
  input:not([type=checkbox]) { width: 5rem; }
  .actions { display: flex; gap: .6rem; flex-wrap: wrap; }
  .muted { color: var(--text-muted); font-size: .9em; }
  .bad { color: var(--danger); }
</style>
