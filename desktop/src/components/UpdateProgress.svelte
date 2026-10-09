<script lang="ts">
  import { updateState, updateAction, updateBusy } from '../lib/update-state.svelte';
  import { t, fmtBytes, i18n } from '../lib/i18n.svelte';
  let { detailed = false }: { detailed?: boolean } = $props();
  const status = $derived(updateState.status);
  const busy = $derived(updateBusy(status));
</script>

<div class="update-progress" data-testid="update-progress">
  <p role="status" aria-live="polite">
    {t(`updates.phase.${status?.phase ?? 'idle'}`, { version: status?.version ?? '' })}
  </p>
  {#if status?.phase === 'downloading'}
    <progress max={status.total_bytes || 1} value={status.downloaded_bytes} aria-label={t('updates.progress')}></progress>
    <span>{fmtBytes(status.downloaded_bytes)} / {fmtBytes(status.total_bytes)}</span>
  {:else if status?.phase === 'checking' || status?.phase === 'verifying'}
    <progress aria-label={t(`updates.phase.${status.phase}`)}></progress>
  {/if}
  {#if detailed && status}
    <p class="muted">{t('updates.current', { version: status.current_version })} · {t(`updates.kind.${status.package_kind}`)}</p>
    {#if status.last_checked_ms}<p class="muted">{t('updates.lastCheck', { time: new Date(status.last_checked_ms).toLocaleString(i18n.locale) })}</p>{/if}
    {#if status.package_kind === 'unknown'}<p>{t('updates.unknownHint')}</p>{/if}
    {#if status.package_kind === 'development'}<p>{t('updates.developmentHint')}</p>{/if}
    {#if status.asset_name}<p class="asset">{status.asset_name}</p>{/if}
  {/if}
  {#if status?.error || updateState.error}
    <p class="error" role="alert">{t('updates.error', { message: updateState.error || status?.error || '' })}</p>
  {/if}
  {#if status?.previous_error && status.previous_error !== status.error}
    <p class="error" role="alert">{t('updates.error', { message: status.previous_error })}</p>
  {/if}
  <div class="actions">
    {#if detailed}<button type="button" data-testid="check-update" disabled={busy || updateState.pending} onclick={() => updateAction('check')}>{t('updates.check')}</button>{/if}
    {#if status?.asset_name && !busy && status.phase !== 'ready'}
      <button type="button" data-testid="download-update" disabled={updateState.pending} onclick={() => updateAction('download')}>{t('updates.download')}</button>
    {/if}
    {#if status?.phase === 'ready'}
      <button type="button" class="primary" data-testid="install-update" disabled={updateState.pending} onclick={() => updateAction('install')}>{t(status.package_kind === 'installer' ? 'updates.install' : 'updates.restart')}</button>
    {/if}
    {#if busy && status?.phase !== 'installing'}
      <button type="button" data-testid="cancel-update" disabled={updateState.cancelling} onclick={() => updateAction('cancel')}>{t('updates.cancel')}</button>
    {/if}
  </div>
  {#if status?.phase === 'ready'}<p class="muted">{t('updates.restartHint')}</p>{/if}
</div>

<style>
  p { margin: .4rem 0; }
  progress { width: min(100%, 22rem); vertical-align: middle; margin-right: .75rem; }
  .actions { display: flex; flex-wrap: wrap; gap: .5rem; margin-top: .5rem; }
  .asset { overflow-wrap: anywhere; }
</style>
