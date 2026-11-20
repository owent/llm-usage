<script lang="ts">
  import { onMount } from 'svelte';
  import { api, parseError, type PluginPackageDto } from '../lib/api';
  import { t } from '../lib/i18n.svelte';

  let { onupdated }: { onupdated?: () => void } = $props();
  let installed = $state<PluginPackageDto[]>([]);
  let preview = $state<PluginPackageDto | null>(null);
  let busy = $state(false);
  let error = $state('');

  onMount(() => { void run(async () => { installed = await api.listPlugins(); }); });

  async function run(operation: () => Promise<void>) {
    if (busy) return;
    busy = true;
    error = '';
    try {
      await operation();
    } catch (failure) {
      error = parseError(failure);
    } finally {
      busy = false;
    }
  }

  function choose() {
    return run(async () => {
      preview = null;
      preview = await api.inspectPlugin();
    });
  }

  function install(pack: PluginPackageDto) {
    return run(async () => {
      installed = await api.installPlugin(pack.digest);
      preview = null;
    });
  }

  function toggle(pack: PluginPackageDto) {
    return run(async () => {
      installed = await api.setPluginEnabled(pack, !pack.enabled);
      onupdated?.();
    });
  }

  function remove(pack: PluginPackageDto) {
    return run(async () => {
      installed = await api.removePlugin(pack);
      onupdated?.();
    });
  }
</script>

{#snippet identity(pack: PluginPackageDto)}
  <strong>{pack.manifest.name} · {pack.manifest.version}</strong>
  <p>{pack.manifest.id} · {pack.manifest.publisher} · {t('plugins.unverified')}</p>
  <p>{t('plugins.rules', { count: pack.rule_count, version: pack.rule_version })} · {pack.manifest.license}</p>
{/snippet}

<section class="panel" aria-labelledby="plugin-settings-heading" data-testid="plugin-settings">
  <h3 id="plugin-settings-heading">{t('plugins.title')}</h3>
  <p>{t('plugins.hint')}</p>
  <button type="button" disabled={busy} onclick={() => void choose()} data-testid="plugin-choose">{t('plugins.choose')}</button>
  {#if preview}
    {@const pack = preview}
    <div class="entry" data-testid="plugin-preview">
      {@render identity(pack)}
      <code title="SHA-256">{pack.digest}</code>
      <div class="actions">
        <button type="button" disabled={busy} onclick={() => void install(pack)} data-testid="plugin-install">{t('plugins.install')}</button>
        <button type="button" disabled={busy} onclick={() => (preview = null)}>{t('plugins.dismiss')}</button>
      </div>
    </div>
  {/if}
  {#each installed as pack (pack.digest)}
    <div class="entry" data-testid="installed-plugin">
      {@render identity(pack)}
      <p role="status">{t(pack.enabled ? 'plugins.enabled' : 'plugins.disabled')}</p>
      {#if !pack.enabled && installed.some(other => other.enabled && other.manifest.id === pack.manifest.id)}
        <p>{t('plugins.switch')}</p>
      {/if}
      <div class="actions">
        <button type="button" disabled={busy} onclick={() => void toggle(pack)} data-testid="plugin-toggle">
          {t(pack.enabled ? 'plugins.disable' : 'plugins.enable')}
        </button>
        <button type="button" disabled={busy} onclick={() => void remove(pack)} data-testid="plugin-remove">{t('plugins.remove')}</button>
      </div>
    </div>
  {/each}
  {#if error}<p role="alert">{t('plugins.error', { code: error })}</p>{/if}
</section>

<style>
  .entry { margin-top: 1rem; padding-block-start: .75rem; border-block-start: 1px solid var(--border); overflow-wrap: anywhere; }
  .entry p { margin-block: .4rem; }
  .actions { display: flex; flex-wrap: wrap; gap: .5rem; margin-top: .6rem; }
</style>
