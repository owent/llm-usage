<script lang="ts">
  import { onMount } from 'svelte';
  import { pollUpdates, updateState } from '../lib/update-state.svelte';
  import { t } from '../lib/i18n.svelte';
  import UpdateProgress from './UpdateProgress.svelte';
  let dismissed = $state('');
  const key = $derived(`${updateState.status?.phase}:${updateState.status?.version}:${updateState.status?.asset_name}:${updateState.status?.error}:${updateState.status?.previous_error}:${updateState.error}`);
  const visible = $derived(['available', 'downloading', 'verifying', 'ready', 'installing', 'error'].includes(updateState.status?.phase ?? '') || Boolean(updateState.error || updateState.status?.previous_error));
  onMount(pollUpdates);
</script>

{#if visible && key !== dismissed}
  <aside class="update-notice" aria-label={t('updates.title')} data-testid="update-notice">
    <strong>{t('updates.title')}</strong>
    <UpdateProgress />
    {#if !['downloading','verifying','installing'].includes(updateState.status?.phase ?? '')}
      <button type="button" data-testid="dismiss-update" onclick={() => dismissed = key}>{t('updates.dismiss')}</button>
    {/if}
  </aside>
{/if}

<style>
  .update-notice { margin: .75rem 1rem; padding: .75rem 1rem; border: 1px solid var(--accent, #5078bb); border-radius: .5rem; }
  button { margin-top: .5rem; }
</style>
