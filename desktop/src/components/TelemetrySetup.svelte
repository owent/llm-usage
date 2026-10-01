<script lang="ts">
  import { onMount } from 'svelte';
  import { telemetry, telemetrySetup, checkTelemetry, configureAllTelemetry,
    prepareTelemetry, applyPreparedTelemetry, undoTelemetry } from '../lib/telemetry.svelte';
  import { t } from '../lib/i18n.svelte';
  import { telemetryReasonKey } from '../lib/telemetry-locales';

  let { full = false, onDetails }: { full?: boolean; onDetails?: () => void } = $props();
  let section = $state<HTMLElement>();
  const pending = $derived(telemetry.rows.filter((row) => row.status !== 'configured'));
  const ready = $derived(telemetry.rows.filter((row) => row.status === 'missing' && row.configurable));
  const results = $derived(new Map(telemetrySetup.results.map((r) => [r.target.id, r])));
  const summary = $derived.by(() => {
    if (telemetrySetup.busy) return telemetrySetup.batch ? t('telemetry.progress', {
      completed: telemetrySetup.completed, total: telemetrySetup.total,
    }) : t('common.loading');
    if (telemetrySetup.results.length) return t('telemetry.result', {
      success: telemetrySetup.results.filter((r) => r.status === 'applied').length,
      failed: telemetrySetup.results.filter((r) => r.status === 'failed').length,
      manual: telemetrySetup.results.filter((r) => r.status === 'manual').length,
    });
    return t('telemetry.brief', { count: pending.length });
  });
  onMount(() => {
    if (!telemetrySetup.busy) void checkTelemetry(full);
    if (full) { section?.focus(); section?.scrollIntoView({ block: 'start' }); }
  });
</script>

{#if full || pending.length || telemetrySetup.results.length || telemetrySetup.busy || telemetry.error || telemetrySetup.error}
  <section bind:this={section} id={full ? 'telemetry-settings' : undefined} tabindex="-1"
    class="telemetry-setup" class:compact={!full} aria-label={t('telemetry.title')}>
    <div class="heading">
      <div>
        <h4>{t('telemetry.title')}</h4>
        {#if !full}<p class="hint" role="status">{summary}</p>{/if}
      </div>
      <div class="actions">
        <button type="button" disabled={telemetrySetup.busy || telemetry.checking || !ready.length}
          onclick={() => void configureAllTelemetry()}>{t('telemetry.configureAll')}</button>
        {#if full}
          <button type="button" disabled={telemetrySetup.busy || telemetry.checking}
            onclick={() => void checkTelemetry(true)}>{t('telemetry.refresh')}</button>
        {:else}
          <button type="button" onclick={() => onDetails?.()}>{t('telemetry.details')}</button>
        {/if}
      </div>
    </div>
    {#if telemetry.error}<p class="error" role="alert">{t(telemetry.error)}</p>{/if}
    {#if telemetrySetup.error}<p class="error" role="alert">{t(telemetrySetup.error)}</p>{/if}
    {#if full}
      <p class="hint">{t('telemetry.hint')}</p>
      <p class="hint">{t('telemetry.bulkHint')}</p>
      {#if telemetry.checking}<p role="status">{t('telemetry.checking')}</p>{/if}
      {#if telemetrySetup.busy || telemetrySetup.results.length}<p role="status">{summary}</p>{/if}
      {#if telemetry.checked && !telemetry.rows.length}<p>{t('telemetry.empty')}</p>{/if}
      {#each telemetry.rows as row (row.id)}
        {@const result = results.get(row.id)}
        <div class="target" data-telemetry-id={row.id}>
          <div class="target-info">
            <strong>{row.name}</strong> <span class="status">{t('telemetry.status.' + row.status)}</span>
            <p class="path">{row.config_path}</p>
            {#if row.reason}<p class="hint">{t(telemetryReasonKey(row.reason))}</p>{/if}
            {#if result?.errorKey}<p class="error">{t(result.errorKey)}</p>{/if}
            {#if telemetrySetup.applied[row.id]}
              <p class="hint">{t(row.kind === 'launcher' ? 'telemetry.launcherHint' : 'telemetry.saved')}</p>
              <p class="path">{row.kind === 'launcher' ? row.config_path : row.output_path}</p>
            {/if}
          </div>
          <div class="actions">
            <a href={row.docs_url} target="_blank" rel="noreferrer">{t('telemetry.guide')}</a>
            {#if row.configurable}
              <button type="button" disabled={telemetrySetup.busy} onclick={() => void prepareTelemetry(row.id)}>
                {t(row.kind === 'launcher' ? 'telemetry.launcher' : 'telemetry.configure')}</button>
            {/if}
            {#if telemetrySetup.applied[row.id]}
              <button type="button" disabled={telemetrySetup.busy} onclick={() => void undoTelemetry(row.id)}>{t('telemetry.undo')}</button>
            {/if}
          </div>
        </div>
      {/each}
      {#if telemetrySetup.preview}
        {@const preview = telemetrySetup.preview}
        <div class="preview" role="region" aria-label={t('telemetry.preview')}>
          <strong>{t('telemetry.preview')} · {preview.target.name}</strong>
          <p class="path">{preview.target.config_path}</p>
          <ul>{#each preview.changes as [key, value]}<li><code>{key} = {JSON.stringify(value)}</code></li>{/each}</ul>
          {#if preview.sync_config_path}
            <p class="path">{preview.sync_config_path}</p>
            <ul>{#each preview.sync_changes ?? [] as [key, value]}<li><code>{key} = {JSON.stringify(value)}</code></li>{/each}</ul>
          {/if}
          <p class="path">{t('telemetry.output')}: {preview.target.output_path}</p>
          <p class="hint">{t(preview.target.kind === 'launcher' ? 'telemetry.launcherHint' : 'telemetry.mergeHint')}</p>
          {#if preview.receiver}<p class="hint">{t('telemetry.receiverHint')}</p>{/if}
          <div class="actions">
            <button type="button" disabled={telemetrySetup.busy} onclick={() => void applyPreparedTelemetry()}>{t('telemetry.apply')}</button>
            <button type="button" disabled={telemetrySetup.busy} onclick={() => telemetrySetup.preview = null}>{t('telemetry.cancel')}</button>
          </div>
        </div>
      {/if}
    {/if}
  </section>
{/if}

<style>
  .telemetry-setup { background: var(--bg-card, #f7f9fc); border: 1px solid var(--border, #dde4ed); border-radius: 8px; padding: 14px 16px; margin: 0 0 16px; min-width: 0; }
  .heading, .target { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  .heading { flex-wrap: wrap; }
  h4 { margin: 0; }
  .compact .hint { margin-bottom: 0; }
  .target { padding: 10px 0; border-top: 1px solid var(--border, #dde4ed); flex-wrap: wrap; }
  .target-info { min-width: 0; flex: 1 1 240px; }
  .status, .hint { font-size: 12px; color: var(--text-secondary, #63738c); }
  .path { font-family: monospace; font-size: 12px; overflow-wrap: anywhere; margin: 5px 0; }
  .actions { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; font-size: 12px; }
  button { cursor: pointer; padding: 5px 10px; border: 1px solid var(--border, #dde4ed); border-radius: 5px; background: var(--bg-input, #fff); color: inherit; }
  button:disabled { cursor: default; opacity: .5; }
  a { color: var(--accent, #2869be); }
  .preview { margin-top: 12px; padding: 12px; border: 1px solid var(--accent, #2869be); border-radius: 6px; }
  ul { padding-left: 20px; font-size: 12px; overflow-wrap: anywhere; }
  .error { color: var(--danger, #c34242); }
</style>
