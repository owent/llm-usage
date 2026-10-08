<script lang="ts">
  import type { UpdateSettings } from '../lib/api';
  import { t } from '../lib/i18n.svelte';
  import UpdateProgress from './UpdateProgress.svelte';
  let { value = $bindable() }: { value: UpdateSettings } = $props();
</script>

<section class="panel" data-testid="update-settings">
  <h3>{t('updates.title')}</h3>
  <label>{t('updates.schedule')}
    <select bind:value={value.schedule} data-testid="update-schedule">
      {#each ['launch','daily','weekly','manual'] as choice}
        <option value={choice}>{t(`updates.schedule.${choice}`)}</option>
      {/each}
    </select>
  </label>
  <label class="switch-field"><input class="switch" role="switch" type="checkbox" bind:checked={value.auto_download} data-testid="update-auto-download" /> <span>{t('updates.autoDownload')}</span></label>
  <p class="muted">{t('updates.settingsHint')}</p>
  <UpdateProgress detailed />
</section>

<style>
  label { display: flex; align-items: center; gap: .6rem; margin: .75rem 0; flex-wrap: wrap; }
</style>
